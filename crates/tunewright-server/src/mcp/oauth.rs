use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::{Form, Json};
use base64::Engine;
use reqwest::Url;
use serde::Deserialize;
use serde_json::json;

use crate::auth::{session_from_headers, verify_credentials, ClientIp};
use crate::mcp_auth::{
    extract_origin, is_own_resource, is_redirect_uri_allowed, is_registrable_redirect_uri,
    is_valid_code_challenge, normalize_scope, resource_url, verify_pkce, AuthCodeEntry,
    McpTokenPublic, OAuthClient, RefreshError, ACCESS_TOKEN_TTL_SECS, SCOPES,
};
use crate::state::AppState;
use crate::users;

const MAX_CLIENT_NAME_CHARS: usize = 200;
const MAX_REDIRECT_URIS: usize = 10;
const MAX_REDIRECT_URI_BYTES: usize = 512;
const MAX_PAT_NAME_CHARS: usize = 100;

async fn off_runtime<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, &'static str> {
    tokio::task::spawn_blocking(work).await.map_err(|e| {
        tracing::error!("MCP auth task failed: {e}");
        "Internal error"
    })
}

pub async fn protected_resource_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let origin = extract_origin(&headers, &state.config);
    Json(json!({
        "resource": resource_url(&origin),
        "resource_name": "Tunewright",
        "authorization_servers": [origin],
        "scopes_supported": SCOPES,
        "bearer_methods_supported": ["header"]
    }))
    .into_response()
}

pub async fn authorization_server_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let origin = extract_origin(&headers, &state.config);
    Json(json!({
        "issuer": origin,
        "authorization_endpoint": format!("{origin}/oauth/authorize"),
        "token_endpoint": format!("{origin}/oauth/token"),
        "registration_endpoint": format!("{origin}/oauth/register"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["none"],
        "scopes_supported": SCOPES,
        "authorization_response_iss_parameter_supported": true
    }))
    .into_response()
}

fn oauth_error(status: StatusCode, error: &str, description: &str) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({ "error": error, "error_description": description })),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    client_name: Option<String>,
    redirect_uris: Option<Vec<String>>,
    grant_types: Option<Vec<String>>,
    response_types: Option<Vec<String>>,
    token_endpoint_auth_method: Option<String>,
}

