use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use tagstudio_lookup::musicbrainz;
use tagstudio_lookup::types::{ReleaseDetail, ReleaseSearchResult};

use crate::state::AppState;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub query: String,
}

pub async fn musicbrainz_search(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<ReleaseSearchResult>>, Response> {
    // Rate limit: simple sleep before each request
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

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
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

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
