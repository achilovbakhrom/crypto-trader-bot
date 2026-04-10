use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("Binance API error {code}: {msg}")]
    ApiError { code: i32, msg: String },

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Signature error: {0}")]
    SignatureError(String),

    #[error("Insufficient balance for {asset}: needed {needed}, available {available}")]
    InsufficientBalance {
        asset: String,
        needed: String,
        available: String,
    },

    #[error("Rate limited by exchange")]
    RateLimited,
}

impl From<reqwest::Error> for ExecutionError {
    fn from(e: reqwest::Error) -> Self {
        ExecutionError::HttpError(e.to_string())
    }
}

impl From<serde_json::Error> for ExecutionError {
    fn from(e: serde_json::Error) -> Self {
        ExecutionError::HttpError(format!("JSON parse error: {e}"))
    }
}
