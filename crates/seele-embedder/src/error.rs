use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbedderError {
    #[error("hf-hub error: {0}")]
    HfHub(String),

    #[error("tokenizer error: {0}")]
    Tokenizer(String),

    #[error("ort error: {0}")]
    Ort(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("empty input: {0}")]
    EmptyInput(&'static str),

    #[error("cannot resolve embedder cache dir; set SEELE_EMBEDDER_DIR or check that dirs::cache_dir() returns Some")]
    CacheDirUnresolvable,

    #[error("sha256 mismatch for {file}: expected {expected}, got {got}. Delete the cache file and retry to redownload")]
    HashMismatch {
        file: String,
        expected: String,
        got: String,
    },

    #[error("global embedder already initialized")]
    GlobalAlreadyInitialized,
}

pub type Result<T> = std::result::Result<T, EmbedderError>;

impl From<hf_hub::api::sync::ApiError> for EmbedderError {
    fn from(e: hf_hub::api::sync::ApiError) -> Self {
        Self::HfHub(e.to_string())
    }
}

impl From<tokenizers::tokenizer::Error> for EmbedderError {
    fn from(e: tokenizers::tokenizer::Error) -> Self {
        Self::Tokenizer(e.to_string())
    }
}

impl From<ort::Error> for EmbedderError {
    fn from(e: ort::Error) -> Self {
        Self::Ort(e.to_string())
    }
}
