use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid decimal value for field `{field}`: {value}")]
    InvalidDecimal { field: String, value: String },

    #[error("negative value not allowed for field `{field}`: {value}")]
    NegativeValue { field: String, value: String },

    #[error("invalid timestamp `{0}` (expected RFC3339 with Z suffix)")]
    InvalidTimestamp(String),

    #[error("unknown event type `{0}`")]
    UnknownEventType(String),

    #[error("invalid event: {0}")]
    InvalidEvent(String),

    #[error("invalid canonical record: {0}")]
    InvalidCanonical(String),

    #[error("io error on `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("io error: {0}")]
    IoBare(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("toml deserialize error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("signature error: {0}")]
    Signature(String),

    #[error("verification failed: {0}")]
    Verification(String),

    #[error("bundle error: {0}")]
    Bundle(String),

    #[error("anchor error: {0}")]
    Anchor(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("refusing demo key in production mode (use --allow-demo-key to override)")]
    DemoKeyInProduction,

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn io<P: Into<PathBuf>>(path: P, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
