use thiserror::Error;

#[derive(Debug, Error)]
pub enum LiquidationError {
    #[error("detection error: {0}")]
    DetectionError(String),

    #[error("execution error: {0}")]
    ExecutionError(String),

    #[error(
        "insufficient profit: estimated {estimated_cents} cents, minimum {minimum_cents} cents"
    )]
    InsufficientProfit {
        estimated_cents: i64,
        minimum_cents: i64,
    },

    #[error("chain error: {0}")]
    ChainError(#[from] chain::error::ChainError),

    #[error("order error: {0}")]
    OrderError(String),
}
