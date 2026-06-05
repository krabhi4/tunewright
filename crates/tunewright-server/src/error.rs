use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use tunewright_core::types::TunewrightError;

pub struct AppError(pub TunewrightError);

impl From<TunewrightError> for AppError {
    fn from(err: TunewrightError) -> Self {
        Self(err)
    }
}

/// Map a blocking-task join failure (panic or cancellation) into an `AppError`.
pub fn join_error(e: tokio::task::JoinError) -> AppError {
    AppError(TunewrightError::TagReadError(format!(
        "Task join error: {e}"
    )))
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log the detailed error server-side
        tracing::warn!("Request error: {}", self.0);

        // Return sanitized messages to the client (no internal paths)
        let (status, message) = match &self.0 {
            TunewrightError::FileNotFound(_) => (StatusCode::NOT_FOUND, "File not found"),
            TunewrightError::PermissionDenied(_) => (StatusCode::FORBIDDEN, "Permission denied"),
            TunewrightError::PathTraversal(_) => (StatusCode::BAD_REQUEST, "Invalid path"),
            TunewrightError::UnsupportedFormat(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "Unsupported audio format")
            }
            TunewrightError::ImageError(_) => (StatusCode::BAD_REQUEST, "Image processing error"),
            TunewrightError::TagReadError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read tags")
            }
            TunewrightError::TagWriteError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write tags")
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error"),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
