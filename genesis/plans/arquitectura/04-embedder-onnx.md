# ADR-04 — Embedder local con ONNX runtime

**Estado**: Aceptado · 2026-05-09
**Decisión**: Embedder local CPU-only con `ort` (ONNX Runtime Rust bindings) + modelo `all-MiniLM-L6-v2` por default. Tokenizer via crate `tokenizers`. Auto-download del modelo desde Hugging Face en el primer init.

## Contexto

SEELE necesita embedder para:
1. Calcular embedding al guardar (`seele save "..."` → embed body → store en `memories_vec`).
2. Calcular embedding al buscar (`seele search "query"` → embed query → cosine search).

Requisitos:
- **Local-first**: no llamadas a APIs externas (default).
- **CPU-only**: no requerir GPU. La mayoría de las máquinas del User no tienen GPU NVIDIA.
- **Modelo razonablemente pequeño**: ~80-300 MB OK; >1 GB no.
- **Calidad razonable**: top-3 en benchmarks abiertos para English/Multi-lingual de su class.
- **Open weights**: para distribuir sin license issues.

## Stack

### Runtime: `ort` 2.0+

`pykeio/ort` es el wrapper Rust oficial de ONNX Runtime. Multi-OS (Linux/Mac/Windows), CPU EP por default, soporte CUDA/DirectML/CoreML opcional via features.

Pros vs alternativas:
- **`candle`** (Hugging Face): excelente API, but ecosystem younger, performance mixto (a veces más lento que ORT en CPU para modelos pequeños).
- **`tract`** (Sonos): pure-Rust, no DLL deps, muy bueno para ARM. Limitación: support de operadores ONNX más restringido.
- **`burn`**: framework training+inference, demasiado scope para SEELE.

ORT gana por: maturity, performance probada, soporte universal de operadores ONNX, pluggable EPs.

### Modelo: `all-MiniLM-L6-v2`

- **Origen**: Sentence Transformers (UKP Lab + Microsoft), open weights.
- **Tamaño**: ~80 MB ONNX quantized.
- **Dim output**: 384 (cosine-similarity-friendly).
- **Idiomas**: English principalmente, performance decente en multi-lingual.
- **Speed**: ~1-3ms per sentence en CPU mid-range (con batching, ~30-50 sentences/sec).
- **Quality**: STSb Spearman 0.82, MTEB top-50.

**¿Por qué este y no `bge-base` o `nomic-embed-text`?**

- `bge-base-en-v1.5` es 0.85 STSb (mejor) pero 110 MB y 768-dim (más espacio en DB).
- `nomic-embed-text` es 137M params (bigger), high quality, pero 8K context — overkill para body típico de memoria.
- `e5-small-v2` similar a minilm pero menos benchmark coverage en 2026.

minilm es el "good enough default". Soporte multi-lingual via `paraphrase-multilingual-MiniLM-L12-v2` como alternativa configurable (más adelante).

### Tokenizer: crate `tokenizers` (HuggingFace)

`tokenizers` 0.20+ es el binding Rust oficial. Soporta WordPiece (que usa MiniLM) y BPE.

Carga del tokenizer config (`tokenizer.json`) shipped junto con el modelo en `~/.seele/embedder/`.

### Auto-download

En el primer `seele init` o `seele embedder install`, el binary descarga:

```
~/.seele/embedder/all-MiniLM-L6-v2/
├── model.onnx                    # ~80 MB
├── tokenizer.json                # ~700 KB
├── config.json                   # metadata
└── README.md                     # licencia MIT del modelo
```

Origen: `https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2` via crate `hf-hub`.

Verificación SHA256 hardcodeada en SEELE para detectar tampering.

### API interna

