use thiserror::Error;

#[derive(Error, Debug)]
pub enum CawError {
    #[error("plugin error: {0}")]
    Plugin(String),

    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
