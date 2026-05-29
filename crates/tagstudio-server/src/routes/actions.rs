use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use rayon::prelude::*;
use tagstudio_core::actions::{Action, ActionContext};
use tagstudio_core::audio;
use tagstudio_core::scanner;
use tagstudio_core::types::{TagData, TagWriteChanges, WriteResult};

use crate::error::{join_error, AppError};
use crate::state::AppState;

/// Filter request entries to those resolving to a safe path, as `(id, rel_path)`.
fn safe_file_entries(data_root: &std::path::Path, files: Vec<ActionFileEntry>) -> Vec<(String, String)> {
    files
        .into_iter()
        .filter(|f| scanner::resolve_safe_path(data_root, &f.path).is_ok())
        .map(|f| (f.id, f.path))
        .collect()
}

// ---------------------------------------------------------------------------
// Execute actions on files (stateless — no saved action groups yet)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct ExecuteActionsRequest {
    pub files: Vec<ActionFileEntry>,
    pub actions: Vec<Action>,
}

#[derive(Deserialize)]
pub struct ActionFileEntry {
    pub id: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct ExecuteActionsResponse {
    pub results: Vec<WriteResult>,
}

/// Apply a list of actions to selected files: read tags, apply actions, write back.
pub async fn execute(
    State(state): State<AppState>,
    Json(body): Json<ExecuteActionsRequest>,
) -> Result<Json<ExecuteActionsResponse>, AppError> {
    let data_root = state.data_root.clone();

    let results = tokio::task::spawn_blocking(move || {
        let valid_files = safe_file_entries(&data_root, body.files);

        // Reads are independent, so run them in parallel. Apply + write stays
        // serial (matching audio::batch_write_tags) so write ordering is unchanged.
        let reads: Vec<Result<TagData, String>> = valid_files
            .par_iter()
            .map(|(_, rel_path)| {
                audio::read_tags_fast(&data_root.join(rel_path))
                    .map_err(|e| format!("Read failed: {e}"))
            })
            .collect();

        let mut results = Vec::with_capacity(valid_files.len());

        for (i, ((id, rel_path), read)) in valid_files.iter().zip(reads).enumerate() {
            let mut tags = match read {
                Ok(t) => t,
                Err(e) => {
                    results.push(WriteResult {
                        id: id.clone(),
                        status: "error".to_string(),
                        error: Some(e),
                    });
                    continue;
                }
            };

            let full_path = data_root.join(rel_path);
            let filename = full_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Apply all actions in sequence
            let ctx = ActionContext { index: i, filename };
            for action in &body.actions {
                action.apply(&mut tags, &ctx);
            }

            // Write modified tags back
            let changes = TagWriteChanges::from(&tags);
            match audio::write_tags(&full_path, &changes) {
                Ok(()) => results.push(WriteResult {
                    id: id.clone(),
                    status: "ok".to_string(),
                    error: None,
                }),
                Err(e) => results.push(WriteResult {
                    id: id.clone(),
                    status: "error".to_string(),
                    error: Some(e.to_string()),
                }),
            }
        }

        results
    })
    .await
    .map_err(join_error)?;

    Ok(Json(ExecuteActionsResponse { results }))
}

// ---------------------------------------------------------------------------
// Preview: show what actions would change without writing
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct PreviewActionsResponse {
    pub previews: Vec<ActionPreview>,
}

#[derive(Serialize)]
pub struct ActionPreview {
    pub id: String,
    pub filename: String,
    pub changes: Vec<FieldChange>,
}

#[derive(Serialize)]
pub struct FieldChange {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

pub async fn preview(
    State(state): State<AppState>,
    Json(body): Json<ExecuteActionsRequest>,
) -> Result<Json<PreviewActionsResponse>, AppError> {
    let data_root = state.data_root.clone();

    let previews = tokio::task::spawn_blocking(move || {
        let valid_files = safe_file_entries(&data_root, body.files);

        let mut previews = Vec::new();

        for (i, (id, rel_path)) in valid_files.iter().enumerate() {
            let full_path = data_root.join(rel_path);
            let filename = full_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let stem = full_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let original = match audio::read_tags_fast(&full_path) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let mut modified = original.clone();
            let ctx = ActionContext {
                index: i,
                filename: stem,
            };
            for action in &body.actions {
                action.apply(&mut modified, &ctx);
            }

            // Diff: find changed fields
            let changes = diff_tags(&original, &modified);
            if !changes.is_empty() {
                previews.push(ActionPreview {
                    id: id.clone(),
                    filename,
                    changes,
                });
            }
        }

        previews
    })
    .await
    .map_err(join_error)?;

    Ok(Json(PreviewActionsResponse { previews }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compare two TagData and return a list of changed fields.
fn diff_tags(
    a: &tagstudio_core::types::TagData,
    b: &tagstudio_core::types::TagData,
) -> Vec<FieldChange> {
    let mut changes = Vec::new();

    macro_rules! diff_opt {
        ($field:ident, $name:expr) => {
            let old = a.$field.as_ref().map(|v| v.to_string()).unwrap_or_default();
            let new = b.$field.as_ref().map(|v| v.to_string()).unwrap_or_default();
            if old != new {
                changes.push(FieldChange {
                    field: $name.to_string(),
                    old_value: old,
                    new_value: new,
                });
            }
        };
    }

    diff_opt!(title, "title");
    diff_opt!(artist, "artist");
    diff_opt!(album, "album");
    diff_opt!(album_artist, "album_artist");
    diff_opt!(year, "year");
    diff_opt!(track_number, "track_number");
    diff_opt!(track_total, "track_total");
    diff_opt!(disc_number, "disc_number");
    diff_opt!(disc_total, "disc_total");
    diff_opt!(genre, "genre");
    diff_opt!(comment, "comment");
    diff_opt!(composer, "composer");

    // Diff extra fields
    let all_keys: std::collections::HashSet<&String> =
        a.extra.keys().chain(b.extra.keys()).collect();
    for key in all_keys {
        let old = a.extra.get(key).cloned().unwrap_or_default();
        let new = b.extra.get(key).cloned().unwrap_or_default();
        if old != new {
            changes.push(FieldChange {
                field: key.clone(),
                old_value: old,
                new_value: new,
            });
        }
    }

    changes
}
