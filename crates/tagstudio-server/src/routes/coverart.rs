use axum::body::Body;
use axum::extract::{Multipart, Query, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::Json;
use reqwest::Url;
use serde::Deserialize;
use tagstudio_core::picture;
use tagstudio_core::scanner;
use tagstudio_core::types::TagStudioError;

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

/// Hosts permitted as a cover-art source and as redirect targets.
fn is_allowed_cover_host(host: &str) -> bool {
    let host = host.trim_end_matches('.');
    host == "coverartarchive.org"
        || host == "archive.org"
        || host.ends_with(".archive.org")
        || host == "mzstatic.com"
        || host.ends_with(".mzstatic.com")
}

/// JPEG (`FF D8`) or PNG (`89 50 4E 47`) magic-byte check.
fn has_image_magic(data: &[u8]) -> bool {
    data.starts_with(&[0xFF, 0xD8]) || data.starts_with(&[0x89, 0x50, 0x4E, 0x47])
}

pub async fn get_cover_art(
    State(state): State<AppState>,
    Query(params): Query<CoverArtQuery>,
) -> Result<Response, AppError> {
    let safe_path = scanner::resolve_safe_path(&state.data_root, &params.path)?;

    let max_size = if params.size == 0 { 0 } else { params.size };

    let result = picture::extract_cover_art_thumbnail(&safe_path, max_size).map_err(AppError)?;

    match result {
        Some((data, mime)) => Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, "private, max-age=60")
            .body(Body::from(data))
            .unwrap()),
        None => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("No cover art"))
            .unwrap()),
    }
}

pub async fn delete_cover_art(
    State(state): State<AppState>,
    Query(params): Query<CoverArtQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let safe_path = scanner::resolve_safe_path(&state.data_root, &params.path)?;

    picture::remove_cover_art(&safe_path).map_err(AppError)?;

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

pub async fn embed_cover_art_from_url(
    State(state): State<AppState>,
    Json(body): Json<CoverArtFromUrlRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Only allow CoverArtArchive or Apple Music URLs
    let parsed_ok = Url::parse(&body.url)
        .ok()
        .and_then(|u| u.host_str().map(is_allowed_cover_host))
        .unwrap_or(false);
    if !parsed_ok {
        return Err(AppError(TagStudioError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "only coverartarchive.org and mzstatic.com URLs are allowed",
        ))));
    }

    if body.paths.is_empty() {
        return Err(AppError(TagStudioError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "no file paths provided",
        ))));
    }

    // Fetch the image once, restricting redirects to known hosts
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (compatible; TagStudio/0.4.1; +https://github.com/tagstudio)")
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if is_allowed_cover_host(attempt.url().host_str().unwrap_or("")) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|e| {
            AppError(TagStudioError::Io(std::io::Error::other(format!(
                "failed to build HTTP client: {}",
                e
            ))))
        })?;

    let mut response = client.get(&body.url).send().await.map_err(|e| {
        AppError(TagStudioError::Io(std::io::Error::other(format!(
            "failed to fetch cover art: {}",
            e
        ))))
    })?;

    if !response.status().is_success() {
        return Err(AppError(TagStudioError::Io(std::io::Error::other(
            format!("cover art fetch returned {}", response.status()),
        ))));
    }

    let too_large = || {
        AppError(TagStudioError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "cover art too large (max 10MB)",
        )))
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
        AppError(TagStudioError::Io(std::io::Error::other(format!(
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
        return Err(AppError(TagStudioError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "invalid image format (JPEG or PNG only)",
        ))));
    }

    let mut embedded = 0u32;
    let mut errors: Vec<String> = Vec::new();

    for (i, path_str) in body.paths.iter().enumerate() {
        match scanner::resolve_safe_path(&state.data_root, path_str) {
            Ok(safe_path) => {
                let data = image_data.clone();
                let result = tokio::task::spawn_blocking(move || {
                    picture::embed_cover_art(&safe_path, &data)
                })
                .await;

                match result {
                    Ok(Ok(())) => embedded += 1,
                    Ok(Err(e)) => {
                        tracing::warn!("cover art embed failed for {:?}: {}", path_str, e);
                        errors.push(format!("file {}: embed failed", i));
                    }
                    Err(e) => {
                        tracing::warn!("cover art embed task panicked for {:?}: {}", path_str, e);
                        errors.push(format!("file {}: internal error", i));
                    }
                }
            }
            Err(e) => {
                tracing::warn!("path resolution failed for {:?}: {}", path_str, e);
                errors.push(format!("file {}: invalid path", i));
            }
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
        AppError(TagStudioError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            msg.to_string(),
        )))
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
