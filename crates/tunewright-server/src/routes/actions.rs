use axum::extract::State;
use axum::Json;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tunewright_core::actions::{self, Action, ActionContext};
use tunewright_core::audio;
use tunewright_core::scanner;
use tunewright_core::types::{TagWriteChanges, TunewrightError, WriteResult};

use crate::error::{check_action_batch, join_error, AppError};
use crate::state::AppState;

/// Filter request entries to those resolving to a safe path, as `(id, rel_path, canonical_path)`.
fn safe_file_entries(
    data_root: &std::path::Path,
    files: Vec<ActionFileEntry>,
) -> Vec<(String, String, PathBuf)> {
    files
        .into_iter()
        .filter_map(|f| {
            scanner::resolve_safe_path(data_root, &f.path)
                .ok()
                .map(|safe_path| (f.id, f.path, safe_path))
        })
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
    check_action_batch(body.files.len(), body.actions.len())?;
    let data_root = state.data_root.clone();

    let results = tokio::task::spawn_blocking(move || {
        let regexes = actions::compile_regexes(&body.actions)
            .map_err(TunewrightError::InvalidFormatString)?;
        let valid_files = safe_file_entries(&data_root, body.files);

        // Each file's read → apply → write is independent (per-path locks
        // serialize conflicting writes), so process files in parallel.
        let results: Vec<WriteResult> = valid_files
            .par_iter()
            .enumerate()
            .map(|(i, (id, _rel_path, canonical_path))| {
                let mut tags = match audio::read_tags_fast(canonical_path) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::error!("Action read failed for {}: {e}", canonical_path.display());
                        return WriteResult {
                            id: id.clone(),
                            status: "error".to_string(),
                            error: Some("Failed to read tags".to_string()),
                        };
                    }
                };

                let filename = canonical_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                // Apply all actions in sequence
                let ctx = ActionContext { index: i, filename };
                for action in &body.actions {
                    action.apply(&mut tags, &ctx, &regexes);
                }

                // Write modified tags back
                let changes = TagWriteChanges::from(&tags);
                match audio::write_tags(canonical_path, &changes) {
                    Ok(()) => WriteResult {
                        id: id.clone(),
                        status: "ok".to_string(),
                        error: None,
                    },
                    Err(e) => {
                        tracing::error!(
                            "Action write failed for {}: {e}",
                            canonical_path.display()
                        );
                        WriteResult {
                            id: id.clone(),
                            status: "error".to_string(),
                            error: Some("Failed to write tags".to_string()),
                        }
                    }
                }
            })
            .collect();

        Ok::<_, TunewrightError>(results)
    })
    .await
    .map_err(join_error)??;

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
    check_action_batch(body.files.len(), body.actions.len())?;
    let data_root = state.data_root.clone();

    let previews = tokio::task::spawn_blocking(move || {
        let regexes = actions::compile_regexes(&body.actions)
            .map_err(TunewrightError::InvalidFormatString)?;
        let valid_files = safe_file_entries(&data_root, body.files);

        let mut previews = Vec::new();

        for (i, (id, _rel_path, canonical_path)) in valid_files.iter().enumerate() {
            let filename = canonical_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let stem = canonical_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let original = match audio::read_tags_fast(canonical_path) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let mut modified = original.clone();
            let ctx = ActionContext {
                index: i,
                filename: stem,
            };
            for action in &body.actions {
                action.apply(&mut modified, &ctx, &regexes);
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

        Ok::<_, TunewrightError>(previews)
    })
    .await
    .map_err(join_error)??;

    Ok(Json(PreviewActionsResponse { previews }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compare two TagData and return a list of changed fields.
fn diff_tags(
    a: &tunewright_core::types::TagData,
    b: &tunewright_core::types::TagData,
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
