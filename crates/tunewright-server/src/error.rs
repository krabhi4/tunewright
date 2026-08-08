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

/// Backstop on per-request item counts. Deliberately generous: axum's 2 MB
/// body limit is the real bound on a list of ids, and the UI legitimately
/// selects every file in a large directory.
pub const MAX_BATCH_ITEMS: usize = 50_000;
/// Actions are quadratic against the file list, so they get their own cap.
pub const MAX_ACTIONS: usize = 100;
/// Ceiling on `files * actions`, the quantity that actually drives the work.
pub const MAX_ACTION_OPERATIONS: usize = 1_000_000;

/// Reject oversized batch requests before any work is scheduled.
pub fn check_batch_size(len: usize) -> Result<(), AppError> {
    if len > MAX_BATCH_ITEMS {
        return Err(AppError(TunewrightError::RequestTooLarge(format!(
            "batch of {len} items exceeds the maximum of {MAX_BATCH_ITEMS}"
        ))));
    }
    Ok(())
}

/// Bound an action request by the work it implies, not by its largest dimension.
pub fn check_action_batch(files: usize, actions: usize) -> Result<(), AppError> {
    check_batch_size(files)?;
    if actions > MAX_ACTIONS {
        return Err(AppError(TunewrightError::RequestTooLarge(format!(
            "{actions} actions exceeds the maximum of {MAX_ACTIONS}"
        ))));
    }
    if files.saturating_mul(actions) > MAX_ACTION_OPERATIONS {
        return Err(AppError(TunewrightError::RequestTooLarge(format!(
            "{files} files x {actions} actions exceeds the maximum of {MAX_ACTION_OPERATIONS} operations"
        ))));
    }
    Ok(())
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
            TunewrightError::InvalidFormatString(_) => {
                (StatusCode::BAD_REQUEST, "Invalid format string or pattern")
            }
            TunewrightError::RequestTooLarge(msg) => (StatusCode::PAYLOAD_TOO_LARGE, msg.as_str()),
            TunewrightError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg.as_str()),
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
