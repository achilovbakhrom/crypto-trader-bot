use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebError {
    #[error("server error: {0}")]
    ServerError(String),

    #[error("storage error: {0}")]
    DbError(#[from] storage::error::StorageError),
}

impl axum::response::IntoResponse for WebError {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({ "error": self.to_string() });
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            axum::Json(body),
        )
            .into_response()
    }
}
