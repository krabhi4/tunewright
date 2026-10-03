pub mod actions;
pub mod coverart;
pub mod filename_to_tag;
pub mod files;
pub mod health;
pub mod lookup;
pub mod rename;
pub mod tags;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue};
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::Router;
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

const CSP: &str = "default-src 'self'; script-src 'self'; img-src 'self' data: https://coverartarchive.org https://archive.org https://*.archive.org https://mzstatic.com https://*.mzstatic.com; style-src 'self' 'unsafe-inline'; font-src 'self' data:; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'";

fn content_security_policy(static_dir: &Path) -> HeaderValue {
    let index = std::fs::read_to_string(static_dir.join("index.html")).unwrap_or_default();
    let hashes: String = index
        .split('\'')
        .filter(|t| {
            t.strip_prefix("sha256-").is_some_and(|h| {
                !h.is_empty()
                    && h.chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '='))
            })
        })
        .map(|t| format!(" '{t}'"))
        .collect();
    let csp = CSP.replacen(
        "script-src 'self'",
        &format!("script-src 'self'{hashes}"),
        1,
    );
    HeaderValue::from_str(&csp).unwrap_or_else(|_| HeaderValue::from_static(CSP))
}

use crate::auth;
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let batch = DefaultBodyLimit::max(32 * 1024 * 1024);
    let api = Router::new()
        .route("/health", get(health::check))
        .route("/auth/setup", post(auth::setup))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/check", get(auth::check))
        .route("/auth/register", post(auth::register))
        .route(
            "/auth/invites",
            get(auth::list_invites).post(auth::create_invite),
        )
        .route("/auth/invites/{token}", delete(auth::delete_invite))
        .route("/auth/users", get(auth::list_users))
        .route("/auth/users/{id}", delete(auth::delete_user))
        .route("/files", get(files::list_files))
        .route("/tags/read", post(tags::read_tags).layer(batch))
        .route(
            "/tags/read-properties",
            post(tags::read_properties).layer(batch),
        )
        .route("/tags/write", post(tags::write_tags).layer(batch))
        .route(
            "/coverart",
            get(coverart::get_cover_art)
                .delete(coverart::delete_cover_art)
                .post(coverart::upload_cover_art)
                .layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .route(
            "/coverart/from-url",
            post(coverart::embed_cover_art_from_url),
        )
        .route("/rename/preview", post(rename::preview).layer(batch))
        .route("/rename/execute", post(rename::execute).layer(batch))
        .route(
            "/filename-to-tag/preview",
            post(filename_to_tag::preview).layer(batch),
        )
        .route("/actions/preview", post(actions::preview).layer(batch))
        .route("/actions/execute", post(actions::execute).layer(batch))
        .route(
            "/lookup/musicbrainz/search",
            get(lookup::musicbrainz_search),
        )
        .route(
            "/lookup/musicbrainz/release/{mbid}",
            get(lookup::musicbrainz_release),
        )
        .route("/lookup/applemusic/search", get(lookup::applemusic_search))
        .route(
            "/lookup/applemusic/release/{id}",
            get(lookup::applemusic_release),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ))
        // Catch-all: any unmatched /api/v1/* path returns a JSON 404 instead
        // of falling through to the unauthenticated SPA fallback.
        .fallback(api_not_found);

    let static_dir = state.config.static_dir.clone();
    let index_file = static_dir.join("index.html");
    let csp = content_security_policy(&static_dir);

    Router::new()
        .nest("/api/v1", api)
        .nest_service("/_app", ServeDir::new(static_dir.join("_app")))
        .fallback_service(ServeDir::new(&static_dir).fallback(ServeFile::new(index_file)))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            csp,
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static(
                "geolocation=(), camera=(), microphone=(), interest-cohort=()",
            ),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn api_not_found() -> axum::response::Response {
    (
        axum::http::StatusCode::NOT_FOUND,
        axum::Json(serde_json::json!({ "error": "Not found" })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csp_carries_sveltekit_script_hashes_only() {
        let dir = std::env::temp_dir().join(format!("tunewright_csp_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("index.html"),
            r#"<meta http-equiv="content-security-policy" content="script-src 'self' 'sha256-AbC+/12='"><script>x='sha256-bad;script-src *'</script>"#,
        )
        .unwrap();

        let csp = content_security_policy(&dir);
        let csp = csp.to_str().unwrap();
        assert!(csp.contains("default-src 'self'; script-src 'self' 'sha256-AbC+/12=';"));
        assert!(!csp.contains("bad"));

        let empty = content_security_policy(&dir.join("missing"));
        assert!(empty.to_str().unwrap().contains("script-src 'self';"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn only_batch_routes_accept_large_bodies() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use tower::ServiceExt;

        let dir = std::env::temp_dir().join(format!("tunewright_limits_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        std::fs::write(
            dir.join("users.json"),
            r#"{"users":[{"id":"u","username":"admin","password_hash":"x","role":"super_admin","created_at":"2026-06-05T07:00:00Z"}],"invites":[]}"#,
        )
        .unwrap();
        let config = crate::config::Config {
            data_dir: dir.clone(),
            state_dir: None,
            static_dir: dir.clone(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            trust_proxy: false,
            setup_token: None,
        };
        let state = AppState::new(
            config,
            crate::users::UserManager::load(dir.join("users.json")),
        );
        state.add_session(
            "t".to_string(),
            crate::state::Session {
                user_id: "u".to_string(),
                username: "admin".to_string(),
                role: crate::users::Role::SuperAdmin,
                created_at: std::time::Instant::now(),
            },
        );
        let app = create_router(state);
        let request = |uri: &str, content_type: &str, body: Vec<u8>| {
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("Cookie", "tunewright_session=t")
                .header("Content-Type", content_type)
                .body(Body::from(body))
                .unwrap()
        };

        let entry = format!(
            r#"{{"id":"x","path":"{}.mp3","tags":{{}}}}"#,
            "a".repeat(200)
        );
        let changes = vec![entry; 15_000].join(",");
        let body = format!(r#"{{"changes":[{changes}]}}"#).into_bytes();
        assert!(body.len() > 3 * 1024 * 1024);
        let resp = app
            .clone()
            .oneshot(request(
                "/api/v1/tags/write",
                "application/json",
                body.clone(),
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .oneshot(request("/api/v1/auth/register", "application/json", body))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
