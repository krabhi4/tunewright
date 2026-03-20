use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tagstudio_core::audio;
use tagstudio_core::scanner;
use tagstudio_core::types::{TagData, TagStudioError, TagWriteChanges, WriteResult};

use crate::error::AppError;
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

pub async fn read_tags(
    State(state): State<AppState>,
    Json(body): Json<ReadTagsRequest>,
) -> Result<Json<ReadTagsResponse>, AppError> {
    let data_root = state.data_root.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut valid_paths: Vec<String> = Vec::new();
        let mut id_to_path: HashMap<String, String> = HashMap::new();

        for id in &body.ids {
            if let Some(rel_path) = body.paths.get(id) {
                match scanner::resolve_safe_path(&data_root, rel_path) {
                    Ok(_) => {
                        valid_paths.push(rel_path.clone());
                        id_to_path.insert(id.clone(), rel_path.clone());
                    }
                    Err(e) => {
                        tracing::warn!("Unsafe path rejected: {} - {}", rel_path, e);
                    }
                }
            }
        }

        let path_tags = audio::batch_read_tags(&data_root, &valid_paths);

        let mut result: HashMap<String, TagData> = HashMap::new();
        for (id, rel_path) in &id_to_path {
            if let Some(tags) = path_tags.get(rel_path) {
                result.insert(id.clone(), tags.clone());
            }
        }

        ReadTagsResponse { tags: result }
    })
    .await
    .map_err(|e| AppError(TagStudioError::TagReadError(format!("Task join error: {}", e))))?;

    Ok(Json(result))
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
        let mut changes_vec: Vec<(String, String, TagWriteChanges)> = Vec::new();

        for entry in body.changes {
            match scanner::resolve_safe_path(&data_root, &entry.path) {
                Ok(_) => {
                    changes_vec.push((entry.id, entry.path, entry.tags));
                }
                Err(e) => {
                    tracing::warn!("Unsafe path rejected for write: {} - {}", entry.path, e);
                }
            }
        }

        audio::batch_write_tags(&data_root, &changes_vec)
    })
    .await
    .map_err(|e| AppError(TagStudioError::TagReadError(format!("Task join error: {}", e))))?;

    Ok(Json(WriteTagsResponse { results }))
}

/// Read full audio properties (duration, bitrate, sample rate) for files.
/// Slower than read_tags — only call for files the user wants to inspect.
pub async fn read_properties(
    State(state): State<AppState>,
    Json(body): Json<ReadTagsRequest>,
) -> Result<Json<ReadTagsResponse>, AppError> {
    let data_root = state.data_root.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut valid_paths: Vec<String> = Vec::new();
        let mut id_to_path: HashMap<String, String> = HashMap::new();

        for id in &body.ids {
            if let Some(rel_path) = body.paths.get(id) {
                if scanner::resolve_safe_path(&data_root, rel_path).is_ok() {
                    valid_paths.push(rel_path.clone());
                    id_to_path.insert(id.clone(), rel_path.clone());
                }
            }
        }

        let path_tags = audio::batch_read_tags_full(&data_root, &valid_paths);

        let mut result: HashMap<String, TagData> = HashMap::new();
        for (id, rel_path) in &id_to_path {
            if let Some(tags) = path_tags.get(rel_path) {
                result.insert(id.clone(), tags.clone());
            }
        }

        ReadTagsResponse { tags: result }
    })
    .await
    .map_err(|e| AppError(TagStudioError::TagReadError(format!("Task join error: {}", e))))?;

    Ok(Json(result))
}
