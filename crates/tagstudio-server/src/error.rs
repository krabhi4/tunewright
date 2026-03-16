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
        let (status, message) = match &self.0 {
            TagStudioError::FileNotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            TagStudioError::PermissionDenied(_) => (StatusCode::FORBIDDEN, self.0.to_string()),
            TagStudioError::PathTraversal(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            TagStudioError::UnsupportedFormat(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, self.0.to_string())
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
