use axum::body::Body;
use axum::extract::{Multipart, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::Json;
use rayon::prelude::*;
use reqwest::Url;
use serde::Deserialize;
use tunewright_core::picture;
use tunewright_core::scanner;
use tunewright_core::types::TunewrightError;

use crate::error::AppError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CoverArtFromUrlRequest {
    pub url: String,
    pub paths: Vec<String>,
}

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

/// Maximum accepted cover-art payload size.
const MAX_IMAGE_SIZE: u64 = 10 * 1024 * 1024;

/// JPEG (`FF D8`) or PNG (`89 50 4E 47`) magic-byte check.
fn has_image_magic(data: &[u8]) -> bool {
    data.starts_with(&[0xFF, 0xD8]) || data.starts_with(&[0x89, 0x50, 0x4E, 0x47])
}

pub async fn get_cover_art(
    State(state): State<AppState>,
    Query(params): Query<CoverArtQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let safe_path = scanner::resolve_safe_path(&state.data_root, &params.path)?;

    let max_size = if params.size == 0 { 0 } else { params.size };

    // ETag from the file's mtime + size plus the requested thumbnail size;
    // the frontend cache-busts with a version param on writes.
    let metadata = std::fs::metadata(&safe_path).map_err(TunewrightError::Io)?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .unwrap_or_default();
    let etag = format!(
        "\"{}-{}-{}-{}\"",
        mtime.as_secs(),
        mtime.subsec_nanos(),
        metadata.len(),
        max_size
    );

    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|candidate| candidate.trim() == etag))
    {
        return Ok(Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(header::ETAG, etag)
            .header(header::CACHE_CONTROL, "private, max-age=3600")
            .body(Body::empty())
            .unwrap());
    }

    let result = tokio::task::spawn_blocking(move || {
        picture::extract_cover_art_thumbnail(&safe_path, max_size)
    })
    .await
    .map_err(|e| AppError(TunewrightError::Io(std::io::Error::other(e.to_string()))))?
    .map_err(AppError)?;

    match result {
        Some((data, mime)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, picture::sanitize_mime(&mime))
            .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
            .header(header::CONTENT_DISPOSITION, "inline")
            .header(header::ETAG, etag)
            .header(header::CACHE_CONTROL, "private, max-age=3600")
            .body(Body::from(data))
            .map_err(|e| AppError(TunewrightError::Io(std::io::Error::other(e.to_string())))),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("No cover art"))
            .map_err(|e| AppError(TunewrightError::Io(std::io::Error::other(e.to_string())))),
    }
}

pub async fn delete_cover_art(
    State(state): State<AppState>,
    Query(params): Query<CoverArtQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let safe_path = scanner::resolve_safe_path(&state.data_root, &params.path)?;

    tokio::task::spawn_blocking(move || picture::remove_cover_art(&safe_path))
        .await
        .map_err(|e| AppError(TunewrightError::Io(std::io::Error::other(e.to_string()))))?
        .map_err(AppError)?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn embed_cover_art_from_url(
    State(state): State<AppState>,
    Json(body): Json<CoverArtFromUrlRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Only allow CoverArtArchive or Apple Music URLs, over https
    let parsed_ok = Url::parse(&body.url)
        .ok()
        .filter(|u| u.scheme() == "https")
        .and_then(|u| u.host_str().map(crate::state::is_allowed_cover_host_safe))
        .unwrap_or(false);
    if !parsed_ok {
        return Err(AppError(TunewrightError::InvalidInput(
            "only https coverartarchive.org and mzstatic.com URLs are allowed".to_string(),
        )));
    }

    crate::error::check_batch_size(body.paths.len())?;

    if body.paths.is_empty() {
        return Err(AppError(TunewrightError::InvalidInput(
            "no file paths provided".to_string(),
        )));
    }

    // Reuse the shared HTTP client with pre-configured redirects
    let client = &state.coverart_client;

    let mut response = client.get(&body.url).send().await.map_err(|e| {
        AppError(TunewrightError::Io(std::io::Error::other(format!(
            "failed to fetch cover art: {}",
            e
        ))))
    })?;

    if !response.status().is_success() {
        return Err(AppError(TunewrightError::Io(std::io::Error::other(
            format!("cover art fetch returned {}", response.status()),
        ))));
    }

    let too_large = || {
        AppError(TunewrightError::RequestTooLarge(
            "cover art too large (max 10MB)".to_string(),
        ))
    };

    // Reject oversized responses before buffering
    if let Some(len) = response.content_length() {
        if len > MAX_IMAGE_SIZE {
            return Err(too_large());
        }
    }

    // Stream with size limit to handle chunked responses without Content-Length
    let mut image_data = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| {
        AppError(TunewrightError::Io(std::io::Error::other(format!(
            "failed to read cover art bytes: {}",
            e
        ))))
    })? {
        image_data.extend_from_slice(&chunk);
        if image_data.len() as u64 > MAX_IMAGE_SIZE {
            return Err(too_large());
        }
    }

    if !has_image_magic(&image_data) {
        return Err(AppError(TunewrightError::InvalidInput(
            "invalid image format (JPEG or PNG only)".to_string(),
        )));
    }

    // Embed into all files in parallel, sharing the downloaded bytes; the
    // per-path results stay in request order.
    let data_root = state.data_root.clone();
    let paths = body.paths;
    let outcomes: Vec<Result<(), String>> = tokio::task::spawn_blocking(move || {
        paths
            .par_iter()
            .enumerate()
            .map(
                |(i, path_str)| match scanner::resolve_safe_path(&data_root, path_str) {
                    Ok(safe_path) => {
                        picture::embed_cover_art(&safe_path, &image_data).map_err(|e| {
                            tracing::warn!("cover art embed failed for {:?}: {}", path_str, e);
                            format!("file {}: embed failed", i)
                        })
                    }
                    Err(e) => {
                        tracing::warn!("path resolution failed for {:?}: {}", path_str, e);
                        Err(format!("file {}: invalid path", i))
                    }
                },
            )
            .collect()
    })
    .await
    .map_err(|e| AppError(TunewrightError::Io(std::io::Error::other(e.to_string()))))?;

    let mut embedded = 0u32;
    let mut errors: Vec<String> = Vec::new();
    for outcome in outcomes {
        match outcome {
            Ok(()) => embedded += 1,
            Err(msg) => errors.push(msg),
        }
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "embedded": embedded,
        "errors": errors,
    })))
}

pub async fn upload_cover_art(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut audio_path: Option<String> = None;
    let mut image_data: Option<Vec<u8>> = None;

    fn multipart_err(msg: &str) -> AppError {
        AppError(TunewrightError::InvalidInput(msg.to_string()))
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
                let text = field
                    .text()
                    .await
                    .map_err(|e| multipart_err(&e.to_string()))?;
                audio_path = Some(text);
            }
            "image" => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| multipart_err(&e.to_string()))?;
                if bytes.len() as u64 > MAX_IMAGE_SIZE {
                    return Err(multipart_err("image too large (max 10MB)"));
                }
                if !has_image_magic(&bytes) {
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

    tokio::task::spawn_blocking(move || picture::embed_cover_art(&safe_path, &data))
        .await
        .map_err(|e| multipart_err(&e.to_string()))?
        .map_err(AppError)?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}
