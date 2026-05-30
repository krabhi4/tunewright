use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use crate::state::{AppState, Session};
use crate::users::{self, Role};

const SESSION_COOKIE: &str = "tunewright_session";

fn generate_session_token() -> String {
    let bytes: [u8; 32] = rand::random();
    hex::encode(bytes)
}

fn set_session_cookie(token: &str) -> String {
    format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400",
        SESSION_COOKIE, token
    )
}

fn create_session_response(
    state: &AppState,
    user_id: &str,
    username: &str,
    role: Role,
) -> Response {
    let token = generate_session_token();
    state.add_session(
        token.clone(),
        Session {
            user_id: user_id.to_string(),
            username: username.to_string(),
            role,
            created_at: std::time::Instant::now(),
        },
    );
    let cookie = set_session_cookie(&token);
    (
        StatusCode::OK,
        [("Set-Cookie", cookie.as_str())],
        Json(serde_json::json!({
            "status": "ok",
            "user": { "username": username, "role": role }
        })),
    )
        .into_response()
}

// --- New-account credential validation + hashing (shared by setup/register) ---

/// Normalize and validate a new account's username/password, then hash the
/// password. On failure returns the user-facing error `Response` to send back.
async fn validate_and_hash(username: &str, password: &str) -> Result<(String, String), Response> {
    let username = username.trim().to_lowercase();
    if username.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Username cannot be empty" })),
        )
            .into_response());
    }
    if password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Password must be at least 8 characters" })),
        )
            .into_response());
    }

    let password = password.to_string();
    match tokio::task::spawn_blocking(move || users::hash_password(&password)).await {
        Ok(Ok(hash)) => Ok((username, hash)),
        _ => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to hash password" })),
        )
            .into_response()),
    }
}

// --- Setup: first user becomes super admin ---

#[derive(Deserialize)]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

pub async fn setup(State(state): State<AppState>, Json(body): Json<SetupRequest>) -> Response {
    let (username, hash) = match validate_and_hash(&body.username, &body.password).await {
        Ok(ok) => ok,
        Err(resp) => return resp,
    };

    match state.users.add_first_user(&username, hash) {
        Ok(user) => create_session_response(&state, &user.id, &user.username, user.role),
        Err(msg) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
    }
}

// --- Login ---

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login(State(state): State<AppState>, Json(body): Json<LoginRequest>) -> Response {
    // Brute-force throttling
    let delay = {
        let mut guard = state
            .failed_logins
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (count, last_failure) = &mut *guard;
        if *count > 0 && last_failure.elapsed().as_secs() > 60 {
            *count = 0;
        }
        let secs = (*count as u64).min(10);
        std::time::Duration::from_millis(secs * 500)
    };
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }

    let user = state.users.find_by_username(&body.username);
    let password = body.password.clone();

    let valid = match &user {
        Some(u) => {
            let hash = u.password_hash.clone();
            tokio::task::spawn_blocking(move || users::verify_password(&password, &hash))
                .await
                .unwrap_or(false)
        }
        None => {
            // Dummy verify to prevent timing oracle
            let dummy =
                "$argon2id$v=19$m=19456,t=2,p=1$dW50cnVzdGVk$AAAAAAAAAAAAAAAAAAAAAA".to_string();
            let _ = tokio::task::spawn_blocking(move || users::verify_password(&password, &dummy))
                .await;
            false
        }
    };

    if valid {
        let user = user.unwrap();
        let mut guard = state
            .failed_logins
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.0 = 0;
        drop(guard);
        create_session_response(&state, &user.id, &user.username, user.role)
    } else {
        let mut guard = state
            .failed_logins
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.0 = guard.0.saturating_add(1);
        guard.1 = std::time::Instant::now();
        drop(guard);
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Invalid credentials" })),
        )
            .into_response()
    }
}

// --- Logout ---

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

// --- Auth check ---

pub async fn check(State(state): State<AppState>, req: Request<Body>) -> Response {
    if !state.users.has_users() {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "setup_required": true })),
        )
            .into_response();
    }

    match get_session(&state, &req) {
        Some(session) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "authenticated": true,
                "user": { "username": session.username, "role": session.role }
            })),
        )
            .into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "authenticated": false })),
        )
            .into_response(),
    }
}

