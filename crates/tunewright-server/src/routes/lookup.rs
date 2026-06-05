use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use std::time::Duration;
use tunewright_lookup::types::{ReleaseDetail, ReleaseSearchResult};
use tunewright_lookup::{applemusic, musicbrainz};

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
async fn rate_limit_musicbrainz(state: &AppState) -> Result<(), Response> {
    let wait = {
        let mut next_allowed = state
            .musicbrainz_next_allowed
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let now = std::time::Instant::now();
        let wait = next_allowed.checked_duration_since(now);

        if let Some(dur) = wait {
            if dur > Duration::from_secs(10) {
                return Err((
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(serde_json::json!({
                        "error": "MusicBrainz rate limit exceeded. Please try again later."
                    })),
                )
                    .into_response());
            }

            *next_allowed += MUSICBRAINZ_MIN_GAP;
            Some(dur)
        } else {
            *next_allowed = now + MUSICBRAINZ_MIN_GAP;
            None
        }
    };

    if let Some(dur) = wait {
        tokio::time::sleep(dur).await;
    }

    Ok(())
}

pub async fn musicbrainz_search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Response {
    if let Err(resp) = rate_limit_musicbrainz(&state).await {
        return resp;
    }
    match musicbrainz::search_releases(&state.http_client, &params.query).await {
        Ok(results) => Json(results).into_response(),
        Err(e) => bad_gateway(e),
    }
}

pub async fn musicbrainz_release(
    State(state): State<AppState>,
    Path(mbid): Path<String>,
) -> Response {
    if let Err(resp) = rate_limit_musicbrainz(&state).await {
        return resp;
    }
    match musicbrainz::get_release(&state.http_client, &mbid).await {
        Ok(detail) => Json(detail).into_response(),
        Err(e) => bad_gateway(e),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::users::UserManager;

    #[tokio::test]
    async fn test_rate_limit_musicbrainz_queue_bounding() {
        let temp_dir =
            std::env::temp_dir().join(format!("tunewright_test_mb_limit_{}", rand_num()));
        let user_manager = UserManager::load(temp_dir.join("users.json"));
        let config = Config {
            data_dir: temp_dir.clone(),
            static_dir: temp_dir.clone(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            setup_token: None,
        };
        let state = AppState::new(config, user_manager);

        // 1. Fire first request: should pass immediately
        let res1 = rate_limit_musicbrainz(&state).await;
        assert!(res1.is_ok());

        // 2. Pre-advance next_allowed to exceed the 10s wait limit (e.g. 12 seconds in future)
        {
            let mut next_allowed = state.musicbrainz_next_allowed.lock().unwrap();
            *next_allowed = std::time::Instant::now() + Duration::from_secs(12);
        }

        // 3. Fire second request: should immediately return 429 Too Many Requests
        let res2 = rate_limit_musicbrainz(&state).await;
        assert!(res2.is_err());
        let err_resp = res2.unwrap_err();
        assert_eq!(err_resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    fn rand_num() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}
