use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tunewright_core::audio;
use tunewright_core::scanner;
use tunewright_core::types::{TagData, TagWriteChanges, WriteResult};

use crate::error::{join_error, AppError};
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

type BatchReadFn = fn(&[(String, PathBuf)]) -> HashMap<String, TagData>;

/// Shared body for the tag-read endpoints: resolve the requested ids to safe
/// paths, batch-read them with `batch_read`, then map results back by id.
async fn read_with(
    state: AppState,
    body: ReadTagsRequest,
    batch_read: BatchReadFn,
) -> Result<Json<ReadTagsResponse>, AppError> {
    let data_root = state.data_root.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut valid_paths: Vec<(String, PathBuf)> = Vec::new();
        let mut id_to_path: HashMap<String, String> = HashMap::new();

        for id in &body.ids {
            if let Some(rel_path) = body.paths.get(id) {
                match scanner::resolve_safe_path(&data_root, rel_path) {
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
    let data_root = state.data_root.clone();

    let results = tokio::task::spawn_blocking(move || {
        let mut changes_vec: Vec<(String, PathBuf, TagWriteChanges)> = Vec::new();

        for entry in body.changes {
            match scanner::resolve_safe_path(&data_root, &entry.path) {
                Ok(safe_path) => {
                    changes_vec.push((entry.id, safe_path, entry.tags));
                }
                Err(e) => {
                    tracing::warn!("Unsafe path rejected for write: {} - {}", entry.path, e);
                }
            }
        }

        audio::batch_write_tags(&changes_vec)
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
