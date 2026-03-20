use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use tagstudio_core::filename_to_tag::{self, FilenameTagPreview};
use tagstudio_core::scanner;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct FilenameToTagRequest {
    pub files: Vec<FileEntry>,
    pub pattern: String,
}

#[derive(Deserialize)]
pub struct FileEntry {
    pub id: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct PreviewResponse {
    pub previews: Vec<FilenameTagPreview>,
}

pub async fn preview(
    State(state): State<AppState>,
    Json(body): Json<FilenameToTagRequest>,
) -> Result<Json<PreviewResponse>, AppError> {
    let files: Vec<(String, String)> = body
        .files
        .into_iter()
        .filter(|f| scanner::resolve_safe_path(&state.data_root, &f.path).is_ok())
        .map(|f| {
            let filename = std::path::Path::new(&f.path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            (f.id, filename)
        })
        .collect();

    let previews = filename_to_tag::preview_extract(&files, &body.pattern)?;
    Ok(Json(PreviewResponse { previews }))
}