pub async fn register_client(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    body: axum::body::Bytes,
) -> Response {
    let invalid =
        |error: &str, description: &str| oauth_error(StatusCode::BAD_REQUEST, error, description);
    let body: RegisterRequest = match serde_json::from_slice(&body) {
        Ok(b) => b,
        Err(e) => return invalid("invalid_client_metadata", &e.to_string()),
    };

    let redirect_uris = body.redirect_uris.unwrap_or_default();
    if redirect_uris.is_empty() || redirect_uris.len() > MAX_REDIRECT_URIS {
        return invalid(
            "invalid_redirect_uri",
            &format!("Between 1 and {MAX_REDIRECT_URIS} redirect URIs are required"),
        );
    }
    if let Some(bad) = redirect_uris
        .iter()
        .find(|u| u.len() > MAX_REDIRECT_URI_BYTES || !is_registrable_redirect_uri(u))
    {
        let shown: String = bad.chars().take(100).collect();
        return invalid(
            "invalid_redirect_uri",
            &format!("Redirect URI must be https, http on a loopback address, or an app scheme, without a fragment: {shown}"),
        );
    }

    let requested_grants = body.grant_types.unwrap_or_else(|| {
        vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ]
    });
    if !requested_grants.iter().any(|g| g == "authorization_code")
        || !requested_grants
            .iter()
            .all(|g| g == "authorization_code" || g == "refresh_token")
    {
        return invalid(
            "invalid_client_metadata",
            "grant_types must include authorization_code and may add refresh_token",
        );
    }
    let mut grant_types = vec!["authorization_code".to_string()];
    if requested_grants.iter().any(|g| g == "refresh_token") {
        grant_types.push("refresh_token".to_string());
    }
    if body
        .response_types
        .is_some_and(|types| types.iter().any(|r| r != "code"))
    {
        return invalid("invalid_client_metadata", "Supported response_types: code");
    }
    if body.token_endpoint_auth_method.is_some_and(|m| m != "none") {
        return invalid(
            "invalid_client_metadata",
            "Only public clients are supported: token_endpoint_auth_method must be none",
        );
    }

    if !state.mcp_auth.allow_registration(ip) {
        return oauth_error(
            StatusCode::TOO_MANY_REQUESTS,
            "temporarily_unavailable",
            "Too many client registrations from this address, try again later",
        );
    }

    let client_name = body
        .client_name
        .map(|n| {
            n.trim()
                .chars()
                .take(MAX_CLIENT_NAME_CHARS)
                .collect::<String>()
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "MCP Client".to_string());

    let client = state
        .mcp_auth
        .register_client(client_name, redirect_uris, grant_types);
    (
        StatusCode::CREATED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({
            "client_id": client.client_id,
            "client_name": client.client_name,
            "redirect_uris": client.redirect_uris,
            "grant_types": client.grant_types,
            "response_types": client.response_types,
            "token_endpoint_auth_method": client.token_endpoint_auth_method,
            "client_id_issued_at": client.client_id_issued_at
        })),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct AuthorizeParams {
    response_type: Option<String>,
    client_id: Option<String>,
    redirect_uri: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    resource: Option<String>,
    decision: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

struct AuthorizeRequest {
    client: OAuthClient,
    redirect_uri: String,
    scope: String,
    state: Option<String>,
    code_challenge: String,
    resource: Option<String>,
    issuer: String,
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

const PAGE_STYLE: &str = r#"
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #0f172a; color: #f8fafc; display: flex; align-items: center; justify-content: center; min-height: 100vh; margin: 0; padding: 20px; box-sizing: border-box; }
    main { background: #1e293b; border: 1px solid #334155; border-radius: 12px; padding: 32px; max-width: 440px; width: 100%; box-shadow: 0 10px 25px rgba(0,0,0,0.5); }
    h1 { font-size: 20px; font-weight: 600; margin: 0 0 16px 0; color: #38bdf8; }
    p { font-size: 14px; color: #cbd5e1; line-height: 1.5; margin: 0 0 16px 0; }
    ul { font-size: 14px; color: #e2e8f0; margin: 0 0 16px 0; padding-left: 20px; line-height: 1.6; }
    .note { font-size: 13px; color: #94a3b8; }
    .error { background: #450a0a; border: 1px solid #b91c1c; color: #fecaca; border-radius: 6px; padding: 10px 12px; font-size: 13px; }
    label { display: block; font-size: 13px; color: #cbd5e1; margin: 0 0 6px 0; }
    input[type="text"], input[type="password"] { width: 100%; box-sizing: border-box; background: #0f172a; border: 1px solid #475569; border-radius: 6px; padding: 10px; color: #f8fafc; font-size: 14px; margin-bottom: 14px; }
    input:focus-visible, button:focus-visible { outline: 2px solid #38bdf8; outline-offset: 2px; }
    .actions { display: flex; gap: 12px; margin-top: 8px; }
    .actions form { flex: 1; margin: 0; }
    button { width: 100%; padding: 10px 16px; border-radius: 6px; font-size: 14px; font-weight: 500; cursor: pointer; border: none; }
    .primary { background: #0284c7; color: #fff; }
    .primary:hover { background: #0369a1; }
    .secondary { background: #334155; color: #e2e8f0; }
    .secondary:hover { background: #475569; }
"#;

fn page(status: StatusCode, title: &str, body: &str, form_target: &str) -> Response {
    let csp = format!(
        "default-src 'none'; style-src 'unsafe-inline'; form-action 'self' {form_target}; frame-ancestors 'none'; base-uri 'none'"
    );
    let title = html_escape(title);
    (
        status,
        [
            (header::CONTENT_SECURITY_POLICY, csp),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        Html(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta name="referrer" content="no-referrer">
  <title>{title} - Tunewright</title>
  <style>{PAGE_STYLE}</style>
</head>
<body>
  <main>
    <h1>{title}</h1>
    {body}
  </main>
</body>
</html>"#
        )),
    )
        .into_response()
}

fn error_page(message: &str) -> Response {
    page(
        StatusCode::BAD_REQUEST,
        "Authorization failed",
        &format!(
            r#"<p class="error" role="alert">{}</p><p class="note">Return to your MCP client and start the connection again.</p>"#,
            html_escape(message)
        ),
        "",
    )
}

fn redirect_target(redirect_uri: &str) -> (String, String) {
    let Ok(url) = Url::parse(redirect_uri) else {
        return (String::new(), String::new());
    };
    let host = url.host_str().unwrap_or("");
    let shown = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    };
    let ip_literal = host.starts_with('[');
    match url.scheme() {
        "http" | "https" if !ip_literal => (url.origin().ascii_serialization(), shown),
        scheme @ ("http" | "https") => (format!("{scheme}:"), shown),
        scheme => (format!("{scheme}:"), format!("{scheme}://{host}")),
    }
}

fn redirect_with(req: &AuthorizeRequest, params: &[(&str, &str)]) -> Response {
    let Ok(mut target) = Url::parse(&req.redirect_uri) else {
        return error_page("Invalid redirect URI");
    };
    {
        let mut query = target.query_pairs_mut();
        for (k, v) in params {
            query.append_pair(k, v);
        }
        if let Some(state) = &req.state {
            query.append_pair("state", state);
        }
        query.append_pair("iss", &req.issuer);
    }
    Redirect::to(target.as_str()).into_response()
}

#[allow(clippy::result_large_err)]
fn validate_authorize(
    state: &AppState,
    headers: &HeaderMap,
    p: &AuthorizeParams,
) -> Result<AuthorizeRequest, Response> {
    let Some(client) = p
        .client_id
        .as_deref()
        .and_then(|id| state.mcp_auth.get_client(id))
    else {
        return Err(error_page(
            "Unknown or missing client_id. The client may need to register again.",
        ));
    };
    let redirect_uri = match (&p.redirect_uri, client.redirect_uris.as_slice()) {
        (Some(uri), registered) if is_redirect_uri_allowed(registered, uri) => uri.clone(),
        (None, [only]) => only.clone(),
        _ => {
            return Err(error_page(
                "The redirect_uri is missing or not registered for this client.",
            ))
        }
    };
    if p.response_type.as_deref() != Some("code") {
        return Err(error_page(
            "Unsupported response_type: only code is supported.",
        ));
    }
    let code_challenge = match p.code_challenge.as_deref() {
        Some(c)
            if p.code_challenge_method.as_deref() == Some("S256") && is_valid_code_challenge(c) =>
        {
            c.to_string()
        }
        _ => {
            return Err(error_page(
                "PKCE with code_challenge_method=S256 is required.",
            ))
        }
    };
    let Some(scope) = normalize_scope(p.scope.as_deref().unwrap_or("")) else {
        return Err(error_page("Unsupported scope: use mcp:read and mcp:write."));
    };
    let issuer = extract_origin(headers, &state.config);
    if p.resource
        .as_deref()
        .is_some_and(|r| !is_own_resource(r, &issuer))
    {
        return Err(error_page(&format!(
            "This server only issues access to {}.",
            resource_url(&issuer)
        )));
    }
    Ok(AuthorizeRequest {
        client,
        redirect_uri,
        scope,
        state: p.state.clone().filter(|s| !s.is_empty()),
        code_challenge,
        resource: p.resource.clone(),
        issuer,
    })
}

fn hidden_fields(req: &AuthorizeRequest) -> String {
    let mut fields = vec![
        ("response_type", "code"),
        ("client_id", req.client.client_id.as_str()),
        ("redirect_uri", req.redirect_uri.as_str()),
        ("scope", req.scope.as_str()),
        ("code_challenge", req.code_challenge.as_str()),
        ("code_challenge_method", "S256"),
    ];
    if let Some(state) = &req.state {
        fields.push(("state", state));
    }
    if let Some(resource) = &req.resource {
        fields.push(("resource", resource));
    }
    fields
        .into_iter()
        .map(|(k, v)| {
            format!(
                r#"<input type="hidden" name="{k}" value="{}">"#,
                html_escape(v)
            )
        })
        .collect()
}

fn consent_page(
    req: &AuthorizeRequest,
    username: Option<&str>,
    error: Option<(StatusCode, &str)>,
) -> Response {
    let (form_target, destination) = redirect_target(&req.redirect_uri);
    let hidden = hidden_fields(req);
    let client = html_escape(&req.client.client_name);
    let permissions: String = req
        .scope
        .split_whitespace()
        .map(|s| match s {
            "mcp:read" => {
                "<li>Browse your library and read tags, audio properties and cover art</li>"
            }
            _ => "<li>Change tags, rename files and embed cover art</li>",
        })
        .collect();
    let error_html = error
        .map(|(_, msg)| format!(r#"<p class="error" role="alert">{}</p>"#, html_escape(msg)))
        .unwrap_or_default();
    let (who, approve) = match username {
        Some(user) => (
            format!(
                "<p>Signed in as <strong>{}</strong>.</p>",
                html_escape(user)
            ),
            format!(
                r#"<form method="post" action="/oauth/authorize">{hidden}<input type="hidden" name="decision" value="approve"><button type="submit" class="primary">Approve</button></form>"#
            ),
        ),
        None => (
            format!(
                r#"<form id="login" method="post" action="/oauth/authorize">{hidden}<input type="hidden" name="decision" value="approve">
      <label for="username">Username</label>
      <input id="username" type="text" name="username" required autofocus autocomplete="username">
      <label for="password">Password</label>
      <input id="password" type="password" name="password" required autocomplete="current-password">
    </form>"#
            ),
            r#"<button type="submit" form="login" class="primary">Sign in and approve</button>"#
                .to_string(),
        ),
    };
    let body = format!(
        r#"{error_html}<p><strong>{client}</strong> wants to connect to your Tunewright library. It will be able to:</p>
    <ul>{permissions}</ul>
    <p class="note">After you decide you will be sent to <strong>{}</strong>. Only approve if you started this connection yourself.</p>
    {who}
    <div class="actions">
      <form method="post" action="/oauth/authorize">{hidden}<input type="hidden" name="decision" value="deny"><button type="submit" class="secondary">Deny</button></form>
      {approve}
    </div>"#,
        html_escape(&destination)
    );
    let status = error.map_or(StatusCode::OK, |(s, _)| s);
    let title = if username.is_some() {
        "Authorize MCP client"
    } else {
        "Sign in to authorize"
    };
    page(status, title, &body, &form_target)
}

pub async fn authorize_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<AuthorizeParams>,
) -> Response {
    let req = match validate_authorize(&state, &headers, &params) {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    let session = session_from_headers(&state, &headers);
    consent_page(&req, session.as_ref().map(|s| s.username.as_str()), None)
}

pub async fn authorize_post(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Form(params): Form<AuthorizeParams>,
) -> Response {
    let cross_site = headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v != "same-origin" && v != "none");
    if cross_site {
        return error_page("Cross-site authorization requests are refused.");
    }
    let req = match validate_authorize(&state, &headers, &params) {
        Ok(r) => r,
        Err(resp) => return resp,
    };
    match params.decision.as_deref() {
        Some("deny") => return redirect_with(&req, &[("error", "access_denied")]),
        Some("approve") => {}
        _ => return error_page("Missing authorization decision."),
    }

    let user_id = match (&params.username, &params.password) {
        (Some(username), Some(password)) if !username.trim().is_empty() => {
            match verify_credentials(&state, ip, username, password).await {
                Ok(user) => user.id,
                Err(resp) => {
                    let message = match resp.status() {
                        StatusCode::UNAUTHORIZED => "Invalid username or password.",
                        StatusCode::TOO_MANY_REQUESTS => {
                            "Too many attempts. Wait a moment and try again."
                        }
                        StatusCode::SERVICE_UNAVAILABLE => "The server is busy. Try again.",
                        _ => "Sign-in failed.",
                    };
                    return consent_page(&req, None, Some((resp.status(), message)));
                }
            }
        }
        _ => match session_from_headers(&state, &headers) {
            Some(session) => session.user_id,
            None => {
                return consent_page(
                    &req,
                    None,
                    Some((
                        StatusCode::UNAUTHORIZED,
                        "Your session has expired. Sign in to continue.",
                    )),
                )
            }
        },
    };

    match state.mcp_auth.issue_authorization_code(AuthCodeEntry {
        client_id: req.client.client_id.clone(),
        user_id,
        redirect_uri: req.redirect_uri.clone(),
        scope: req.scope.clone(),
        code_challenge: req.code_challenge.clone(),
        created_at: std::time::Instant::now(),
    }) {
        Ok(code) => redirect_with(&req, &[("code", &code)]),
        Err(e) => redirect_with(
            &req,
            &[
                ("error", "temporarily_unavailable"),
                ("error_description", e),
            ],
        ),
    }
}

#[derive(Deserialize)]
pub struct TokenRequest {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
    resource: Option<String>,
}

fn basic_auth_client_id(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let scheme = value
        .get(..6)
        .filter(|p| p.eq_ignore_ascii_case("basic "))?;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(value[scheme.len()..].trim())
        .ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    decoded
        .split_once(':')
        .map(|(client_id, _)| client_id.to_string())
}

fn token_response(access_token: String, refresh_token: Option<String>, scope: &str) -> Response {
    let mut body = json!({
        "access_token": access_token,
        "token_type": "Bearer",
        "expires_in": ACCESS_TOKEN_TTL_SECS,
        "scope": scope
    });
    if let Some(refresh_token) = refresh_token {
        body["refresh_token"] = json!(refresh_token);
    }
    (
        StatusCode::OK,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
        ],
        Json(body),
    )
        .into_response()
}

pub async fn token_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let bad =
        |error: &str, description: &str| oauth_error(StatusCode::BAD_REQUEST, error, description);
    let is_json = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/json"));
    let parsed: Result<TokenRequest, String> = if is_json {
        serde_json::from_slice(&body).map_err(|e| e.to_string())
    } else {
        serde_urlencoded::from_bytes(&body).map_err(|e| e.to_string())
    };
    let req = match parsed {
        Ok(r) => r,
        Err(e) => return bad("invalid_request", &e),
    };

    let Some(client_id) = req
        .client_id
        .clone()
        .or_else(|| basic_auth_client_id(&headers))
    else {
        return bad("invalid_request", "client_id is required");
    };
    let Some(client) = state.mcp_auth.get_client(&client_id) else {
        return oauth_error(
            StatusCode::UNAUTHORIZED,
            "invalid_client",
            "Unknown client_id",
        );
    };
    let origin = extract_origin(&headers, &state.config);
    if req
        .resource
        .as_deref()
        .is_some_and(|r| !is_own_resource(r, &origin))
    {
        return bad(
            "invalid_target",
            "This server only issues tokens for its own /mcp resource",
        );
    }

    match req.grant_type.as_deref() {
        Some("authorization_code") => {
            let (Some(code), Some(verifier)) = (req.code.as_deref(), req.code_verifier.as_deref())
            else {
                return bad("invalid_request", "code and code_verifier are required");
            };
            let Some(stored) = state.mcp_auth.take_authorization_code(code) else {
                return bad(
                    "invalid_grant",
                    "Authorization code is invalid, expired or already used",
                );
            };
            if stored.client_id != client_id {
                return bad(
                    "invalid_grant",
                    "Authorization code was issued to another client",
                );
            }
            if req
                .redirect_uri
                .as_deref()
                .is_some_and(|uri| uri != stored.redirect_uri)
            {
                return bad(
                    "invalid_grant",
                    "redirect_uri does not match the authorization request",
                );
            }
            if !verify_pkce(verifier, &stored.code_challenge) {
                return bad("invalid_grant", "PKCE verification failed");
            }
            if !state.users.has_user_id(&stored.user_id) {
                return bad("invalid_grant", "User no longer exists");
            }
            let mcp_auth = state.mcp_auth.clone();
            let (user_id, scope) = (stored.user_id, stored.scope);
            let scope_for_task = scope.clone();
            match off_runtime(move || mcp_auth.issue_tokens(&user_id, &client, &scope_for_task))
                .await
                .and_then(|r| r)
            {
                Ok((access, refresh)) => token_response(access, refresh, &scope),
                Err(e) => oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", e),
            }
        }
        Some("refresh_token") => {
            let Some(refresh_token) = req.refresh_token else {
                return bad("invalid_request", "refresh_token is required");
            };
            if !client.allows_refresh() {
                return bad(
                    "unauthorized_client",
                    "This client did not register the refresh_token grant",
                );
            }
            let mcp_auth = state.mcp_auth.clone();
            let outcome =
                off_runtime(move || mcp_auth.refresh_tokens(&refresh_token, &client_id)).await;
            match outcome {
                Ok(Ok(issued)) if state.users.has_user_id(&issued.user_id) => token_response(
                    issued.access_token,
                    Some(issued.refresh_token),
                    &issued.scope,
                ),
                Ok(Ok(_)) => bad("invalid_grant", "User no longer exists"),
                Ok(Err(RefreshError::Invalid)) => {
                    bad("invalid_grant", "Refresh token is invalid or expired")
                }
                Ok(Err(RefreshError::Reused)) => bad(
                    "invalid_grant",
                    "Refresh token was already used, so this sign-in was revoked. Sign in again",
                ),
                Ok(Err(RefreshError::Storage(e))) | Err(e) => {
                    oauth_error(StatusCode::INTERNAL_SERVER_ERROR, "server_error", e)
                }
            }
        }
        _ => bad(
            "unsupported_grant_type",
            "Supported grant types: authorization_code, refresh_token",
        ),
    }
}

fn api_error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

#[allow(clippy::result_large_err)]
fn session_from(state: &AppState, headers: &HeaderMap) -> Result<crate::state::Session, Response> {
    session_from_headers(state, headers)
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "Authentication required"))
}

pub async fn mcp_config(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let origin = extract_origin(&headers, &state.config);
    Json(json!({ "url": resource_url(&origin) })).into_response()
}

pub async fn list_user_tokens(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let session = match session_from(&state, &headers) {
        Ok(s) => s,
        Err(resp) => return resp,
    };
    let visible =
        |user_id: &str| session.role == users::Role::SuperAdmin || user_id == session.user_id;
    let usernames: HashMap<_, _> = state
        .users
        .list_users()
        .into_iter()
        .map(|u| (u.id, u.username))
        .collect();
    let username = |user_id: &str| {
        usernames
            .get(user_id)
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string())
    };
    let (clients, grants, pats) = state.mcp_auth.grants_and_pats();
    let client_names: HashMap<_, _> = clients
        .into_iter()
        .map(|c| (c.client_id, c.client_name))
        .collect();

    let mut rows: Vec<McpTokenPublic> = grants
        .into_iter()
        .filter(|g| visible(&g.user_id))
        .map(|g| McpTokenPublic {
            username: username(&g.user_id),
            client_name: client_names.get(&g.client_id).cloned(),
            id: g.id,
            user_id: g.user_id,
            client_id: Some(g.client_id),
            name: None,
            token_type: "oauth".to_string(),
            scope: g.scope,
            created_at: g.created_at,
            expires_at: Some(g.refresh_expires_at),
            last_used_at: g.last_used_at,
        })
        .collect();
    rows.extend(
        pats.into_iter()
            .filter(|p| visible(&p.user_id))
            .map(|p| McpTokenPublic {
                username: username(&p.user_id),
                id: p.id,
                user_id: p.user_id,
                client_id: None,
                client_name: None,
                name: Some(p.name),
                token_type: "pat".to_string(),
                scope: p.scope,
                created_at: p.created_at,
                expires_at: None,
                last_used_at: p.last_used_at,
            }),
    );
    rows.sort_by_key(|r| std::cmp::Reverse(r.created_at));

    Json(rows).into_response()
}

#[derive(Deserialize)]
pub struct CreatePatRequest {
    name: String,
    scope: String,
}

pub async fn create_pat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreatePatRequest>,
) -> Response {
    let session = match session_from(&state, &headers) {
        Ok(s) => s,
        Err(resp) => return resp,
    };
    let name = body.name.trim().to_string();
    if name.is_empty() || name.chars().count() > MAX_PAT_NAME_CHARS {
        return api_error(
            StatusCode::BAD_REQUEST,
            &format!("Token name must be 1 to {MAX_PAT_NAME_CHARS} characters"),
        );
    }
    let scope = match normalize_scope(&body.scope) {
        Some(scope) if !body.scope.trim().is_empty() => scope,
        _ => {
            return api_error(
                StatusCode::BAD_REQUEST,
                "scope must be \"mcp:read\" or \"mcp:read mcp:write\"",
            )
        }
    };

    let mcp_auth = state.mcp_auth.clone();
    match off_runtime(move || mcp_auth.create_pat(&session.user_id, &name, &scope))
        .await
        .and_then(|r| r)
    {
        Ok((raw_token, pat)) => (
            StatusCode::CREATED,
            [(header::CACHE_CONTROL, "no-store")],
            Json(json!({
                "id": pat.id,
                "name": pat.name,
                "token": raw_token,
                "scope": pat.scope,
                "created_at": pat.created_at,
            })),
        )
            .into_response(),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

pub async fn revoke_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let session = match session_from(&state, &headers) {
        Ok(s) => s,
        Err(resp) => return resp,
    };
    let Some(owner) = state.mcp_auth.owner_of(&id) else {
        return api_error(StatusCode::NOT_FOUND, "Connection not found");
    };
    if session.role != users::Role::SuperAdmin && owner != session.user_id {
        return api_error(
            StatusCode::FORBIDDEN,
            "You can only revoke your own connections",
        );
    }
    let mcp_auth = state.mcp_auth.clone();
    match off_runtime(move || mcp_auth.revoke(&id))
        .await
        .and_then(|r| r)
    {
        Ok(true) => Json(json!({ "status": "ok" })).into_response(),
        Ok(false) => api_error(StatusCode::NOT_FOUND, "Connection not found"),
        Err(e) => api_error(StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}
