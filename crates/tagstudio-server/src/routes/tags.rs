use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tagstudio_core::audio;
use tagstudio_core::scanner;
use tagstudio_core::types::{TagData, TagWriteChanges, WriteResult};

use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ReadTagsRequest {
    pub ids: Vec<String>,
    /// Map of id -> relative_path (provided by frontend)
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
    // Validate all paths are safe
    let mut valid_paths: Vec<String> = Vec::new();
    let mut id_to_path: HashMap<String, String> = HashMap::new();

    for id in &body.ids {
        if let Some(rel_path) = body.paths.get(id) {
            // Validate path is safe (within data root)
            match scanner::resolve_safe_path(&state.data_root, rel_path) {
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

    // Batch read tags
    let path_tags = audio::batch_read_tags(&state.data_root, &valid_paths);

    // Map back to IDs
    let mut result: HashMap<String, TagData> = HashMap::new();
    for (id, rel_path) in &id_to_path {
        if let Some(tags) = path_tags.get(rel_path) {
            result.insert(id.clone(), tags.clone());
        }
    }

    Ok(Json(ReadTagsResponse { tags: result }))
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
    let mut changes_vec: Vec<(String, String, TagWriteChanges)> = Vec::new();

    for entry in body.changes {
        match scanner::resolve_safe_path(&state.data_root, &entry.path) {
            Ok(_) => {
                changes_vec.push((entry.id, entry.path, entry.tags));
            }
            Err(e) => {
                tracing::warn!("Unsafe path rejected for write: {} - {}", entry.path, e);
            }
        }
    }

    let results = audio::batch_write_tags(&state.data_root, &changes_vec);
    Ok(Json(WriteTagsResponse { results }))
}
