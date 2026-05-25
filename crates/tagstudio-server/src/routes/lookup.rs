use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use std::time::Duration;
use tagstudio_lookup::types::{ReleaseDetail, ReleaseSearchResult};
use tagstudio_lookup::{applemusic, musicbrainz};

use crate::state::AppState;

const MUSICBRAINZ_MIN_GAP: Duration = Duration::from_millis(1100);

#[derive(Deserialize)]
pub struct SearchQuery {
    pub query: String,
}

/// Enforce minimum gap between MusicBrainz requests globally.
/// Tracks next-allowed time to properly serialize concurrent requests.
async fn rate_limit_musicbrainz(state: &AppState) {
    let sleep_dur = {
        let mut next_allowed = state.musicbrainz_next_allowed.lock().unwrap();
        let now = std::time::Instant::now();
        let wait = next_allowed.checked_duration_since(now);
        // Schedule this request's slot and advance the next-allowed time
        *next_allowed = std::cmp::max(*next_allowed, now) + MUSICBRAINZ_MIN_GAP;
        wait
    };

    if let Some(dur) = sleep_dur {
        tokio::time::sleep(dur).await;
    }
}

pub async fn musicbrainz_search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<ReleaseSearchResult>>, Response> {
    rate_limit_musicbrainz(&state).await;

    let client = reqwest::Client::new();
    match musicbrainz::search_releases(&client, &params.query).await {
        Ok(results) => Ok(Json(results)),
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response()),
    }
}

pub async fn musicbrainz_release(
    State(state): State<AppState>,
    Path(mbid): Path<String>,
) -> Result<Json<ReleaseDetail>, Response> {
    rate_limit_musicbrainz(&state).await;

    let client = reqwest::Client::new();
    match musicbrainz::get_release(&client, &mbid).await {
        Ok(detail) => Ok(Json(detail)),
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response()),
    }
}

pub async fn applemusic_search(
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<ReleaseSearchResult>>, Response> {
    let client = reqwest::Client::new();
    match applemusic::search_releases(&client, &params.query).await {
        Ok(results) => Ok(Json(results)),
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response()),
    }
}

pub async fn applemusic_release(Path(id): Path<String>) -> Result<Json<ReleaseDetail>, Response> {
    let client = reqwest::Client::new();
    match applemusic::get_release(&client, &id).await {
        Ok(detail) => Ok(Json(detail)),
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response()),
    }
}
