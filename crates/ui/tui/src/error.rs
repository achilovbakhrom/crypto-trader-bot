use thiserror::Error;

#[derive(Debug, Error)]
pub enum TuiError {
    #[error("terminal I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("draw error: {0}")]
    DrawError(String),
}
