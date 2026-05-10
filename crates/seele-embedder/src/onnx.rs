//! ONNX-backed implementation of `Embedder`.
//!
//! Default model is `sentence-transformers/all-MiniLM-L6-v2` (384-dim).
//! On first construction, the ONNX model + tokenizer are downloaded via
//! `hf-hub` and cached in `~/.cache/huggingface/`.
//!
//! Inference recipe matches the sentence-transformers reference: mean
//! pooling over token embeddings with attention-mask weighting, then L2
//! normalization. Without that recipe the cosine similarity in vec0 would
//! be miscalibrated.

use std::path::PathBuf;
use std::sync::Mutex;

use hf_hub::api::sync::Api;
use ndarray::{s, Array1, Array2};
use ort::session::Session;
use ort::value::TensorRef;
use tokenizers::Tokenizer;

use crate::embedder::Embedder;
use crate::error::{EmbedderError, Result};

const DEFAULT_MODEL_REPO: &str = "sentence-transformers/all-MiniLM-L6-v2";
const ONNX_PATH_IN_REPO: &str = "onnx/model.onnx";
const TOKENIZER_PATH_IN_REPO: &str = "tokenizer.json";
const DEFAULT_DIM: usize = 384;
const DEFAULT_MAX_LEN: usize = 256;

#[derive(Debug, Clone)]
pub struct OnnxConfig {
    pub model_repo: String,
    pub max_seq_len: usize,
    pub dim: usize,
}

impl Default for OnnxConfig {
    fn default() -> Self {
        Self {
            model_repo: DEFAULT_MODEL_REPO.to_string(),
            max_seq_len: DEFAULT_MAX_LEN,
            dim: DEFAULT_DIM,
        }
    }
}

pub struct OnnxEmbedder {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    config: OnnxConfig,
}

impl OnnxEmbedder {
    /// Build an embedder with the default sentence-transformers model.
    /// Downloads the model + tokenizer on first call (cached after).
    pub fn new() -> Result<Self> {
        Self::with_config(OnnxConfig::default())
    }

    pub fn with_config(config: OnnxConfig) -> Result<Self> {
        let (model_path, tokenizer_path) = resolve_model_files(&config.model_repo)?;
        let tokenizer = Tokenizer::from_file(&tokenizer_path)?;
        let session = Session::builder()?.commit_from_file(&model_path)?;
        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            config,
        })
    }

    fn run_inference(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let batch = texts.len();
        let max_len = self.config.max_seq_len;

        // Tokenize batch.
        let mut input_ids = Array2::<i64>::zeros((batch, max_len));
        let mut attention_mask = Array2::<i64>::zeros((batch, max_len));
        let mut token_type_ids = Array2::<i64>::zeros((batch, max_len));

        for (i, text) in texts.iter().enumerate() {
            let encoding = self.tokenizer.encode(*text, true)?;
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            let type_ids = encoding.get_type_ids();
            let n = ids.len().min(max_len);
            for j in 0..n {
                input_ids[(i, j)] = ids[j] as i64;
                attention_mask[(i, j)] = mask[j] as i64;
                token_type_ids[(i, j)] = type_ids[j] as i64;
            }
        }

        // Run the session, extract data, then drop everything that borrows
        // from session so we can release the lock before pooling.
        let (seq, dim, hidden_buf) = {
            let mut session = self.session.lock().expect("OnnxEmbedder mutex poisoned");
            let outputs = session.run(ort::inputs![
                "input_ids" => TensorRef::from_array_view(&input_ids)?,
                "attention_mask" => TensorRef::from_array_view(&attention_mask)?,
                "token_type_ids" => TensorRef::from_array_view(&token_type_ids)?,
            ])?;
            let (shape, hidden) = outputs["last_hidden_state"].try_extract_tensor::<f32>()?;
            if shape.len() != 3 {
                return Err(EmbedderError::Ort(format!(
                    "expected last_hidden_state rank 3, got shape {:?}",
                    shape
                )));
            }
            let dim = shape[2] as usize;
            if dim != self.config.dim {
                return Err(EmbedderError::DimensionMismatch {
                    expected: self.config.dim,
                    actual: dim,
                });
            }
            let seq = shape[1] as usize;
            // Copy hidden state out of the session-owned buffer so the lock
            // can be released before we do CPU-bound pooling work.
            (seq, dim, hidden.to_vec())
        };

        let hidden_view = ndarray::ArrayView3::from_shape((batch, seq, dim), &hidden_buf)
            .map_err(|e| EmbedderError::Ort(format!("bad hidden shape: {e}")))?;

        let mut out = Vec::with_capacity(batch);
        for i in 0..batch {
            let mut pooled = Array1::<f32>::zeros(dim);
            let mut mask_sum = 0.0_f32;
            for j in 0..seq {
                let m = attention_mask[(i, j)] as f32;
                if m == 0.0 {
                    continue;
                }
                mask_sum += m;
                let token_embed = hidden_view.slice(s![i, j, ..]);
                pooled.scaled_add(m, &token_embed);
            }
            if mask_sum < 1e-9 {
                mask_sum = 1e-9;
            }
            pooled.mapv_inplace(|v| v / mask_sum);
            l2_normalize(&mut pooled);
            out.push(pooled.to_vec());
        }
        Ok(out)
    }
}

