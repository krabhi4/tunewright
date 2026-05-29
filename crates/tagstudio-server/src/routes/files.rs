use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use tagstudio_core::scanner;
use tagstudio_core::types::{DirNode, FileListResult};

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
    let limit = params.limit;

    let result = tokio::task::spawn_blocking(move || {
        scanner::scan_directory(&data_root, &path, offset, limit)
    })
    .await
    .map_err(join_error)??;

    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct DirTreeQuery {
    #[serde(default = "default_depth")]
    pub depth: usize,
}

fn default_depth() -> usize {
    2
}

pub async fn dir_tree(
    State(state): State<AppState>,
    Query(params): Query<DirTreeQuery>,
) -> Result<Json<DirNode>, AppError> {
    let data_root = state.data_root.clone();
    let depth = params.depth.min(50);

    let tree = tokio::task::spawn_blocking(move || scanner::build_dir_tree(&data_root, depth))
        .await
        .map_err(join_error)??;

    Ok(Json(tree))
}
