use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use crate::state::AppState;

const SESSION_COOKIE: &str = "tagstudio_session";

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Response {
    if !state.config.auth_enabled {
        return (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response();
    }

    // Constant-time comparison would be ideal; this is fine for single-user
    if body.username == state.config.username && body.password == state.config.password {
        // Simple token: hash of secret + timestamp
        let token = format!("{}:{}", state.config.session_secret, "authenticated");

        let cookie = format!(
            "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400",
            SESSION_COOKIE, token
        );

        (
            StatusCode::OK,
            [("Set-Cookie", cookie.as_str())],
            Json(serde_json::json!({ "status": "ok" })),
        )
            .into_response()
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Invalid credentials" })),
        )
            .into_response()
    }
}

pub async fn logout() -> Response {
    let cookie = format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        SESSION_COOKIE
    );

    (
        StatusCode::OK,
        [("Set-Cookie", cookie.as_str())],
        Json(serde_json::json!({ "status": "ok" })),
    )
        .into_response()
}

pub async fn check(State(state): State<AppState>, req: Request<Body>) -> Response {
    if !state.config.auth_enabled {
        return (StatusCode::OK, Json(serde_json::json!({ "authenticated": true, "auth_required": false }))).into_response();
    }

    if is_authenticated(&state, &req) {
        (StatusCode::OK, Json(serde_json::json!({ "authenticated": true, "auth_required": true }))).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "authenticated": false, "auth_required": true }))).into_response()
    }
}

/// Middleware to require authentication
pub async fn require_auth(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !state.config.auth_enabled {
        return next.run(req).await;
    }

    // Allow auth endpoints through
    let path = req.uri().path();
    if path.starts_with("/api/v1/auth/") || path == "/api/v1/health" {
        return next.run(req).await;
    }

    // Allow static files through
    if !path.starts_with("/api/") {
        return next.run(req).await;
    }

    if is_authenticated(&state, &req) {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Authentication required" })),
        )
            .into_response()
    }
}

fn is_authenticated(state: &AppState, req: &Request<Body>) -> bool {
    let expected_token = format!("{}:{}", state.config.session_secret, "authenticated");

    req.headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .map(|cookies| {
            cookies.split(';').any(|c| {
                let c = c.trim();
                c == format!("{}={}", SESSION_COOKIE, expected_token)
            })
        })
        .unwrap_or(false)
}