impl Embedder for OnnxEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if text.is_empty() {
            return Err(EmbedderError::EmptyInput("text"));
        }
        let mut v = self.run_inference(&[text])?;
        v.pop()
            .ok_or(EmbedderError::EmptyInput("empty inference result"))
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if texts.iter().any(|t| t.is_empty()) {
            return Err(EmbedderError::EmptyInput(
                "at least one text in batch is empty",
            ));
        }
        self.run_inference(texts)
    }

    fn dim(&self) -> usize {
        self.config.dim
    }

    fn model_id(&self) -> &str {
        &self.config.model_repo
    }
}

fn resolve_model_files(repo: &str) -> Result<(PathBuf, PathBuf)> {
    let api = Api::new().map_err(|e| EmbedderError::HfHub(e.to_string()))?;
    let model = api.model(repo.to_string());
    let model_path = model.get(ONNX_PATH_IN_REPO)?;
    let tokenizer_path = model.get(TOKENIZER_PATH_IN_REPO)?;
    Ok((model_path, tokenizer_path))
}

fn l2_normalize(v: &mut Array1<f32>) {
    let norm = v.dot(v).sqrt().max(1e-12);
    v.mapv_inplace(|x| x / norm);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l2_normalize_makes_unit_vector() {
        let mut v = Array1::from_vec(vec![3.0_f32, 4.0]);
        l2_normalize(&mut v);
        let norm = v.dot(&v).sqrt();
        assert!((norm - 1.0).abs() < 1e-6, "norm = {norm}");
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_handles_zero_vector_safely() {
        let mut v = Array1::<f32>::zeros(4);
        l2_normalize(&mut v); // must not divide by zero
        assert!(v.iter().all(|&x| x == 0.0));
    }

    /// Real ONNX integration is `#[ignore]` because:
    ///  1. First run downloads ~90MB from Hugging Face.
    ///  2. CI runners shouldn't pay that cost on every push.
    ///
    /// Run locally with `cargo test --package seele-embedder -- --ignored`.
    #[test]
    #[ignore = "downloads model from hugging face; run locally"]
    fn embed_returns_unit_vector_of_correct_dim() {
        let emb = OnnxEmbedder::new().expect("embedder init");
        let v = emb.embed("Postgres uses WAL for crash recovery.").unwrap();
        assert_eq!(v.len(), DEFAULT_DIM);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "norm = {norm}");
    }

    #[test]
    #[ignore = "downloads model from hugging face; run locally"]
    fn similar_texts_have_higher_cosine_than_unrelated() {
        let emb = OnnxEmbedder::new().expect("embedder init");
        let a = emb.embed("Postgres uses WAL for crash recovery.").unwrap();
        let b = emb
            .embed("PostgreSQL write-ahead log helps recover after crashes.")
            .unwrap();
        let c = emb.embed("Tropical beaches in Bora Bora.").unwrap();
        let dot = |x: &[f32], y: &[f32]| -> f32 { x.iter().zip(y).map(|(a, b)| a * b).sum() };
        let ab = dot(&a, &b);
        let ac = dot(&a, &c);
        assert!(ab > ac, "ab={ab} ac={ac}");
    }
}
