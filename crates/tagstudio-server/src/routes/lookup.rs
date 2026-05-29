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

/// Map a lookup-provider error string to a 502 Bad Gateway JSON response.
fn bad_gateway(e: String) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({ "error": e })),
    )
        .into_response()
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
    musicbrainz::search_releases(&state.http_client, &params.query)
        .await
        .map(Json)
        .map_err(bad_gateway)
}

pub async fn musicbrainz_release(
    State(state): State<AppState>,
    Path(mbid): Path<String>,
) -> Result<Json<ReleaseDetail>, Response> {
    rate_limit_musicbrainz(&state).await;
    musicbrainz::get_release(&state.http_client, &mbid)
        .await
        .map(Json)
        .map_err(bad_gateway)
}

pub async fn applemusic_search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<ReleaseSearchResult>>, Response> {
    applemusic::search_releases(&state.http_client, &params.query)
        .await
        .map(Json)
        .map_err(bad_gateway)
}

pub async fn applemusic_release(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ReleaseDetail>, Response> {
    applemusic::get_release(&state.http_client, &id)
        .await
        .map(Json)
        .map_err(bad_gateway)
}
