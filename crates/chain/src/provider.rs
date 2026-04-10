use alloy::providers::{Provider, ProviderBuilder};
use alloy::transports::ws::WsConnect;
use url::Url;

use crate::error::ChainError;

/// Create an HTTP JSON-RPC provider for the given URL.
pub async fn create_http_provider(
    rpc_url: &str,
) -> Result<impl Provider + Clone + 'static, ChainError> {
    let url: Url = rpc_url
        .parse()
        .map_err(|e| ChainError::ParseError(format!("Invalid RPC URL '{}': {}", rpc_url, e)))?;

    let provider = ProviderBuilder::new().connect_http(url);
    Ok(provider)
}

/// Create a WebSocket JSON-RPC provider for the given URL.
pub async fn create_ws_provider(
    ws_url: &str,
) -> Result<impl Provider + Clone + 'static, ChainError> {
    let provider = ProviderBuilder::new()
        .connect_ws(WsConnect::new(ws_url))
        .await
        .map_err(|e| ChainError::ProviderError(format!("WebSocket connection failed: {}", e)))?;
    Ok(provider)
}
