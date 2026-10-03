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

/// Backstop on per-request item counts. Deliberately generous: the batch
/// routes' 32 MiB body limit is the real bound on a list of ids, and the UI
/// legitimately selects every file in a large directory.
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

pub const MAX_FORMAT_BYTES: usize = 4 * 1024;
pub const MAX_FIELD_NAME_BYTES: usize = 256;
pub const MAX_ACTION_FIELDS: usize = 64;
pub const MAX_ACTIONS_JSON_BYTES: usize = 64 * 1024;
pub const MAX_PREVIEW_BYTES: usize = 16 * 1024 * 1024;

pub fn check_format_len(format: &str) -> Result<(), AppError> {
    if format.len() > MAX_FORMAT_BYTES {
        return Err(AppError(TunewrightError::RequestTooLarge(format!(
            "format of {} bytes exceeds the maximum of {MAX_FORMAT_BYTES}",
            format.len()
        ))));
    }
    Ok(())
}

/// Bound an action request by the work it implies, not by its largest dimension.
pub fn check_action_batch(
    files: usize,
    actions: &[tunewright_core::actions::Action],
) -> Result<(), AppError> {
    use tunewright_core::actions::Action;
    check_batch_size(files)?;
    let actions_bytes = serde_json::to_vec(actions).map_or(usize::MAX, |v| v.len());
    if actions_bytes > MAX_ACTIONS_JSON_BYTES {
        return Err(AppError(TunewrightError::RequestTooLarge(format!(
            "actions of {actions_bytes} bytes exceed the maximum of {MAX_ACTIONS_JSON_BYTES}"
        ))));
    }
    for action in actions {
        let names: Vec<&String> = match action {
            Action::CaseConversion { field, .. }
            | Action::Replace { field, .. }
            | Action::SetField { field, .. }
            | Action::RemoveField { field }
            | Action::AutoNumber { field, .. }
            | Action::TrimField { field } => vec![field],
            Action::FormatValue { field, format } => {
                check_format_len(format)?;
                vec![field]
            }
            Action::RemoveAllExcept { fields } => fields.iter().collect(),
            Action::SplitField { source, target, .. } => vec![source, target],
            Action::MergeFields {
                sources, target, ..
            } => sources.iter().chain([target]).collect(),
        };
        if names.len() > MAX_ACTION_FIELDS {
            return Err(AppError(TunewrightError::RequestTooLarge(format!(
                "{} fields in one action exceeds the maximum of {MAX_ACTION_FIELDS}",
                names.len()
            ))));
        }
        if let Some(name) = names.into_iter().find(|n| n.len() > MAX_FIELD_NAME_BYTES) {
            return Err(AppError(TunewrightError::RequestTooLarge(format!(
                "field name of {} bytes exceeds the maximum of {MAX_FIELD_NAME_BYTES}",
                name.len()
            ))));
        }
    }
    let actions = actions.len();
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
                (StatusCode::UNPROCESSABLE_ENTITY, "Failed to read tags")
            }
            TunewrightError::Upstream(_) => (StatusCode::BAD_GATEWAY, "Upstream request failed"),
            TunewrightError::TagWriteError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write tags")
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error"),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tunewright_core::actions::Action;

    #[test]
    fn action_field_names_and_formats_are_bounded() {
        let set = |field: String| Action::SetField {
            field,
            value: "x".to_string(),
        };
        assert!(check_action_batch(1, &[set("comment".to_string())]).is_ok());
        let big = Action::SetField {
            field: "comment".to_string(),
            value: "x".repeat(MAX_ACTIONS_JSON_BYTES),
        };
        assert!(check_action_batch(1, &[big]).is_err());
        assert!(check_action_batch(1, &[set("k".repeat(MAX_FIELD_NAME_BYTES + 1))]).is_err());
        let keep = Action::RemoveAllExcept {
            fields: vec!["a".to_string(); MAX_ACTION_FIELDS + 1],
        };
        assert!(check_action_batch(1, &[keep]).is_err());
        let merge = Action::MergeFields {
            sources: vec!["k".repeat(MAX_FIELD_NAME_BYTES + 1)],
            separator: String::new(),
            target: "title".to_string(),
        };
        assert!(check_action_batch(1, &[merge]).is_err());
        let format = Action::FormatValue {
            field: "title".to_string(),
            format: "x".repeat(MAX_FORMAT_BYTES + 1),
        };
        assert!(check_action_batch(1, &[format]).is_err());
    }
}
