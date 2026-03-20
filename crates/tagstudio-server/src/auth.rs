use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use rand::Rng;
use serde::Deserialize;

use crate::state::AppState;

const SESSION_COOKIE: &str = "tagstudio_session";

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

fn generate_session_token() -> String {
    let mut rng = rand::rng();
    let bytes: [u8; 32] = rng.random();
    hex::encode(bytes)
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Response {
    if !state.config.auth_enabled {
        return (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response();
    }

    // Brute-force throttling: delay based on consecutive failures, decays after 60s idle
    let delay = {
        let mut guard = state.failed_logins.lock().unwrap_or_else(|e| e.into_inner());
        let (count, last_failure) = &mut *guard;
        // Decay: reset if no failure in the last 60 seconds
        if *count > 0 && last_failure.elapsed().as_secs() > 60 {
            *count = 0;
        }
        let secs = (*count as u64).min(10);
        std::time::Duration::from_millis(secs * 500)
    };
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }

    if body.username == state.config.username && body.password == state.config.password {
        // Reset failed counter on success
        if let Ok(mut guard) = state.failed_logins.lock() {
            guard.0 = 0;
        }

        let token = generate_session_token();
        state.add_session(token.clone());

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
        // Increment failed counter with timestamp
        if let Ok(mut guard) = state.failed_logins.lock() {
            guard.0 = guard.0.saturating_add(1);
            guard.1 = std::time::Instant::now();
        }

        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Invalid credentials" })),
        )
            .into_response()
    }
}

pub async fn logout(State(state): State<AppState>, req: Request<Body>) -> Response {
    if let Some(token) = extract_token(&req) {
        state.remove_session(&token);
    }

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

fn extract_token(req: &Request<Body>) -> Option<String> {
    req.headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                let c = c.trim();
                let prefix = format!("{}=", SESSION_COOKIE);
                if c.starts_with(&prefix) {
                    Some(c[prefix.len()..].to_string())
                } else {
                    None
                }
            })
        })
}

fn is_authenticated(state: &AppState, req: &Request<Body>) -> bool {
    match extract_token(req) {
        Some(token) => state.is_session_valid(&token),
        None => false,
    }
}
