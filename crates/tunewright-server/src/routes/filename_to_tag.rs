use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use tunewright_core::filename_to_tag::{self, FilenameTagPreview};
use tunewright_core::scanner;

use crate::error::{check_batch_size, join_error, AppError};
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
    check_batch_size(body.files.len())?;
    let data_root = state.data_root.clone();

    let previews = tokio::task::spawn_blocking(move || {
        let files: Vec<(String, String)> = body
            .files
            .into_iter()
            .filter(|f| scanner::resolve_safe_path(&data_root, &f.path).is_ok())
            .map(|f| {
                let filename = std::path::Path::new(&f.path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                (f.id, filename)
            })
            .collect();

        filename_to_tag::preview_extract(&files, &body.pattern)
    })
    .await
    .map_err(join_error)??;

    Ok(Json(PreviewResponse { previews }))
}