```rust
pub struct Embedder {
    session: ort::Session,
    tokenizer: tokenizers::Tokenizer,
    dim: usize,
}

impl Embedder {
    pub fn load(model_dir: &Path) -> Result<Self> { ... }

    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = self.tokenizer.encode(text, true)?;
        // ...run ORT inference...
        // mean-pool over token embeddings (estándar para sentence-transformers)
        // L2-normalize
        Ok(vec)
    }

    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> { ... }
}
```

Mean-pooling + L2 normalize después del forward pass (es lo que sentence-transformers hace por convención).

### Singleton runtime

ORT runtime initialization es lenta (~50-200ms). El binary mantiene una `Embedder` instance via `OnceCell<Mutex<Embedder>>` global o explicitly passed-down.

## Decisiones secundarias

### ONNX vs ONNX-quantized

Modelo en INT8 quantization es ~25 MB y ~30% más rápido, con ~1-2% drop en quality. Default: usar el quantized (`model_quantized.onnx`) para reducir disk + faster inference. Caller puede forzar full-precision con `--no-quantize`.

### Token limit

MiniLM context = 512 tokens. Si el body > 512 tokens, options:
- **Truncate**: cortar en 512 tokens (default v0.1, simple).
- **Mean-of-chunks**: split en chunks de 512, embeddear cada uno, promediar (v0.2 si emerge demanda).
- **Reject**: error y delegar al caller (descartado, mala UX).

### Embedder swap

API permite cargar otros modelos:

```bash
seele embedder install paraphrase-multilingual-MiniLM-L12-v2
seele config set embedder.model paraphrase-multilingual-MiniLM-L12-v2
```

Con caveat: cambiar de modelo invalida embeddings existentes. SEELE lo detecta por comparing model hash y warning + opción de re-embed all (`seele embedder reembed-all`).

### CUDA / Metal

Default: CPU EP. Si el user tiene GPU NVIDIA y quiere acelerar, habilitar via feature flag:

```bash
cargo install seele --features cuda
```

Para v0.1: solo CPU. CUDA/Metal/DirectML para v0.2.

## Manejo de errores

```rust
#[derive(Debug, thiserror::Error)]
pub enum EmbedderError {
    #[error("model directory not found: {0}")]
    ModelNotFound(PathBuf),

    #[error("ONNX runtime init failed: {0}")]
    OrtInit(#[from] ort::Error),

    #[error("tokenizer load failed: {0}")]
    TokenizerLoad(String),

    #[error("inference failed: {0}")]
    Inference(String),

    #[error("text too long: {chars} chars (max ~2000 chars / 512 tokens)")]
    TextTooLong { chars: usize },
}
```

CLI muestra el error con suggested fix (`seele embedder install all-MiniLM-L6-v2`).

## Performance esperada

En Ryzen 5 5600 / M2 / similar:

- Init runtime + load model: ~150-300ms (one-time).
- `embed()` single sentence: ~2-5ms.
- `embed_batch(32)`: ~15-30ms total (~0.5-1ms per).

Memory footprint: ~250 MB RAM con modelo cargado.

Para queries via HTTP server: el embedder se mantiene cargado en memoria (singleton). Search típica = embed(query) + sql_search ≈ 5-200ms total (depende del search).

## Consecuencias

### Positivas
- Local-first total. Cero llamadas externas en runtime.
- Quality razonable para casos típicos.
- Multi-OS funciona out-of-the-box.

### Negativas
- ~250 MB RAM mientras el server corre.
- Modelo descargado ocupa ~80 MB en disk (~25 MB con quantization).
- Quality limitada vs modelos más grandes (BGE, E5). Si MNEMA encuentra el ceiling, swap a otro modelo.

### Mitigaciones
- Lazy loading: si solo se usa CLI (sin server), embedder se carga on-demand.
- Quantization activa por default reduce overhead.

## Referencias

- Sentence-Transformers `all-MiniLM-L6-v2`: https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2
- ORT crate docs: https://github.com/pykeio/ort
- MTEB benchmark leaderboard 2026.
- Hugging Face Hub Rust client: https://github.com/huggingface/hf-hub
