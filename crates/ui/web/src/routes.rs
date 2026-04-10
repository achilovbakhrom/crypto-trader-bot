use axum::extract::{Query, State};
use axum::response::IntoResponse;
use axum::Json;
use chrono::Utc;

use crate::error::WebError;
use crate::state::AppState;

/// Query parameters shared by all paginated endpoints.
#[derive(serde::Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

// ---------------------------------------------------------------------------
// GET /api/health
// ---------------------------------------------------------------------------

/// Simple liveness probe.  Returns `{ "status": "ok", "timestamp": "..." }`.
pub async fn health() -> impl IntoResponse {
    let body = serde_json::json!({
        "status": "ok",
        "timestamp": Utc::now().to_rfc3339(),
    });
    Json(body)
}

// ---------------------------------------------------------------------------
// GET /api/metrics
// ---------------------------------------------------------------------------

/// Return an instantaneous snapshot of all bot metrics.
pub async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.metrics.snapshot())
}

// ---------------------------------------------------------------------------
// GET /api/trades?limit=50
// ---------------------------------------------------------------------------

/// Return the most recent trades, newest first.
pub async fn get_trades(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, WebError> {
    let trades =
        storage::queries::get_recent_trades(state.db.pool(), params.limit).await?;
    Ok(Json(trades))
}

// ---------------------------------------------------------------------------
// GET /api/liquidations?limit=50
// ---------------------------------------------------------------------------

/// Return the most recent liquidation attempts, newest first.
pub async fn get_liquidations(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, WebError> {
    let liquidations =
        storage::queries::get_liquidations(state.db.pool(), params.limit).await?;
    Ok(Json(liquidations))
}

// ---------------------------------------------------------------------------
// GET /api/events?limit=100
// ---------------------------------------------------------------------------

/// Return recent system events across all event types, newest first.
pub async fn get_events(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, WebError> {
    let rows =
        storage::queries::get_all_recent_events(state.db.pool(), params.limit).await?;
    Ok(Json(rows))
}
