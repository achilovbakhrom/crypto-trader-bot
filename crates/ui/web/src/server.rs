use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

use crate::routes::{get_events, get_liquidations, get_metrics, get_trades, health};
use crate::state::AppState;
use crate::ws::ws_handler;

/// HTTP / WebSocket server for the React admin panel.
pub struct WebServer {
    state: AppState,
    host: String,
    port: u16,
}

impl WebServer {
    /// Create a new server that will bind to `host:port`.
    pub fn new(state: AppState, host: String, port: u16) -> Self {
        Self { state, host, port }
    }

    /// Build the Axum router, bind to the configured address, and serve
    /// requests until the process is terminated.
    pub async fn run(self) -> anyhow::Result<()> {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = Router::new()
            .route("/api/health", get(health))
            .route("/api/metrics", get(get_metrics))
            .route("/api/trades", get(get_trades))
            .route("/api/liquidations", get(get_liquidations))
            .route("/api/events", get(get_events))
            .route("/ws", get(ws_handler))
            .with_state(self.state)
            .layer(cors);

        let addr = format!("{}:{}", self.host, self.port);
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!(address = %addr, "web-ui server listening");

        axum::serve(listener, app).await?;

        Ok(())
    }
}
