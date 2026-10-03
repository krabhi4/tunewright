use axum::extract::State;
use axum::Json;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tunewright_core::actions::{self, Action, ActionContext};
use tunewright_core::audio;
use tunewright_core::scanner;
use tunewright_core::types::{TunewrightError, WriteResult};

use crate::error::{check_action_batch, join_error, AppError, MAX_PREVIEW_BYTES};
use crate::state::AppState;

/// Split request entries into those resolving to a safe path, as
/// `(id, rel_path, canonical_path)` with each file kept once, and the ids of
/// those that do not.
fn safe_file_entries(
    data_root: &std::path::Path,
    files: Vec<ActionFileEntry>,
) -> (Vec<(String, String, PathBuf)>, Vec<String>) {
    let mut valid = Vec::new();
    let mut rejected = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for f in files {
        match scanner::resolve_safe_path(data_root, &f.path) {
            Ok(safe_path) if !seen.insert(safe_path.clone()) => {}
            Ok(safe_path) => valid.push((f.id, f.path, safe_path)),
            Err(_) => rejected.push(f.id),
        }
    }
    (valid, rejected)
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
    check_action_batch(body.files.len(), &body.actions)?;
    let data_root = state.data_root.clone();

    let results = tokio::task::spawn_blocking(move || {
        let regexes = actions::compile_regexes(&body.actions)
            .map_err(TunewrightError::InvalidFormatString)?;
        let (valid_files, rejected) = safe_file_entries(&data_root, body.files);

        // Each file's read → apply → write runs under that file's lock and
        // writes back only the fields the actions changed, so process files
        // in parallel.
        let mut results: Vec<WriteResult> = valid_files
            .par_iter()
            .enumerate()
            .map(|(i, (id, _rel_path, canonical_path))| {
                let filename = canonical_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let ctx = ActionContext { index: i, filename };

                let mut within_budget = true;
                match audio::modify_tags(canonical_path, |tags| {
                    let original = tags.clone();
                    within_budget = actions::apply_all(&body.actions, tags, &ctx, &regexes);
                    if !within_budget {
                        *tags = original;
                    }
                }) {
                    Ok(()) if !within_budget => WriteResult {
                        id: id.clone(),
                        status: "error".to_string(),
                        error: Some("Actions grow the tags past the size limit".to_string()),
                    },
                    Ok(()) => WriteResult {
                        id: id.clone(),
                        status: "ok".to_string(),
                        error: None,
                    },
                    Err(e) => {
                        tracing::error!("Action failed for {}: {e}", canonical_path.display());
                        let error = match e {
                            TunewrightError::TagReadError(_) => "Failed to read tags",
                            _ => "Failed to write tags",
                        };
                        WriteResult {
                            id: id.clone(),
                            status: "error".to_string(),
                            error: Some(error.to_string()),
                        }
                    }
                }
            })
            .collect();
        results.extend(rejected.into_iter().map(|id| WriteResult {
            id,
            status: "error".to_string(),
            error: Some("File not found".to_string()),
        }));

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
    check_action_batch(body.files.len(), &body.actions)?;
    let data_root = state.data_root.clone();

    let previews = tokio::task::spawn_blocking(move || {
        let regexes = actions::compile_regexes(&body.actions)
            .map_err(TunewrightError::InvalidFormatString)?;
        let (valid_files, _) = safe_file_entries(&data_root, body.files);

        let mut previews = Vec::new();
        let mut preview_bytes = 0usize;

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

            let original = match audio::read_tags_fast_with(canonical_path, true) {
                Ok(t) => t,
                Err(_) => continue,
            };

            let mut modified = original.clone();
            let ctx = ActionContext {
                index: i,
                filename: stem,
            };
            if !actions::apply_all(&body.actions, &mut modified, &ctx, &regexes) {
                continue;
            }

            // Diff: find changed fields
            let changes = diff_tags(&original, &modified);
            preview_bytes += changes
                .iter()
                .map(|c| c.field.len() + c.old_value.len() + c.new_value.len())
                .sum::<usize>();
            if preview_bytes > MAX_PREVIEW_BYTES {
                return Err(TunewrightError::RequestTooLarge(
                    "preview too large, select fewer files".to_string(),
                ));
            }
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

const MAX_PREVIEW_VALUE_BYTES: usize = 1024;

fn preview_value(mut value: String) -> String {
    if value.len() > MAX_PREVIEW_VALUE_BYTES {
        let mut end = MAX_PREVIEW_VALUE_BYTES;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        value.truncate(end);
        value.push('…');
    }
    value
}

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
                    old_value: preview_value(old),
                    new_value: preview_value(new),
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
                old_value: preview_value(old),
                new_value: preview_value(new),
            });
        }
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_value_truncates_on_char_boundary() {
        assert_eq!(preview_value("short".into()), "short");
        let long = "é".repeat(MAX_PREVIEW_VALUE_BYTES);
        let out = preview_value(long);
        assert!(out.ends_with('…'));
        assert!(out.len() <= MAX_PREVIEW_VALUE_BYTES + '…'.len_utf8());
        assert!(out.trim_end_matches('…').chars().all(|c| c == 'é'));
    }

    #[test]
    fn each_file_is_listed_once() {
        let dir =
            std::env::temp_dir().join(format!("tunewright_actions_once_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        std::fs::write(dir.join("a.flac"), b"x").unwrap();
        let entry = |id: &str, path: &str| ActionFileEntry {
            id: id.to_string(),
            path: path.to_string(),
        };
        let (valid, rejected) = safe_file_entries(
            &dir,
            vec![
                entry("a", "a.flac"),
                entry("a", "a.flac"),
                entry("b", "./a.flac"),
                entry("c", "gone.flac"),
            ],
        );
        assert_eq!(valid.len(), 1);
        assert_eq!(rejected, vec!["c".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
