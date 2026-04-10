use std::sync::Arc;
use tokio::sync::broadcast;

use monitor::metrics::MetricsSnapshot;

/// Shared state injected into every Axum handler.
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<storage::db::Database>,
    pub metrics: Arc<monitor::metrics::Metrics>,
    /// Broadcast channel for pushing real-time updates to WebSocket clients.
    pub ws_tx: broadcast::Sender<WsMessage>,
}

/// Messages that can be broadcast to connected WebSocket clients.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum WsMessage {
    MetricsUpdate(MetricsSnapshot),
    LogEvent {
        level: String,
        message: String,
        timestamp: String,
    },
    LiquidationExecuted {
        profit_cents: i64,
        borrower: String,
    },
}

impl AppState {
    /// Construct a new [`AppState`].
    ///
    /// A broadcast channel with capacity 256 is created internally; senders
    /// are exposed via [`AppState::ws_tx`] and receivers via
    /// [`broadcast::Sender::subscribe`].
    pub fn new(db: Arc<storage::db::Database>, metrics: Arc<monitor::metrics::Metrics>) -> Self {
        let (ws_tx, _) = broadcast::channel(256);
        Self { db, metrics, ws_tx }
    }

    /// Send a [`WsMessage`] to all currently subscribed WebSocket clients.
    ///
    /// Errors are silently dropped – it is normal for there to be no active
    /// subscribers, and a lagged receiver is simply skipped by the broadcast
    /// channel.
    pub fn broadcast(&self, msg: WsMessage) {
        // SendError means no receivers are connected; that is fine.
        let _ = self.ws_tx.send(msg);
    }
}
