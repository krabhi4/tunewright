use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use tagstudio_core::types::TagStudioError;

pub struct AppError(pub TagStudioError);

impl From<TagStudioError> for AppError {
    fn from(err: TagStudioError) -> Self {
        Self(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Log the detailed error server-side
        tracing::warn!("Request error: {}", self.0);

        // Return sanitized messages to the client (no internal paths)
        let (status, message) = match &self.0 {
            TagStudioError::FileNotFound(_) => (StatusCode::NOT_FOUND, "File not found"),
            TagStudioError::PermissionDenied(_) => (StatusCode::FORBIDDEN, "Permission denied"),
            TagStudioError::PathTraversal(_) => (StatusCode::BAD_REQUEST, "Invalid path"),
            TagStudioError::UnsupportedFormat(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "Unsupported audio format")
            }
            TagStudioError::ImageError(_) => {
                (StatusCode::BAD_REQUEST, "Image processing error")
            }
            TagStudioError::TagReadError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read tags")
            }
            TagStudioError::TagWriteError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write tags")
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error"),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
