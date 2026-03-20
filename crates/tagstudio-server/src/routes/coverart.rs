use axum::body::Body;
use axum::extract::{Multipart, Query, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::Json;
use serde::Deserialize;
use tagstudio_core::picture;
use tagstudio_core::scanner;
use tagstudio_core::types::TagStudioError;

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

pub async fn upload_cover_art(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut audio_path: Option<String> = None;
    let mut image_data: Option<Vec<u8>> = None;

    fn multipart_err(msg: &str) -> AppError {
        AppError(TagStudioError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, msg.to_string())))
    }

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return Err(multipart_err(&e.to_string())),
        };

        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "path" => {
                let text = field.text().await.map_err(|e| multipart_err(&e.to_string()))?;
                audio_path = Some(text);
            }
            "image" => {
                let bytes = field.bytes().await.map_err(|e| multipart_err(&e.to_string()))?;
                if bytes.len() > 10 * 1024 * 1024 {
                    return Err(multipart_err("image too large (max 10MB)"));
                }
                if !bytes.starts_with(&[0xFF, 0xD8]) && !bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                    return Err(multipart_err("invalid image format (JPEG or PNG only)"));
                }
                image_data = Some(bytes.to_vec());
            }
            _ => {}
        }
    }

    let path_str = audio_path.ok_or_else(|| multipart_err("missing 'path' field"))?;
    let data = image_data.ok_or_else(|| multipart_err("missing 'image' field"))?;

    let safe_path = scanner::resolve_safe_path(&state.data_root, &path_str)?;

    tokio::task::spawn_blocking(move || {
        picture::embed_cover_art(&safe_path, &data)
    })
    .await
    .map_err(|e| multipart_err(&e.to_string()))?
    .map_err(|e| AppError(e))?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}
