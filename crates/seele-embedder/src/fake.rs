//! Deterministic in-memory embedder for downstream tests.
//!
//! Hashes the input text + a per-axis seed into a 384-dim vector, then
//! L2-normalizes. Two near-identical strings produce close-but-distinct
//! vectors; unrelated strings produce vectors that are decorrelated.

use sha2::{Digest, Sha256};

use crate::embedder::Embedder;
use crate::error::Result;

const FAKE_DIM: usize = 384;
const FAKE_MODEL_ID: &str = "seele/fake-embedder";

#[derive(Default, Clone)]
pub struct FakeEmbedder;

impl FakeEmbedder {
    pub fn new() -> Self {
        Self
    }

    fn hash_to_vector(text: &str) -> Vec<f32> {
        let mut v = vec![0.0_f32; FAKE_DIM];
        // 32 bytes per axis-block; we need FAKE_DIM/8=48 blocks of 8 bytes.
        for (block_idx, chunk) in v.chunks_mut(8).enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(text.as_bytes());
            hasher.update((block_idx as u32).to_le_bytes());
            let digest = hasher.finalize();
            for (j, byte_pair) in chunk.iter_mut().enumerate() {
                let lo = digest[j * 2] as u16;
                let hi = digest[j * 2 + 1] as u16;
                let raw = ((hi << 8) | lo) as f32;
                *byte_pair = raw / 32_768.0 - 1.0;
            }
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-12);
        v.iter_mut().for_each(|x| *x /= norm);
        v
    }
}

impl Embedder for FakeEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        Ok(Self::hash_to_vector(text))
    }

    fn dim(&self) -> usize {
        FAKE_DIM
    }

    fn model_id(&self) -> &str {
        FAKE_MODEL_ID
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_has_correct_dim() {
        let e = FakeEmbedder;
        let v = e.embed("hello").unwrap();
        assert_eq!(v.len(), FAKE_DIM);
    }

    #[test]
    fn vector_is_l2_normalized() {
        let e = FakeEmbedder;
        let v = e.embed("anything").unwrap();
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "norm = {norm}");
    }

    #[test]
    fn same_text_same_vector() {
        let e = FakeEmbedder;
        let a = e.embed("repeatable").unwrap();
        let b = e.embed("repeatable").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn different_text_different_vector() {
        let e = FakeEmbedder;
        let a = e.embed("alpha").unwrap();
        let b = e.embed("beta").unwrap();
        assert_ne!(a, b);
    }
}
