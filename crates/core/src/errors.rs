use thiserror::Error;

pub type Result<T> = std::result::Result<T, DevcontainerError>;

#[derive(Debug, Error)]
pub enum DevcontainerError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("unsupported feature: {0}")]
    Unsupported(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
