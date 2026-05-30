use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use tunewright_core::rename::{self, RenamePreview, RenameResult};
use tunewright_core::scanner;

use crate::error::AppError;
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
    let files: Vec<(String, String)> = body
        .files
        .into_iter()
        .filter(|f| scanner::resolve_safe_path(&state.data_root, &f.path).is_ok())
        .map(|f| (f.id, f.path))
        .collect();

    let previews = rename::preview_renames(&state.data_root, &files, &body.format)?;
    Ok(Json(PreviewResponse { previews }))
}

pub async fn execute(
    State(state): State<AppState>,
    Json(body): Json<RenameRequest>,
) -> Result<Json<ExecuteResponse>, AppError> {
    let files: Vec<(String, String)> = body
        .files
        .into_iter()
        .filter(|f| scanner::resolve_safe_path(&state.data_root, &f.path).is_ok())
        .map(|f| (f.id, f.path))
        .collect();

    let results = rename::execute_renames(&state.data_root, &files, &body.format);
    Ok(Json(ExecuteResponse { results }))
}
