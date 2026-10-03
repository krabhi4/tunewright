use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tunewright_core::audio;
use tunewright_core::scanner;
use tunewright_core::types::{TagData, TagWriteChanges, TunewrightError, WriteResult};

use crate::error::{check_batch_size, join_error, AppError};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ReadTagsRequest {
    pub ids: Vec<String>,
    pub paths: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct ReadTagsResponse {
    pub tags: HashMap<String, TagData>,
}

const MAX_EXTRA_KEYS: usize = 1024;

type BatchReadFn = fn(&[(String, PathBuf)]) -> HashMap<String, TagData>;

/// Shared body for the tag-read endpoints: resolve the requested ids to safe
/// paths, batch-read them with `batch_read`, then map results back by id.
async fn read_with(
    state: AppState,
    body: ReadTagsRequest,
    batch_read: BatchReadFn,
) -> Result<Json<ReadTagsResponse>, AppError> {
    check_batch_size(body.ids.len().max(body.paths.len()))?;
    let data_root = state.data_root.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut valid_paths: Vec<(String, PathBuf)> = Vec::new();
        let mut id_to_path: HashMap<String, String> = HashMap::new();
        let mut seen = std::collections::HashSet::new();

        for id in &body.ids {
            if let Some(rel_path) = body.paths.get(id) {
                match scanner::resolve_safe_path(&data_root, rel_path) {
                    Ok(safe_path) if !seen.insert(safe_path.clone()) => {}
                    Ok(safe_path) => {
                        valid_paths.push((rel_path.clone(), safe_path));
                        id_to_path.insert(id.clone(), rel_path.clone());
                    }
                    Err(e) => {
                        tracing::warn!("Unsafe path rejected: {} - {}", rel_path, e);
                    }
                }
            }
        }

        let path_tags = batch_read(&valid_paths);

        let mut result: HashMap<String, TagData> = HashMap::new();
        for (id, rel_path) in &id_to_path {
            if let Some(tags) = path_tags.get(rel_path) {
                result.insert(id.clone(), tags.clone());
            }
        }

        ReadTagsResponse { tags: result }
    })
    .await
    .map_err(join_error)?;

    Ok(Json(result))
}

pub async fn read_tags(
    State(state): State<AppState>,
    Json(body): Json<ReadTagsRequest>,
) -> Result<Json<ReadTagsResponse>, AppError> {
    read_with(state, body, audio::batch_read_tags).await
}

#[derive(Deserialize)]
pub struct WriteTagsRequest {
    pub changes: Vec<WriteTagsEntry>,
}

#[derive(Deserialize)]
pub struct WriteTagsEntry {
    pub id: String,
    pub path: String,
    pub tags: TagWriteChanges,
}

#[derive(Serialize)]
pub struct WriteTagsResponse {
    pub results: Vec<WriteResult>,
}

pub async fn write_tags(
    State(state): State<AppState>,
    Json(body): Json<WriteTagsRequest>,
) -> Result<Json<WriteTagsResponse>, AppError> {
    check_batch_size(body.changes.len())?;
    if let Some(n) = body
        .changes
        .iter()
        .filter_map(|c| c.tags.extra.as_ref().map(|e| e.len()))
        .find(|&n| n > MAX_EXTRA_KEYS)
    {
        return Err(AppError(TunewrightError::RequestTooLarge(format!(
            "{n} custom fields exceed the maximum of {MAX_EXTRA_KEYS} per file"
        ))));
    }
    let data_root = state.data_root.clone();

    let results = tokio::task::spawn_blocking(move || {
        let mut changes_vec: Vec<(String, PathBuf, TagWriteChanges)> = Vec::new();
        let mut rejected = Vec::new();

        for entry in body.changes {
            match scanner::resolve_safe_path(&data_root, &entry.path) {
                Ok(safe_path) => {
                    changes_vec.push((entry.id, safe_path, entry.tags));
                }
                Err(e) => {
                    tracing::warn!("Unsafe path rejected for write: {} - {}", entry.path, e);
                    rejected.push(WriteResult {
                        id: entry.id,
                        status: "error".to_string(),
                        error: Some("File not found".to_string()),
                    });
                }
            }
        }

        let mut results = audio::batch_write_tags(&changes_vec);
        results.extend(rejected);
        results
    })
    .await
    .map_err(join_error)?;

    Ok(Json(WriteTagsResponse { results }))
}

/// Read full audio properties (duration, bitrate, sample rate) for files.
/// Slower than read_tags — only call for files the user wants to inspect.
pub async fn read_properties(
    State(state): State<AppState>,
    Json(body): Json<ReadTagsRequest>,
) -> Result<Json<ReadTagsResponse>, AppError> {
    read_with(state, body, audio::batch_read_tags_full).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn each_file_is_read_once_per_request() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static READS: AtomicUsize = AtomicUsize::new(0);
        fn count(paths: &[(String, PathBuf)]) -> HashMap<String, TagData> {
            READS.fetch_add(paths.len(), Ordering::Relaxed);
            HashMap::new()
        }

        let dir = std::env::temp_dir().join(format!("tunewright_read_once_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        std::fs::write(dir.join("a.flac"), b"x").unwrap();
        let config = crate::config::Config {
            data_dir: dir.clone(),
            state_dir: None,
            static_dir: dir.clone(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            trust_proxy: false,
            setup_token: None,
        };
        let state = AppState::new(
            config,
            crate::users::UserManager::load(dir.join("users.json")),
        );
        let body = ReadTagsRequest {
            ids: vec!["a".into(), "a".into(), "b".into(), "c".into()],
            paths: HashMap::from([
                ("a".into(), "a.flac".into()),
                ("b".into(), "/a.flac".into()),
                ("c".into(), "./a.flac".into()),
            ]),
        };
        assert!(read_with(state, body, count).await.is_ok());
        assert_eq!(READS.load(Ordering::Relaxed), 1);

        let extra: HashMap<String, Option<String>> = (0..=MAX_EXTRA_KEYS)
            .map(|i| (format!("K{i}"), None))
            .collect();
        let body = WriteTagsRequest {
            changes: vec![WriteTagsEntry {
                id: "a".into(),
                path: "a.flac".into(),
                tags: TagWriteChanges {
                    extra: Some(extra),
                    ..Default::default()
                },
            }],
        };
        let state = AppState::new(
            crate::config::Config {
                data_dir: dir.clone(),
                state_dir: None,
                static_dir: dir.clone(),
                port: 8080,
                host: "127.0.0.1".to_string(),
                cookie_secure: false,
                trust_proxy: false,
                setup_token: None,
            },
            crate::users::UserManager::load(dir.join("users.json")),
        );
        assert!(write_tags(State(state), Json(body)).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
