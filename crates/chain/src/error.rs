use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Contract error: {0}")]
    ContractError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Provider error: {0}")]
    ProviderError(String),
}
