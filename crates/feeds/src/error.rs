use thiserror::Error;

#[derive(Debug, Error)]
pub enum FeedsError {
    #[error("WebSocket error: {0}")]
    WebSocketError(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Disconnected")]
    Disconnected,
}
