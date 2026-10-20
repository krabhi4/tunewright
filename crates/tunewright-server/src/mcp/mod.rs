pub mod oauth;
pub mod tools;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::mcp_auth::{extract_origin, has_scope, Principal};
use crate::state::AppState;

const LATEST_PROTOCOL_VERSION: &str = "2025-11-25";
const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];
const BATCH_PROTOCOL_VERSION: &str = "2025-03-26";
const ALL_SCOPES: &str = "mcp:read mcp:write";
const INSTRUCTIONS: &str = "Tunewright edits the tags, file names and cover art of a self-hosted music library. Paths are relative to the library root; explore with tunewright_list_files starting at `/`. Use the preview tools before tunewright_execute_rename and tunewright_apply_actions, because write tools change files on disk.";

pub fn cors_layer() -> tower_http::cors::CorsLayer {
    tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::HeaderName::from_static("mcp-protocol-version"),
        ])
        .expose_headers([header::WWW_AUTHENTICATE])
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn http_error(status: StatusCode, code: i64, message: &str) -> Response {
    (status, Json(rpc_error(Value::Null, code, message))).into_response()
}

fn auth_challenge(
    status: StatusCode,
    origin: &str,
    error: Option<&str>,
    message: &str,
) -> Response {
    let mut challenge = format!(
        r#"Bearer resource_metadata="{origin}/.well-known/oauth-protected-resource/mcp", scope="{ALL_SCOPES}""#
    );
    if let Some(error) = error {
        challenge.push_str(&format!(r#", error="{error}""#));
    }
    (
        status,
        [(header::WWW_AUTHENTICATE, challenge)],
        Json(rpc_error(Value::Null, -32001, message)),
    )
        .into_response()
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let scheme = value
        .get(..7)
        .filter(|s| s.eq_ignore_ascii_case("bearer "))?;
    Some(value[scheme.len()..].trim()).filter(|t| !t.is_empty())
}

fn origin_allowed(headers: &HeaderMap, own_origin: &str) -> bool {
    headers.get(header::ORIGIN).is_none_or(|o| {
        o.to_str()
            .is_ok_and(|o| o.trim_end_matches('/').eq_ignore_ascii_case(own_origin))
    })
}

enum Message<'a> {
    Request {
        id: Value,
        method: &'a str,
        params: Option<&'a Value>,
    },
    Notification,
    Response,
}

fn classify(msg: &Value) -> Result<Message<'_>, &'static str> {
    let obj = msg
        .as_object()
        .ok_or("Invalid Request: expected a JSON-RPC object")?;
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err("Invalid Request: jsonrpc must be \"2.0\"");
    }
    let id = obj.get("id");
    if id.is_some_and(|id| !(id.is_string() || id.is_i64() || id.is_u64())) {
        return Err("Invalid Request: id must be a string or an integer");
    }
    match (obj.get("method"), id) {
        (Some(Value::String(method)), Some(id)) => Ok(Message::Request {
            id: id.clone(),
            method,
            params: obj.get("params"),
        }),
        (Some(Value::String(_)), None) => Ok(Message::Notification),
        (None, Some(_)) if obj.contains_key("result") || obj.contains_key("error") => {
            Ok(Message::Response)
        }
        _ => Err("Invalid Request: missing method"),
    }
}

fn required_scope(method: &str, params: Option<&Value>) -> Option<(&'static str, String)> {
    if method != "tools/call" {
        return None;
    }
    let name = params?.get("name")?.as_str()?;
    let tool = tools::find(name)?;
    let scope = if tools::is_read_only(tool) {
        "mcp:read"
    } else {
        "mcp:write"
    };
    Some((scope, name.to_string()))
}

