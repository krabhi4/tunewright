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

fn set_session_cookie(token: &str, secure: bool) -> String {
    if secure {
        format!(
            "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400; Secure",
            SESSION_COOKIE, token
        )
    } else {
        format!(
            "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400",
            SESSION_COOKIE, token
        )
    }
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
    let cookie = set_session_cookie(&token, state.config.cookie_secure);
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
    #[serde(default)]
    pub setup_token: Option<String>,
}

pub async fn setup(State(state): State<AppState>, Json(body): Json<SetupRequest>) -> Response {
    // When configured, the setup token gates first-admin creation so a
    // network-exposed instance can't be claimed by whoever connects first.
    if let Some(required) = &state.config.setup_token {
        if body.setup_token.as_deref() != Some(required.as_str()) {
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({ "error": "Invalid setup token" })),
            )
                .into_response();
        }
    }

    let (username, hash) = match validate_and_hash(&body.username, &body.password).await {
        Ok(ok) => ok,
        Err(resp) => return resp,
    };

    let users = state.users.clone();
    let res = tokio::task::spawn_blocking(move || users.add_first_user(&username, hash)).await;

    match res {
        Ok(Ok(user)) => create_session_response(&state, &user.id, &user.username, user.role),
        Ok(Err(msg)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Spawn blocking failed: {e}") })),
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

/// Upper bound on tracked failed-login usernames; prevents unbounded memory
/// growth from spraying random usernames. Decayed entries are dropped first,
/// then the stalest entry if the map is still full.
const MAX_FAILED_LOGIN_ENTRIES: usize = 1000;
/// A username's failure counter resets after this much quiet time.
const FAILED_LOGIN_DECAY_SECS: u64 = 60;

fn record_failed_login(
    map: &mut std::collections::HashMap<String, (u32, std::time::Instant)>,
    key: String,
) {
    if !map.contains_key(&key) && map.len() >= MAX_FAILED_LOGIN_ENTRIES {
        map.retain(|_, (_, last)| last.elapsed().as_secs() <= FAILED_LOGIN_DECAY_SECS);
        while map.len() >= MAX_FAILED_LOGIN_ENTRIES {
            let Some(oldest) = map
                .iter()
                .min_by_key(|(_, (_, last))| *last)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            map.remove(&oldest);
        }
    }
    let entry = map.entry(key).or_insert((0, std::time::Instant::now()));
    entry.0 = entry.0.saturating_add(1);
    entry.1 = std::time::Instant::now();
}

pub async fn login(State(state): State<AppState>, Json(body): Json<LoginRequest>) -> Response {
    // Brute-force throttling, keyed per normalized username (matches the
    // normalization applied at account creation).
    let throttle_key = body.username.trim().to_lowercase();
    let delay = {
        let mut guard = state
            .failed_logins
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match guard.get_mut(&throttle_key) {
            Some(entry) => {
                if entry.0 > 0 && entry.1.elapsed().as_secs() > FAILED_LOGIN_DECAY_SECS {
                    entry.0 = 0;
                }
                let secs = (entry.0 as u64).min(10);
                std::time::Duration::from_millis(secs * 500)
            }
            None => std::time::Duration::ZERO,
        }
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
        guard.remove(&throttle_key);
        drop(guard);
        create_session_response(&state, &user.id, &user.username, user.role)
    } else {
        let mut guard = state
            .failed_logins
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        record_failed_login(&mut guard, throttle_key);
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
            Json(serde_json::json!({
                "setup_required": true,
                "setup_token_required": state.config.setup_token.is_some()
            })),
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

    let users = state.users.clone();
    let token = body.token.clone();
    let res =
        tokio::task::spawn_blocking(move || users.register_with_invite(&token, &username, hash))
            .await;

    match res {
        Ok(Ok(user)) => create_session_response(&state, &user.id, &user.username, user.role),
        Ok(Err(msg)) => {
            let status = if msg.contains("taken") {
                StatusCode::CONFLICT
            } else {
                StatusCode::BAD_REQUEST
            };
            (status, Json(serde_json::json!({ "error": msg }))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Spawn blocking failed: {e}") })),
        )
            .into_response(),
    }
}

// --- Invite management (super_admin only) ---

pub async fn create_invite(State(state): State<AppState>, req: Request<Body>) -> Response {
    let session = match require_super_admin(&state, &req) {
        Ok(s) => s,
        Err(r) => return r,
    };

    let users = state.users.clone();
    let user_id = session.user_id.clone();
    let res = tokio::task::spawn_blocking(move || users.create_invite(&user_id)).await;

    match res {
        Ok(Ok(invite)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "token": invite.token,
                "created_by": invite.created_by,
                "expires_at": invite.expires_at,
                "link": format!("/register?token={}", invite.token)
            })),
        )
            .into_response(),
        Ok(Err(msg)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Spawn blocking failed: {e}") })),
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

    let users = state.users.clone();
    let token_clone = token.clone();
    let res = tokio::task::spawn_blocking(move || users.delete_invite(&token_clone)).await;

    match res {
        Ok(Ok(true)) => {
            (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
        }
        Ok(Ok(false)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Invite not found" })),
        )
            .into_response(),
        Ok(Err(msg)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Spawn blocking failed: {e}") })),
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

    let users = state.users.clone();
    let id_clone = id.clone();
    let res = tokio::task::spawn_blocking(move || users.remove_user(&id_clone)).await;

    match res {
        Ok(Ok(true)) => {
            // Purge all sessions belonging to the deleted user
            let mut sessions = state.sessions.lock().unwrap_or_else(|e| e.into_inner());
            sessions.retain(|_, s| s.user_id != id);
            drop(sessions);
            (StatusCode::OK, Json(serde_json::json!({ "status": "ok" }))).into_response()
        }
        Ok(Ok(false)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "User not found" })),
        )
            .into_response(),
        Ok(Err(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": msg })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Spawn blocking failed: {e}") })),
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

    // Allow only the specific public auth endpoints and health unconditionally.
    // This provides defense-in-depth for private /auth/ endpoints.
    let is_public = path == "/auth/setup"
        || path == "/auth/login"
        || path == "/auth/logout"
        || path == "/auth/check"
        || path == "/auth/register"
        || path == "/health";

    if is_public {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::users::UserManager;

    #[test]
    fn test_set_session_cookie_secure() {
        let cookie_insecure = set_session_cookie("test_token", false);
        assert!(!cookie_insecure.contains("; Secure"));

        let cookie_secure = set_session_cookie("test_token", true);
        assert!(cookie_secure.contains("; Secure"));
    }

    #[tokio::test]
    async fn test_login_brute_force_throttling_per_username() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_srv_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let users_path = temp_dir.join("users.json");

        let user_manager = UserManager::load(users_path);
        let config = Config {
            data_dir: temp_dir.clone(),
            static_dir: temp_dir.clone(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            setup_token: None,
        };
        let state = AppState::new(config, user_manager);

        // Initially no failed logins
        {
            let guard = state.failed_logins.lock().unwrap();
            assert!(guard.is_empty());
        }

        // Simulate a failed login for user1
        let req1 = LoginRequest {
            username: "user1".to_string(),
            password: "wrong_password".to_string(),
        };
        let resp = login(State(state.clone()), Json(req1)).await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Verify that user1's failed login count is 1
        {
            let guard = state.failed_logins.lock().unwrap();
            assert_eq!(guard.get("user1").unwrap().0, 1);
            // user2 should still have no entries
            assert!(guard.get("user2").is_none());
        }

        // Simulate a failed login for user2
        let req2 = LoginRequest {
            username: "user2".to_string(),
            password: "wrong_password".to_string(),
        };
        let resp2 = login(State(state.clone()), Json(req2)).await;
        assert_eq!(resp2.status(), StatusCode::UNAUTHORIZED);

        // Verify that user2's failed login count is 1, and user1 is still 1
        {
            let guard = state.failed_logins.lock().unwrap();
            assert_eq!(guard.get("user1").unwrap().0, 1);
            assert_eq!(guard.get("user2").unwrap().0, 1);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_require_auth_middleware() {
        use axum::body::Body;
        use axum::http::Request;
        use axum::middleware::from_fn_with_state;
        use axum::routing::get;
        use axum::Router;
        use tower::ServiceExt;

        let temp_dir = std::env::temp_dir().join(format!("tunewright_srv_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Write a user to users.json so that has_users() returns true
        let users_path = temp_dir.join("users.json");
        std::fs::write(
            &users_path,
            r#"{"users":[{"id":"admin-id","username":"admin","password_hash":"nonsense","role":"super_admin","created_at":"2026-06-05T07:00:00Z"}],"invites":[]}"#,
        )
        .unwrap();

        let user_manager = UserManager::load(users_path);
        let config = Config {
            data_dir: temp_dir.clone(),
            static_dir: temp_dir.clone(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            setup_token: None,
        };
        let state = AppState::new(config, user_manager);

        // Build a router with the middleware
        let app = Router::new()
            .route("/auth/setup", get(|| async { "setup" }))
            .route("/auth/login", get(|| async { "login" }))
            .route("/auth/logout", get(|| async { "logout" }))
            .route("/auth/check", get(|| async { "check" }))
            .route("/auth/register", get(|| async { "register" }))
            .route("/health", get(|| async { "health" }))
            .route("/auth/invites", get(|| async { "invites" }))
            .route("/auth/users", get(|| async { "users" }))
            .route("/files", get(|| async { "files" }))
            .layer(from_fn_with_state(state.clone(), require_auth))
            .with_state(state);

        // 1. Check public endpoints (should pass and return OK)
        for public_path in &[
            "/auth/setup",
            "/auth/login",
            "/auth/logout",
            "/auth/check",
            "/auth/register",
            "/health",
        ] {
            let req = Request::builder()
                .uri(*public_path)
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::OK,
                "Path {} should be public",
                public_path
            );
        }

        // 2. Check protected endpoints without session (should return UNAUTHORIZED)
        for private_path in &["/auth/invites", "/auth/users", "/files"] {
            let req = Request::builder()
                .uri(*private_path)
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::UNAUTHORIZED,
                "Path {} should require authentication",
                private_path
            );
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_setup_token_enforced() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_srv_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let users_path = temp_dir.join("users.json");

        let user_manager = UserManager::load(users_path);
        let config = Config {
            data_dir: temp_dir.clone(),
            static_dir: temp_dir.clone(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            setup_token: Some("sekret123".to_string()),
        };
        let state = AppState::new(config, user_manager);

        // Missing token -> forbidden, no user created
        let resp = setup(
            State(state.clone()),
            Json(SetupRequest {
                username: "admin".to_string(),
                password: "password123".to_string(),
                setup_token: None,
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert!(!state.users.has_users());

        // Wrong token -> forbidden
        let resp = setup(
            State(state.clone()),
            Json(SetupRequest {
                username: "admin".to_string(),
                password: "password123".to_string(),
                setup_token: Some("wrong".to_string()),
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
        assert!(!state.users.has_users());

        // Correct token -> setup succeeds
        let resp = setup(
            State(state.clone()),
            Json(SetupRequest {
                username: "admin".to_string(),
                password: "password123".to_string(),
                setup_token: Some("sekret123".to_string()),
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(state.users.has_users());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_check_reports_setup_token_required() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_srv_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let users_path = temp_dir.join("users.json");

        let user_manager = UserManager::load(users_path);
        let config = Config {
            data_dir: temp_dir.clone(),
            static_dir: temp_dir.clone(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            setup_token: Some("sekret123".to_string()),
        };
        let state = AppState::new(config, user_manager);

        let req = Request::builder()
            .uri("/auth/check")
            .body(Body::empty())
            .unwrap();
        let resp = check(State(state), req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["setup_required"], true);
        assert_eq!(body["setup_token_required"], true);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_record_failed_login_bounded() {
        let mut map = std::collections::HashMap::new();
        for i in 0..(MAX_FAILED_LOGIN_ENTRIES + 100) {
            record_failed_login(&mut map, format!("user{}", i));
        }
        assert!(
            map.len() <= MAX_FAILED_LOGIN_ENTRIES,
            "failed-login map must be bounded, got {} entries",
            map.len()
        );
        // The most recent key must still be present
        assert!(map.contains_key(&format!("user{}", MAX_FAILED_LOGIN_ENTRIES + 99)));

        // Repeat failures increment the counter, not duplicate the entry
        record_failed_login(&mut map, "repeat".to_string());
        record_failed_login(&mut map, "repeat".to_string());
        assert_eq!(map.get("repeat").unwrap().0, 2);
    }

    #[tokio::test]
    async fn test_failed_login_key_normalized() {
        let temp_dir = std::env::temp_dir().join(format!("tunewright_srv_test_{}", rand_num()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let users_path = temp_dir.join("users.json");

        let user_manager = UserManager::load(users_path);
        let config = Config {
            data_dir: temp_dir.clone(),
            static_dir: temp_dir.clone(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            setup_token: None,
        };
        let state = AppState::new(config, user_manager);

        // Case/whitespace variants of the same username share one throttle entry
        let resp = login(
            State(state.clone()),
            Json(LoginRequest {
                username: "  User1 ".to_string(),
                password: "wrong_password".to_string(),
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let resp = login(
            State(state.clone()),
            Json(LoginRequest {
                username: "user1".to_string(),
                password: "wrong_password".to_string(),
            }),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        {
            let guard = state.failed_logins.lock().unwrap();
            assert_eq!(guard.len(), 1, "variants must share a single entry");
            assert_eq!(guard.get("user1").unwrap().0, 2);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    fn rand_num() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }
}
