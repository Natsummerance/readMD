use thiserror::Error;

pub type HostResult<T> = Result<T, HostError>;

#[derive(Debug, Error)]
pub enum HostError {
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("bridge I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON protocol failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("window backend failed: {0}")]
    Backend(String),
    #[error("WebView failed: {0}")]
    WebView(String),
    #[error("asset path is outside the runtime directory")]
    UnsafeAssetPath,
    #[error("unsupported renderer: {0}")]
    UnsupportedRenderer(String),
}
