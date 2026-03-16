use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;
use tagstudio_core::scanner;
use tagstudio_core::types::{DirNode, FileListResult};

use crate::error::AppError;
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
        "/"
    } else {
        &params.path
    };

    let result =
        scanner::scan_directory(&state.data_root, path, params.offset, params.limit)?;

    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct DirTreeQuery {
    #[serde(default = "default_depth")]
    pub depth: usize,
}

fn default_depth() -> usize {
    3
}

pub async fn dir_tree(
    State(state): State<AppState>,
    Query(params): Query<DirTreeQuery>,
) -> Result<Json<DirNode>, AppError> {
    let tree = scanner::build_dir_tree(&state.data_root, params.depth)?;
    Ok(Json(tree))
}
