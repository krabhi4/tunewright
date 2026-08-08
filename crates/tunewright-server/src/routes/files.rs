use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use tunewright_core::scanner;
use tunewright_core::types::FileListResult;

use crate::error::{join_error, AppError};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ListFilesQuery {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    500
}

pub async fn list_files(
    State(state): State<AppState>,
    Query(params): Query<ListFilesQuery>,
) -> Result<Json<FileListResult>, AppError> {
    let path = if params.path.is_empty() {
        "/".to_string()
    } else {
        params.path.clone()
    };

    let data_root = state.data_root.clone();
    let offset = params.offset;
    let limit = params.limit.clamp(1, 5000);

    let result = tokio::task::spawn_blocking(move || {
        scanner::scan_directory(&data_root, &path, offset, limit)
    })
    .await
    .map_err(join_error)??;

    Ok(Json(result))
}