async fn handle_request(
    state: &AppState,
    principal: &Principal,
    id: Value,
    method: &str,
    params: Option<&Value>,
) -> Value {
    let param = |key: &str| params.and_then(|p| p.get(key));
    match method {
        "initialize" => {
            let requested = param("protocolVersion").and_then(Value::as_str);
            let version = requested
                .filter(|v| SUPPORTED_PROTOCOL_VERSIONS.contains(v))
                .unwrap_or(LATEST_PROTOCOL_VERSION);
            rpc_result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": "tunewright",
                        "title": "Tunewright",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "instructions": INSTRUCTIONS
                }),
            )
        }
        "ping" => rpc_result(id, json!({})),
        "tools/list" => rpc_result(
            id,
            json!({ "tools": tools::list(has_scope(&principal.scope, "mcp:write")) }),
        ),
        "tools/call" => {
            let name = param("name").and_then(Value::as_str).unwrap_or_default();
            if tools::find(name).is_none() {
                return rpc_error(id, -32602, &format!("Unknown tool: {name}"));
            }
            let arguments = param("arguments").cloned().unwrap_or_else(|| json!({}));
            if !arguments.is_object() {
                return rpc_error(id, -32602, "Tool arguments must be an object");
            }
            let (text, is_error) = match tools::call(state.clone(), name, arguments).await {
                Ok(text) => (text, false),
                Err(text) => (text, true),
            };
            rpc_result(
                id,
                json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }),
            )
        }
        other => rpc_error(id, -32601, &format!("Method not found: {other}")),
    }
}

