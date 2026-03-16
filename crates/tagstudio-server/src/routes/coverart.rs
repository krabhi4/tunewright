use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use tagstudio_core::picture;
use tagstudio_core::scanner;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CoverArtQuery {
    #[serde(default)]
    pub path: String,
    #[serde(default = "default_size")]
    pub size: u32,
}

fn default_size() -> u32 {
    250
}

pub async fn get_cover_art(
    State(state): State<AppState>,
    Query(params): Query<CoverArtQuery>,
) -> Result<Response, AppError> {
    let safe_path = scanner::resolve_safe_path(&state.data_root, &params.path)?;

    let max_size = if params.size == 0 { 0 } else { params.size };

    let result = picture::extract_cover_art_thumbnail(&safe_path, max_size)
        .map_err(|e| AppError(e))?;

    match result {
        Some((data, mime)) => {
            Ok(Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "private, max-age=60")
                .body(Body::from(data))
                .unwrap())
        }
        None => {
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("No cover art"))
                .unwrap())
        }
    }
}

pub async fn delete_cover_art(
    State(state): State<AppState>,
    Query(params): Query<CoverArtQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let safe_path = scanner::resolve_safe_path(&state.data_root, &params.path)?;

    picture::remove_cover_art(&safe_path).map_err(|e| AppError(e))?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}