// --- Register via invite ---

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub token: String,
    pub username: String,
    pub password: String,
}

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Response {
    let (username, hash) = match validate_and_hash(&body.username, &body.password).await {
        Ok(ok) => ok,
        Err(resp) => return resp,
    };

    match state
        .users
        .register_with_invite(&body.token, &username, hash)
    {
        Ok(user) => create_session_response(&state, &user.id, &user.username, user.role),
        Err(msg) => {
            let status = if msg.contains("taken") {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
    }
}

// --- Invite management (super_admin only) ---

pub async fn create_invite(State(state): State<AppState>, req: Request<Body>) -> Response {
    let session = match require_super_admin(&state, &req) {
        Ok(s) => s,
        Err(r) => return r,
    };

    match state.users.create_invite(&session.user_id) {
        Ok(invite) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "token": invite.token,
                "expires_at": invite.expires_at,
                "link": format!("/register?token={}", invite.token)
            })),
        )
            .into_response(),
        Err(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
    }
}

pub async fn list_invites(State(state): State<AppState>, req: Request<Body>) -> Response {
    if let Err(r) = require_super_admin(&state, &req) {
        return r;
    }

    let invites: Vec<_> = state
        .users
        .list_invites()
        .into_iter()
        .map(|i| {
            serde_json::json!({
                "token": i.token,
                "created_by": i.created_by,
                "expires_at": i.expires_at,
                "link": format!("/register?token={}", i.token)
            })
        })
        .collect();

    (StatusCode::OK, Json(serde_json::json!(invites))).into_response()
}

pub async fn delete_invite(
    State(state): State<AppState>,
    Path(token): Path<String>,
    req: Request<Body>,
) -> Response {
    if let Err(r) = require_super_admin(&state, &req) {
        return r;
    }

    match state.users.delete_invite(&token) {
        Ok(true) => (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Invite not found" })),
        )
            .into_response(),
        Err(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
    }
}

// --- User management (super_admin only) ---

pub async fn list_users(State(state): State<AppState>, req: Request<Body>) -> Response {
    if let Err(r) = require_super_admin(&state, &req) {
        return r;
    }

    let users = state.users.list_users();
    (StatusCode::OK, Json(serde_json::json!(users))).into_response()
}

pub async fn delete_user(
    State(state): State<AppState>,
    Path(id): Path<String>,
    req: Request<Body>,
) -> Response {
    let session = match require_super_admin(&state, &req) {
        Ok(s) => s,
        Err(r) => return r,
    };

    if session.user_id == id {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Cannot delete yourself" })),
        )
            .into_response();
    }

    match state.users.remove_user(&id) {
        Ok(true) => {
            // Purge all sessions belonging to the deleted user
            let mut sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
            sessions.retain(|_, s| s.user_id != id);
            drop(sessions);
            (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "User not found" })),
        )
            .into_response(),
        Err(msg) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
    }
}

// --- Middleware ---

pub async fn require_auth(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();

    // Allow auth endpoints and health through unconditionally
    if path.starts_with("/auth/") || path == "/health" {
        return next.run(req).await;
    }

    // Setup mode: no users yet, block all non-auth API endpoints
    if !state.users.has_users() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "Setup required", "setup_required": true })),
        )
            .into_response();
    }

    if get_session(&state, &req).is_some() {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Authentication required" })),
        )
            .into_response()
    }
}

// --- Helpers ---

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

fn get_session(state: &AppState, req: &Request<Body>) -> Option<Session> {
    extract_token(req).and_then(|token| state.get_session(&token))
}

#[allow(clippy::result_large_err)]
fn require_super_admin(state: &AppState, req: &Request<Body>) -> Result<Session, Response> {
    let session = get_session(state, req).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Authentication required" })),
        )
            .into_response()
    })?;

    if session.role != Role::SuperAdmin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "Super admin access required" })),
        )
            .into_response());
    }

    Ok(session)
}
