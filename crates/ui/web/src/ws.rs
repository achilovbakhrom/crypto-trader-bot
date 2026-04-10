use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use tokio::sync::broadcast;

use crate::state::{AppState, WsMessage};

/// Axum handler that upgrades an HTTP connection to a WebSocket and begins
/// streaming [`WsMessage`] events to the client.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.ws_tx.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

/// Forward broadcast messages to the WebSocket client as JSON text frames.
///
/// The loop exits cleanly when:
/// - the client disconnects (send returns an error), or
/// - the broadcast channel is closed (the sender was dropped).
async fn handle_socket(mut socket: WebSocket, mut rx: broadcast::Receiver<WsMessage>) {
    loop {
        let msg = match rx.recv().await {
            Ok(m) => m,
            // The channel was closed – no more messages will ever arrive.
            Err(broadcast::error::RecvError::Closed) => break,
            // We fell behind; skip the lost messages and keep going.
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
        };

        let text = match serde_json::to_string(&msg) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialise WsMessage");
                continue;
            }
        };

        if socket.send(Message::Text(text.into())).await.is_err() {
            // Client disconnected.
            break;
        }
    }
}
