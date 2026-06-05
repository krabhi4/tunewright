use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tunewright_core::rename::{self, RenamePreview, RenameResult};
use tunewright_core::scanner;

use crate::error::{join_error, AppError};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct RenameRequest {
    pub files: Vec<RenameFileEntry>,
    pub format: String,
}

#[derive(Deserialize)]
pub struct RenameFileEntry {
    pub id: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct PreviewResponse {
    pub previews: Vec<RenamePreview>,
}

#[derive(Serialize)]
pub struct ExecuteResponse {
    pub results: Vec<RenameResult>,
}

pub async fn preview(
    State(state): State<AppState>,
    Json(body): Json<RenameRequest>,
) -> Result<Json<PreviewResponse>, AppError> {
    let data_root = state.data_root.clone();
    let format = body.format.clone();

    let previews = tokio::task::spawn_blocking(move || {
        let files: Vec<(String, String, PathBuf)> = body
            .files
            .into_iter()
            .filter_map(|f| {
                scanner::resolve_safe_path(&data_root, &f.path)
                    .ok()
                    .map(|safe_path| (f.id, f.path, safe_path))
            })
            .collect();

        rename::preview_renames(&data_root, &files, &format)
    })
    .await
    .map_err(join_error)??;

    Ok(Json(PreviewResponse { previews }))
}

pub async fn execute(
    State(state): State<AppState>,
    Json(body): Json<RenameRequest>,
) -> Result<Json<ExecuteResponse>, AppError> {
    let data_root = state.data_root.clone();
    let format = body.format.clone();

    let results = tokio::task::spawn_blocking(move || {
        let files: Vec<(String, String, PathBuf)> = body
            .files
            .into_iter()
            .filter_map(|f| {
                scanner::resolve_safe_path(&data_root, &f.path)
                    .ok()
                    .map(|safe_path| (f.id, f.path, safe_path))
            })
            .collect();

        rename::execute_renames(&data_root, &files, &format)
    })
    .await
    .map_err(join_error)?;

    Ok(Json(ExecuteResponse { results }))
}