pub async fn mcp_post(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let origin = extract_origin(&headers, &state.config);
    if !origin_allowed(&headers, &origin) {
        return http_error(StatusCode::FORBIDDEN, -32600, "Origin not allowed");
    }

    let presented = bearer_token(&headers);
    let Some(principal) = presented
        .and_then(|t| state.mcp_auth.validate_bearer_token(t))
        .filter(|p| state.users.has_user_id(&p.user_id))
    else {
        return auth_challenge(
            StatusCode::UNAUTHORIZED,
            &origin,
            presented.map(|_| "invalid_token"),
            "Authentication required",
        );
    };

    let version = match headers.get("mcp-protocol-version").map(|v| v.to_str()) {
        None => BATCH_PROTOCOL_VERSION,
        Some(Ok(v)) if SUPPORTED_PROTOCOL_VERSIONS.contains(&v) => v,
        Some(_) => {
            return http_error(
                StatusCode::BAD_REQUEST,
                -32600,
                &format!(
                    "Unsupported MCP-Protocol-Version; supported: {}",
                    SUPPORTED_PROTOCOL_VERSIONS.join(", ")
                ),
            )
        }
    };

    let Ok(message) = serde_json::from_slice::<Value>(&body) else {
        return http_error(StatusCode::BAD_REQUEST, -32700, "Parse error");
    };
    let messages: Vec<&Value> = match &message {
        Value::Array(batch) if version == BATCH_PROTOCOL_VERSION && !batch.is_empty() => {
            batch.iter().collect()
        }
        Value::Array(_) => {
            return http_error(
                StatusCode::BAD_REQUEST,
                -32600,
                "Invalid Request: send one JSON-RPC message per POST",
            )
        }
        single => vec![single],
    };

    let mut requests = Vec::new();
    for msg in messages {
        match classify(msg) {
            Err(reason) => return http_error(StatusCode::BAD_REQUEST, -32600, reason),
            Ok(Message::Request { id, method, params }) => {
                if let Some((required, tool)) = required_scope(method, params) {
                    if !has_scope(&principal.scope, required) {
                        return auth_challenge(
                            StatusCode::FORBIDDEN,
                            &origin,
                            Some("insufficient_scope"),
                            &format!("{tool} requires the {required} scope"),
                        );
                    }
                }
                requests.push((id, method, params));
            }
            Ok(Message::Notification | Message::Response) => {}
        }
    }

    let mut replies = Vec::with_capacity(requests.len());
    for (id, method, params) in requests {
        replies.push(handle_request(&state, &principal, id, method, params).await);
    }
    match (message.is_array(), replies.pop()) {
        (_, None) => StatusCode::ACCEPTED.into_response(),
        (false, Some(reply)) => Json(reply).into_response(),
        (true, Some(last)) => {
            replies.push(last);
            Json(Value::Array(replies)).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::users::UserManager;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use base64::Engine;
    use reqwest::Url;
    use sha2::Digest;
    use tower::ServiceExt;

    fn setup_test_app() -> (axum::Router, AppState, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("tunewright_mcp_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let dir = dir.canonicalize().unwrap();
        let config = Config {
            data_dir: dir.clone(),
            state_dir: Some(dir.clone()),
            static_dir: dir.clone(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            trust_proxy: false,
            setup_token: None,
            public_url: None,
        };
        let users = UserManager::load(dir.join("users.json"));
        let hash = crate::users::hash_password("correct horse battery").unwrap();
        let _ = users.add_first_user("admin", hash).unwrap();
        let state = AppState::new(config, users);
        let app = crate::routes::create_router(state.clone());
        (app, state, dir)
    }

    fn pat(state: &AppState, scope: &str) -> String {
        let user = state.users.list_users().into_iter().next().unwrap();
        state
            .mcp_auth
            .create_pat(&user.id, "Test PAT", scope)
            .unwrap()
            .0
    }

    async fn post_mcp(
        app: &axum::Router,
        token: &str,
        extra: &[(&str, &str)],
        body: &str,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut req = Request::builder()
            .uri("/mcp")
            .method("POST")
            .header("Host", "127.0.0.1:8080")
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("Authorization", format!("Bearer {token}"));
        for (k, v) in extra {
            req = req.header(*k, *v);
        }
        let resp = app
            .clone()
            .oneshot(req.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, json)
    }

    async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri(uri)
            .header("Host", "127.0.0.1:8080")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[tokio::test]
    async fn test_oauth_metadata_discovery() {
        let (app, _state, dir) = setup_test_app();

        for uri in [
            "/.well-known/oauth-protected-resource",
            "/.well-known/oauth-protected-resource/mcp",
        ] {
            let (status, json) = get_json(&app, uri).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(json["resource"], "http://127.0.0.1:8080/mcp");
            assert_eq!(json["authorization_servers"][0], "http://127.0.0.1:8080");
        }

        let (status, json) = get_json(&app, "/.well-known/oauth-authorization-server").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["issuer"], "http://127.0.0.1:8080");
        assert_eq!(json["token_endpoint"], "http://127.0.0.1:8080/oauth/token");
        assert_eq!(json["code_challenge_methods_supported"][0], "S256");
        assert!(json.as_object().unwrap().values().all(|v| !v.is_null()));

        for uri in [
            "/.well-known/openid-configuration",
            "/.well-known/oauth-authorization-server/mcp",
        ] {
            let (status, json) = get_json(&app, uri).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
            assert!(json.is_object(), "{uri} must not fall through to the SPA");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_mcp_streamable_http_dispatch() {
        let (app, state, dir) = setup_test_app();
        let token = pat(&state, "mcp:read mcp:write");

        let req = Request::builder()
            .uri("/mcp")
            .method("GET")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::METHOD_NOT_ALLOWED
        );

        let (status, headers, _) =
            post_mcp(&app, "", &[], r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let challenge = headers.get("www-authenticate").unwrap().to_str().unwrap();
        assert!(challenge.contains(
            r#"resource_metadata="http://127.0.0.1:8080/.well-known/oauth-protected-resource/mcp""#
        ));
        assert!(!challenge.contains("error="));

        let (status, headers, _) = post_mcp(
            &app,
            "tw_mcp_at_bogus",
            &[],
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(headers
            .get("www-authenticate")
            .unwrap()
            .to_str()
            .unwrap()
            .contains(r#"error="invalid_token""#));

        let (status, _, json) = post_mcp(&app, &token, &[], r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(json["result"]["serverInfo"]["name"], "tunewright");

        let (_, _, json) = post_mcp(&app, &token, &[], r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2099-01-01"}}"#).await;
        assert_eq!(json["result"]["protocolVersion"], LATEST_PROTOCOL_VERSION);

        let modern = [("MCP-Protocol-Version", "2025-11-25")];
        let (status, _, json) = post_mcp(
            &app,
            &token,
            &modern,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["result"]["tools"].as_array().unwrap().len(), 15);

        let (status, _, _) = post_mcp(
            &app,
            &token,
            &modern,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);

        let (status, _, _) = post_mcp(
            &app,
            &token,
            &modern,
            r#"{"jsonrpc":"2.0","id":7,"result":{}}"#,
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);

        let (status, _, json) = post_mcp(
            &app,
            &token,
            &modern,
            r#"{"jsonrpc":"2.0","id":null,"method":"initialize"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], -32600);

        let (status, _, json) = post_mcp(&app, &token, &modern, "{not json").await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], -32700);

        let (status, _, json) = post_mcp(&app, &token, &modern, r#"[]"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"]["code"], -32600);

        let (status, _, _) = post_mcp(
            &app,
            &token,
            &modern,
            r#"[{"jsonrpc":"2.0","method":"notifications/initialized"}]"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _, _) = post_mcp(
            &app,
            &token,
            &[],
            r#"[{"jsonrpc":"2.0","method":"notifications/initialized"}]"#,
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);

        let (status, _, json) = post_mcp(&app, &token, &[], r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","id":2,"method":"ping"}]"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json.as_array().unwrap().len(), 2);

        let (status, _, _) = post_mcp(
            &app,
            &token,
            &[("MCP-Protocol-Version", "2026-07-28")],
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _, _) = post_mcp(
            &app,
            &token,
            &[("Origin", "https://evil.example")],
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let (status, _, _) = post_mcp(
            &app,
            &token,
            &[("Origin", "http://127.0.0.1:8080")],
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (_, _, json) = post_mcp(
            &app,
            &token,
            &modern,
            r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#,
        )
        .await;
        assert_eq!(json["error"]["code"], -32601);

        let (_, _, json) = post_mcp(
            &app,
            &token,
            &modern,
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"nope"}}"#,
        )
        .await;
        assert_eq!(json["error"]["code"], -32602);

        let (_, _, json) = post_mcp(&app, &token, &modern, r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"tunewright_read_tags","arguments":{}}}"#).await;
        assert_eq!(json["result"]["isError"], true);

        let req = Request::builder()
            .uri(format!("/mcp?token={token}"))
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );

        let req = Request::builder()
            .uri("/api/v1/mcp")
            .method("POST")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
            .unwrap();
        assert_ne!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::OK
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_read_only_token_cannot_write() {
        let (app, state, dir) = setup_test_app();
        let token = pat(&state, "mcp:read");

        let (_, _, json) = post_mcp(
            &app,
            &token,
            &[],
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
        )
        .await;
        let tools = json["result"]["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
        assert!(tools
            .iter()
            .all(|t| t["annotations"]["readOnlyHint"] == true));

        let (status, headers, _) = post_mcp(&app, &token, &[], r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tunewright_write_tags","arguments":{"changes":[]}}}"#).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let challenge = headers.get("www-authenticate").unwrap().to_str().unwrap();
        assert!(challenge.contains(r#"error="insufficient_scope""#));
        assert!(challenge.contains("mcp:write"));

        std::fs::write(dir.join("song.mp3"), b"not audio").unwrap();
        let (status, _, _) = post_mcp(&app, &token, &[], r#"[{"jsonrpc":"2.0","id":1,"method":"ping"},{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tunewright_execute_rename","arguments":{"files":["song.mp3"],"format":"renamed"}}}]"#).await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        let writer = pat(&state, "mcp:read mcp:write");
        let (status, _, _) = post_mcp(&app, &writer, &[], r#"[{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"tunewright_execute_rename","arguments":{"files":["song.mp3"],"format":"renamed"}}},{"jsonrpc":"1.0","id":2,"method":"ping"}]"#).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            dir.join("song.mp3").exists(),
            "a rejected batch must not run any of its calls"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_every_tool_is_dispatched() {
        let (_app, state, dir) = setup_test_app();
        for tool in tools::list(true) {
            let name = tool["name"].as_str().unwrap();
            if let Err(e) = tools::call(state.clone(), name, json!({ "invalid": true })).await {
                assert!(
                    !e.starts_with("Unknown tool"),
                    "{name} is listed but not dispatched"
                );
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_tools_reach_library() {
        let (app, state, dir) = setup_test_app();
        let token = pat(&state, "mcp:read mcp:write");
        std::fs::create_dir_all(dir.join("Album")).unwrap();
        std::fs::write(dir.join("Album/not-audio.txt"), b"x").unwrap();

        let (_, _, json) = post_mcp(&app, &token, &[], r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"tunewright_list_files","arguments":{}}}"#).await;
        assert_eq!(json["result"]["isError"], false);
        let listing: Value =
            serde_json::from_str(json["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert!(listing["directories"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d.as_str().unwrap().contains("Album")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_path_safety_in_tools() {
        let (app, state, dir) = setup_test_app();
        let token = pat(&state, "mcp:read mcp:write");

        let (status, _, json) = post_mcp(&app, &token, &[], r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"tunewright_list_files","arguments":{"path":"../../etc"}}}"#).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["result"]["isError"], true);

        let (status, _, json) = post_mcp(&app, &token, &[], r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tunewright_read_tags","arguments":{"paths":["../../etc/passwd"]}}}"#).await;
        assert_eq!(status, StatusCode::OK);
        let text = json["result"]["content"][0]["text"].as_str().unwrap();
        let result: Value = serde_json::from_str(text).unwrap();
        assert!(result["tags"].as_object().unwrap().is_empty());

        let (_, _, json) = post_mcp(&app, &token, &[], r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tunewright_get_coverart_info","arguments":{"path":"../../etc/passwd"}}}"#).await;
        assert_eq!(json["result"]["isError"], true);

        let (_, _, json) = post_mcp(&app, &token, &[], r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"tunewright_embed_coverart_url","arguments":{"url":"https://evil.example/x.jpg","paths":["a.mp3"]}}}"#).await;
        assert_eq!(json["result"]["isError"], true);

        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn register(app: &axum::Router, body: &str) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri("/oauth/register")
            .method("POST")
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    #[tokio::test]
    async fn test_oauth_registration_validation() {
        let (app, _state, dir) = setup_test_app();

        let (status, json) = register(
            &app,
            r#"{"client_name":"Good","redirect_uris":["http://127.0.0.1:54321/callback"]}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(json["token_endpoint_auth_method"], "none");
        assert!(json.as_object().unwrap().values().all(|v| !v.is_null()));

        let flood: Vec<&str> = std::iter::repeat_n("authorization_code", 5000)
            .chain(["refresh_token"; 5000])
            .collect();
        let body = json!({ "redirect_uris": ["http://[::1]:5555/cb"], "grant_types": flood });
        let (status, json) = register(&app, &body.to_string()).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            json["grant_types"],
            json!(["authorization_code", "refresh_token"])
        );

        let (status, code_only) = register(
            &app,
            r#"{"redirect_uris":["http://localhost/cb"],"grant_types":["authorization_code"]}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(code_only["grant_types"], json!(["authorization_code"]));
        let (status, json_default) =
            register(&app, r#"{"redirect_uris":["http://localhost/cb"]}"#).await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(
            json_default["grant_types"],
            json!(["authorization_code", "refresh_token"])
        );
        let req = Request::builder()
            .uri("/oauth/token")
            .method("POST")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(Body::from(format!(
                "grant_type=refresh_token&refresh_token=tw_mcp_rt_x.y&client_id={}",
                code_only["client_id"].as_str().unwrap()
            )))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"], "unauthorized_client");

        let ipv6_client = json["client_id"].as_str().unwrap();
        let req = Request::builder()
            .uri(format!("/oauth/authorize?client_id={ipv6_client}&redirect_uri=http%3A%2F%2F%5B%3A%3A1%5D%3A5555%2Fcb&response_type=code&code_challenge={}&code_challenge_method=S256", "a".repeat(43)))
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let csp = resp
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(csp.contains("form-action 'self' http:;"), "{csp}");

        let (status, _) = register(&app, r#"{"client_name":"Cursor","redirect_uris":["cursor://anysphere.cursor-mcp/oauth/callback","https://www.cursor.com/agents/mcp/oauth/callback","http://localhost:8787/callback"],"client_uri":null}"#).await;
        assert_eq!(status, StatusCode::CREATED);

        for bad in [
            r#"{"client_name":"BadFragment","redirect_uris":["http://127.0.0.1:54321/callback#token"]}"#,
            r#"{"client_name":"BadGrant","redirect_uris":["http://127.0.0.1:54321/callback"],"grant_types":["client_credentials"]}"#,
            r#"{"client_name":"BadAuth","redirect_uris":["http://127.0.0.1:54321/callback"],"token_endpoint_auth_method":"client_secret_post"}"#,
            r#"{"client_name":"PlainHttp","redirect_uris":["http://attacker.example/callback"]}"#,
            r#"{"client_name":"Script","redirect_uris":["javascript:alert(1)"]}"#,
            r#"{"client_name":"None","redirect_uris":[]}"#,
            r#"{"client_name":"Shell","redirect_uris":["search-ms:query=x&crumb=location:\\\\evil\\share"]}"#,
            r#"{"client_name":"NoCode","redirect_uris":["http://localhost/cb"],"grant_types":["refresh_token"]}"#,
            r#"not json"#,
        ] {
            let (status, json) = register(&app, bad).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
            assert!(json["error"].is_string(), "{bad}");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    async fn token_request(app: &axum::Router, body: String) -> (StatusCode, Value) {
        let req = Request::builder()
            .uri("/oauth/token")
            .method("POST")
            .header("Host", "127.0.0.1:8080")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(Body::from(body))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        if status == StatusCode::OK {
            assert_eq!(resp.headers().get("cache-control").unwrap(), "no-store");
        }
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    async fn authorize_form(app: &axum::Router, body: String, cookie: Option<&str>) -> Response {
        let mut req = Request::builder()
            .uri("/oauth/authorize")
            .method("POST")
            .header("Host", "127.0.0.1:8080")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("sec-fetch-site", "same-origin");
        if let Some(cookie) = cookie {
            req = req.header("Cookie", cookie);
        }
        app.clone()
            .oneshot(req.body(Body::from(body)).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_oauth_end_to_end_flow() {
        let (app, state, dir) = setup_test_app();

        let (_, client_json) = register(
            &app,
            r#"{"client_name":"Test Client","redirect_uris":["http://localhost:54321/callback"]}"#,
        )
        .await;
        let client_id = client_json["client_id"].as_str().unwrap().to_string();

        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(sha2::Sha256::digest(verifier.as_bytes()));
        let redirect = "http%3A%2F%2Flocalhost%3A60000%2Fcallback";
        let resource = "http%3A%2F%2F127.0.0.1%3A8080%2Fmcp";
        let query = format!("client_id={client_id}&redirect_uri={redirect}&response_type=code&code_challenge={challenge}&code_challenge_method=S256&state=xyz&resource={resource}");

        let req = Request::builder()
            .uri(format!("/oauth/authorize?{query}"))
            .header("Host", "127.0.0.1:8080")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let csp = resp
            .headers()
            .get("content-security-policy")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(csp.contains("form-action 'self' http://localhost:60000"));
        let html = String::from_utf8(
            axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(html.contains("localhost:60000"));
        assert!(html.contains(r#"name="password""#));

        let req = Request::builder()
            .uri(format!("/oauth/authorize?client_id={client_id}&redirect_uri={redirect}&response_type=code&code_challenge=short&code_challenge_method=S256&state=xyz"))
            .header("Host", "127.0.0.1:8080")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(
            resp.headers().get("location").is_none(),
            "errors before consent must not redirect"
        );

        let req = Request::builder()
            .uri(format!("/oauth/authorize?client_id={client_id}&redirect_uri=https%3A%2F%2Fevil.example%2Fcb&response_type=code"))
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(resp.headers().get("location").is_none());

        let req = Request::builder()
            .uri(format!("/oauth/authorize?client_id={client_id}&redirect_uri={redirect}&response_type=code&code_challenge={challenge}&code_challenge_method=S256&resource=https%3A%2F%2Fother.example%2Fmcp"))
            .header("Host", "127.0.0.1:8080")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(resp.headers().get("location").is_none());

        let req = Request::builder()
            .uri("/oauth/authorize")
            .method("POST")
            .header("Host", "127.0.0.1:8080")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("sec-fetch-site", "cross-site")
            .body(Body::from(format!(
                "{query}&decision=approve&username=admin&password=correct%20horse%20battery"
            )))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert!(resp.headers().get("location").is_none());

        let resp = authorize_form(
            &app,
            format!("{query}&decision=approve&username=admin&password=wrong"),
            None,
        )
        .await;
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let resp = authorize_form(&app, format!("{query}&decision=deny"), None).await;
        let location = resp.headers().get("location").unwrap().to_str().unwrap();
        assert!(location.contains("error=access_denied"));
        assert!(location.contains("iss=http%3A%2F%2F127.0.0.1%3A8080"));

        let user = state.users.list_users().into_iter().next().unwrap();
        state.add_session(
            "test_session_token_123".to_string(),
            crate::state::Session {
                user_id: user.id.clone(),
                username: user.username.clone(),
                role: user.role,
                created_at: std::time::Instant::now(),
            },
        );
        let resp = authorize_form(
            &app,
            format!("{query}&decision=approve"),
            Some("tunewright_session=test_session_token_123"),
        )
        .await;
        assert!(resp.status().is_redirection());
        let url = Url::parse(resp.headers().get("location").unwrap().to_str().unwrap()).unwrap();
        assert_eq!(url.port(), Some(60000));
        let param = |k: &str| {
            url.query_pairs()
                .find(|(key, _)| key == k)
                .map(|(_, v)| v.to_string())
        };
        assert_eq!(param("state").as_deref(), Some("xyz"));
        assert_eq!(param("iss").as_deref(), Some("http://127.0.0.1:8080"));
        let code = param("code").unwrap();

        let exchange = format!("grant_type=authorization_code&code={code}&client_id={client_id}&redirect_uri={redirect}&code_verifier={verifier}&resource={resource}");
        let (status, json) = token_request(&app, exchange.replace(verifier, &"x".repeat(43))).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"], "invalid_grant");

        let resp = authorize_form(
            &app,
            format!("{query}&decision=approve&username=admin&password=correct%20horse%20battery"),
            None,
        )
        .await;
        let url = Url::parse(resp.headers().get("location").unwrap().to_str().unwrap()).unwrap();
        let code = url
            .query_pairs()
            .find(|(k, _)| k == "code")
            .unwrap()
            .1
            .to_string();
        let exchange = format!("grant_type=authorization_code&code={code}&client_id={client_id}&redirect_uri={redirect}&code_verifier={verifier}&resource={resource}");

        let (status, token_json) = token_request(&app, exchange.clone()).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(token_json["token_type"], "Bearer");
        assert_eq!(token_json["scope"], "mcp:read mcp:write");
        let access_token = token_json["access_token"].as_str().unwrap();
        let refresh_token = token_json["refresh_token"].as_str().unwrap();

        let (status, json) = token_request(&app, exchange).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"], "invalid_grant");

        let (status, _, _) = post_mcp(
            &app,
            access_token,
            &[],
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, json) = token_request(
            &app,
            format!(
                "grant_type=refresh_token&refresh_token={refresh_token}&client_id=tw_client_other"
            ),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(json["error"], "invalid_client");

        let list_connections = || async {
            let req = Request::builder()
                .uri("/api/v1/mcp/tokens")
                .header("Cookie", "tunewright_session=test_session_token_123")
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
                .await
                .unwrap();
            serde_json::from_slice::<Value>(&bytes).unwrap()
        };
        let req = Request::builder()
            .uri("/api/v1/mcp/config")
            .header("Host", "127.0.0.1:8080")
            .header("Cookie", "tunewright_session=test_session_token_123")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let config: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(config["url"], "http://127.0.0.1:8080/mcp");

        let rows = list_connections().await;
        let row_id = rows[0]["id"].as_str().unwrap().to_string();
        assert_eq!(rows[0]["token_type"], "oauth");

        let (status, json) = token_request(&app, format!("grant_type=refresh_token&refresh_token={refresh_token}&client_id={client_id}&resource={resource}")).await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(json["refresh_token"], refresh_token);
        let refreshed_access = json["access_token"].as_str().unwrap().to_string();
        let refreshed_refresh = json["refresh_token"].as_str().unwrap().to_string();
        assert_eq!(list_connections().await[0]["id"], row_id.as_str());

        let req = Request::builder()
            .uri(format!("/api/v1/mcp/tokens/{row_id}"))
            .method("DELETE")
            .header("Cookie", "tunewright_session=test_session_token_123")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(req).await.unwrap().status(),
            StatusCode::OK
        );
        let (status, _, _) = post_mcp(
            &app,
            &refreshed_access,
            &[],
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = token_request(
            &app,
            format!(
                "grant_type=refresh_token&refresh_token={refreshed_refresh}&client_id={client_id}"
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, _, _) = post_mcp(
            &app,
            access_token,
            &[],
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, json) = token_request(
            &app,
            format!("grant_type=refresh_token&refresh_token={refresh_token}&client_id={client_id}"),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json["error"], "invalid_grant");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
