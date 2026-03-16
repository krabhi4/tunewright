pub mod coverart;
pub mod files;
pub mod health;
pub mod lookup;
pub mod rename;
pub mod tags;

use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::auth;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let api = Router::new()
        .route("/health", get(health::check))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/check", get(auth::check))
        .route("/files", get(files::list_files))
        .route("/files/tree", get(files::dir_tree))
        .route("/tags/read", post(tags::read_tags))
        .route("/tags/write", post(tags::write_tags))
        .route("/coverart", get(coverart::get_cover_art).delete(coverart::delete_cover_art))
        .route("/rename/preview", post(rename::preview))
        .route("/rename/execute", post(rename::execute))
        .route("/lookup/musicbrainz/search", get(lookup::musicbrainz_search))
        .route("/lookup/musicbrainz/release/{mbid}", get(lookup::musicbrainz_release))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ));

    let static_dir = state.config.static_dir.clone();
    let index_file = static_dir.join("index.html");

    Router::new()
        .nest("/api/v1", api)
        .fallback_service(
            ServeDir::new(&static_dir)
                .not_found_service(ServeFile::new(index_file)),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
