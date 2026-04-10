use thiserror::Error;

#[derive(Debug, Error)]
pub enum TokenUnlockError {
    #[error("scan error: {0}")]
    ScanError(String),

    #[error("trade error: {0}")]
    TradeError(String),

    #[error("insufficient unlock size: {usd_cents} cents, minimum {minimum} cents")]
    InsufficientUnlockSize { usd_cents: i64, minimum: i64 },
}
