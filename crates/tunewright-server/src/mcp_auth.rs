use axum::http::HeaderMap;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::config::Config;

pub const ACCESS_TOKEN_TTL_SECS: i64 = 3600;
const REFRESH_TOKEN_TTL_DAYS: i64 = 30;
const REFRESH_REUSE_GRACE_SECS: u64 = 30;
const RETIRED_REFRESH_TOKENS: usize = 50;
const AUTH_CODE_TTL_SECS: u64 = 300;
const MAX_PENDING_CLIENTS: usize = 1000;
const IDLE_CLIENT_PRUNE_DAYS: i64 = 180;
const MAX_PENDING_CODES: usize = 1000;
const MAX_PENDING_CODES_PER_USER: usize = 20;
const USAGE_PERSIST_MINUTES: i64 = 10;
const REGISTRATIONS_PER_WINDOW: u32 = 20;
const REGISTRATION_WINDOW_SECS: u64 = 600;
const MAX_TRACKED_REGISTRANTS: usize = 1000;
const STORE_VERSION: u32 = 1;
const REFRESH_PREFIX: &str = "tw_mcp_rt_";
pub const SCOPES: [&str; 2] = ["mcp:read", "mcp:write"];
const APP_SCHEMES: [&str; 5] = [
    "cursor",
    "vscode",
    "vscode-insiders",
    "vscodium",
    "windsurf",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthClient {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub token_endpoint_auth_method: String,
    pub client_id_issued_at: i64,
    pub created_at: DateTime<Utc>,
}

impl OAuthClient {
    pub fn allows_refresh(&self) -> bool {
        self.grant_types.iter().any(|g| g == "refresh_token")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessToken {
    hash: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub id: String,
    pub user_id: String,
    pub client_id: String,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    refresh_hash: String,
    pub refresh_expires_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    retired_refresh_hashes: Vec<String>,
    #[serde(default)]
    access_tokens: Vec<AccessToken>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pat {
    pub id: String,
    pub user_id: String,
    pub name: String,
    hash: String,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpTokenPublic {
    pub id: String,
    pub user_id: String,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub token_type: String,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpAuthStore {
    version: u32,
    #[serde(default)]
    clients: Vec<OAuthClient>,
    #[serde(default)]
    grants: Vec<Grant>,
    #[serde(default)]
    pats: Vec<Pat>,
}

impl Default for McpAuthStore {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            clients: Vec::new(),
            grants: Vec::new(),
            pats: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthCodeEntry {
    pub client_id: String,
    pub user_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub code_challenge: String,
    pub created_at: Instant,
}

#[derive(Debug, Clone)]
pub struct Principal {
    pub user_id: String,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct IssuedTokens {
    pub user_id: String,
    pub scope: String,
    pub access_token: String,
    pub refresh_token: String,
}

#[derive(Debug, PartialEq)]
pub enum RefreshError {
    Invalid,
    Reused,
    Storage(&'static str),
}

#[derive(Debug, Clone, Copy)]
struct Usage {
    seen: DateTime<Utc>,
    saved: Option<DateTime<Utc>>,
}

struct RecentRefresh {
    at: Instant,
    client_id: String,
    issued: IssuedTokens,
}

#[derive(Clone)]
pub struct McpAuthManager {
    store: Arc<Mutex<McpAuthStore>>,
    write_lock: Arc<Mutex<()>>,
    path: PathBuf,
    pending_clients: Arc<Mutex<VecDeque<OAuthClient>>>,
    auth_codes: Arc<Mutex<HashMap<String, AuthCodeEntry>>>,
    usage: Arc<Mutex<HashMap<String, Usage>>>,
    recent_refreshes: Arc<Mutex<HashMap<String, RecentRefresh>>>,
    registrations: Arc<Mutex<HashMap<IpAddr, (u32, Instant)>>>,
}

impl std::fmt::Debug for McpAuthManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpAuthManager")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

fn random_hex<const N: usize>() -> String {
    hex::encode(rand::random::<[u8; N]>())
}

fn hash_token(raw: &str) -> String {
    hex::encode(Sha256::digest(raw.as_bytes()))
}

fn new_access_token(now: DateTime<Utc>) -> (String, AccessToken) {
    let raw = format!("tw_mcp_at_{}", random_hex::<32>());
    let token = AccessToken {
        hash: hash_token(&raw),
        expires_at: now + Duration::seconds(ACCESS_TOKEN_TTL_SECS),
    };
    (raw, token)
}

fn new_refresh_token(grant_id: &str) -> String {
    format!("{REFRESH_PREFIX}{grant_id}.{}", random_hex::<32>())
}

fn grant_id_of(raw_refresh_token: &str) -> Option<&str> {
    let (grant_id, _) = raw_refresh_token
        .strip_prefix(REFRESH_PREFIX)?
        .split_once('.')?;
    Some(grant_id)
}

fn registration_bucket(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => IpAddr::V6((u128::from(v6) >> 80 << 80).into()),
        v4 => v4,
    }
}

fn refuse_to_start(path: &std::path::Path, reason: &str) -> ! {
    tracing::error!(
        "FATAL: {:?} cannot be used ({}). Refusing to start to prevent losing MCP sign-ins and tokens. Fix or remove the file manually.",
        path,
        reason
    );
    std::process::exit(1);
}

fn quarantine(path: &std::path::Path, reason: &str) {
    let aside = path.with_extension(format!(
        "json.invalid-{}",
        Utc::now().format("%Y%m%dT%H%M%S%3f")
    ));
    match std::fs::rename(path, &aside) {
        Ok(()) => tracing::error!(
            "{:?} could not be parsed ({}); moved it to {:?} and started with no MCP clients or tokens",
            path,
            reason,
            aside
        ),
        Err(e) => refuse_to_start(path, &format!("{reason}; moving it aside failed: {e}")),
    }
}

impl McpAuthManager {
    pub fn load(path: PathBuf) -> Self {
        crate::persist::restrict_permissions(&path);
        #[derive(Deserialize)]
        struct VersionOnly {
            version: u32,
        }
        let store = match std::fs::read_to_string(&path) {
            Ok(c) if c.trim().is_empty() => McpAuthStore::default(),
            Ok(c) => {
                if let Ok(v) = serde_json::from_str::<VersionOnly>(&c) {
                    if v.version > STORE_VERSION {
                        refuse_to_start(
                            &path,
                            &format!(
                                "it was written by a newer Tunewright (store version {})",
                                v.version
                            ),
                        );
                    }
                }
                match serde_json::from_str::<McpAuthStore>(&c) {
                    Ok(s) if s.version == STORE_VERSION => s,
                    Ok(s) => {
                        quarantine(&path, &format!("unknown store version {}", s.version));
                        McpAuthStore::default()
                    }
                    Err(e) => {
                        quarantine(&path, &e.to_string());
                        McpAuthStore::default()
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => McpAuthStore::default(),
            Err(e) => refuse_to_start(&path, &e.to_string()),
        };
        Self {
            store: Arc::new(Mutex::new(store)),
            write_lock: Arc::new(Mutex::new(())),
            path,
            pending_clients: Arc::new(Mutex::new(VecDeque::new())),
            auth_codes: Arc::new(Mutex::new(HashMap::new())),
            usage: Arc::new(Mutex::new(HashMap::new())),
            recent_refreshes: Arc::new(Mutex::new(HashMap::new())),
            registrations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn snapshot(&self) -> McpAuthStore {
        let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
        let merge = |id: &str, stored: &mut Option<DateTime<Utc>>| {
            if let Some(u) = usage.get(id) {
                *stored = Some(stored.map_or(u.seen, |s| s.max(u.seen)));
            }
        };
        for g in &mut store.grants {
            merge(&g.id, &mut g.last_used_at);
        }
        for p in &mut store.pats {
            merge(&p.id, &mut p.last_used_at);
        }
        store
    }

    fn update<R>(
        &self,
        apply: impl FnOnce(&mut McpAuthStore) -> Result<R, &'static str>,
    ) -> Result<R, &'static str> {
        self.update_if_changed(|store| apply(store).map(|r| (r, true)))
    }

    fn update_if_changed<R>(
        &self,
        apply: impl FnOnce(&mut McpAuthStore) -> Result<(R, bool), &'static str>,
    ) -> Result<R, &'static str> {
        let _write_guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut next = self.snapshot();
        let (result, changed) = apply(&mut next)?;
        if !changed {
            return Ok(result);
        }
        let now = Utc::now();
        next.grants.retain(|g| g.refresh_expires_at > now);
        for g in &mut next.grants {
            g.access_tokens.retain(|t| t.expires_at > now);
        }
        let in_use: HashSet<&str> = next.grants.iter().map(|g| g.client_id.as_str()).collect();
        let idle_cutoff = now - Duration::days(IDLE_CLIENT_PRUNE_DAYS);
        next.clients
            .retain(|c| in_use.contains(c.client_id.as_str()) || c.created_at > idle_cutoff);
        let json = serde_json::to_string_pretty(&next).map_err(|e| {
            tracing::error!("Failed to serialize mcp auth: {}", e);
            "Failed to save MCP auth data"
        })?;
        crate::persist::write_private_atomically(&self.path, json.as_bytes()).map_err(|e| {
            tracing::error!("Failed to save MCP auth file {:?}: {}", self.path, e);
            "Failed to save MCP auth data"
        })?;
        let live: HashSet<String> = next
            .grants
            .iter()
            .map(|g| g.id.clone())
            .chain(next.pats.iter().map(|p| p.id.clone()))
            .collect();
        *self.store.lock().unwrap_or_else(|e| e.into_inner()) = next;
        self.usage
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|id, _| live.contains(id));
        Ok(result)
    }

    fn persist_in_background(&self) {
        let manager = self.clone();
        let persist = move || {
            let _ = manager.update(|_| Ok(()));
        };
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn_blocking(persist);
            }
            Err(_) => persist(),
        }
    }

    pub fn allow_registration(&self, ip: IpAddr) -> bool {
        let bucket = registration_bucket(ip);
        let mut map = self.registrations.lock().unwrap_or_else(|e| e.into_inner());
        if !map.contains_key(&bucket) && map.len() >= MAX_TRACKED_REGISTRANTS {
            map.retain(|_, (_, start)| start.elapsed().as_secs() < REGISTRATION_WINDOW_SECS);
            if map.len() >= MAX_TRACKED_REGISTRANTS {
                if let Some(oldest) = map
                    .iter()
                    .min_by_key(|(_, (_, start))| *start)
                    .map(|(k, _)| *k)
                {
                    map.remove(&oldest);
                }
            }
        }
        let entry = map.entry(bucket).or_insert((0, Instant::now()));
        if entry.1.elapsed().as_secs() >= REGISTRATION_WINDOW_SECS {
            *entry = (0, Instant::now());
        }
        if entry.0 >= REGISTRATIONS_PER_WINDOW {
            return false;
        }
        entry.0 += 1;
        true
    }

    pub fn register_client(
        &self,
        client_name: String,
        redirect_uris: Vec<String>,
        grant_types: Vec<String>,
    ) -> OAuthClient {
        let now = Utc::now();
        let client = OAuthClient {
            client_id: format!("tw_client_{}", random_hex::<16>()),
            client_name,
            redirect_uris,
            grant_types,
            response_types: vec!["code".to_string()],
            token_endpoint_auth_method: "none".to_string(),
            client_id_issued_at: now.timestamp(),
            created_at: now,
        };
        let mut pending = self
            .pending_clients
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if pending.len() >= MAX_PENDING_CLIENTS {
            pending.pop_front();
        }
        pending.push_back(client.clone());
        client
    }

    pub fn get_client(&self, client_id: &str) -> Option<OAuthClient> {
        let stored = {
            let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            store
                .clients
                .iter()
                .find(|c| c.client_id == client_id)
                .cloned()
        };
        stored.or_else(|| {
            let pending = self
                .pending_clients
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            pending.iter().find(|c| c.client_id == client_id).cloned()
        })
    }

    pub fn issue_authorization_code(&self, entry: AuthCodeEntry) -> Result<String, &'static str> {
        let mut codes = self.auth_codes.lock().unwrap_or_else(|e| e.into_inner());
        codes.retain(|_, v| v.created_at.elapsed().as_secs() < AUTH_CODE_TTL_SECS);
        let pending_for_user = codes
            .values()
            .filter(|c| c.user_id == entry.user_id)
            .count();
        if pending_for_user >= MAX_PENDING_CODES_PER_USER || codes.len() >= MAX_PENDING_CODES {
            return Err("Too many pending authorizations, try again in a few minutes");
        }
        let code = format!("tw_code_{}", random_hex::<32>());
        codes.insert(code.clone(), entry);
        Ok(code)
    }

    pub fn take_authorization_code(&self, code: &str) -> Option<AuthCodeEntry> {
        let mut codes = self.auth_codes.lock().unwrap_or_else(|e| e.into_inner());
        codes
            .remove(code)
            .filter(|entry| entry.created_at.elapsed().as_secs() < AUTH_CODE_TTL_SECS)
    }

    pub fn issue_tokens(
        &self,
        user_id: &str,
        client: &OAuthClient,
        scope: &str,
    ) -> Result<(String, Option<String>), &'static str> {
        let client_id = client.client_id.as_str();
        let issued = self.update(|store| {
            if !store.clients.iter().any(|c| c.client_id == client_id) {
                store.clients.push(client.clone());
            }
            let now = Utc::now();
            let id = format!("grant_{}", random_hex::<16>());
            let (access, access_token) = new_access_token(now);
            let refresh = new_refresh_token(&id);
            let refresh_expires_at = if client.allows_refresh() {
                now + Duration::days(REFRESH_TOKEN_TTL_DAYS)
            } else {
                access_token.expires_at
            };
            store.grants.push(Grant {
                refresh_hash: hash_token(&refresh),
                id,
                user_id: user_id.to_string(),
                client_id: client_id.to_string(),
                scope: scope.to_string(),
                created_at: now,
                refresh_expires_at,
                retired_refresh_hashes: Vec::new(),
                access_tokens: vec![access_token],
                last_used_at: None,
            });
            Ok((access, client.allows_refresh().then_some(refresh)))
        })?;
        self.pending_clients
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|c| c.client_id != client_id);
        Ok(issued)
    }

    fn recent_refresh(&self, hash: &str, client_id: &str) -> Option<IssuedTokens> {
        let mut recent = self
            .recent_refreshes
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        recent.retain(|_, r| r.at.elapsed().as_secs() < REFRESH_REUSE_GRACE_SECS);
        recent
            .get(hash)
            .filter(|r| r.client_id == client_id)
            .map(|r| r.issued.clone())
    }

    pub fn refresh_tokens(
        &self,
        raw_refresh_token: &str,
        client_id: &str,
    ) -> Result<IssuedTokens, RefreshError> {
        let grant_id = grant_id_of(raw_refresh_token).ok_or(RefreshError::Invalid)?;
        let hash = hash_token(raw_refresh_token);
        let known = {
            let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            store.grants.iter().any(|g| {
                g.id == grant_id
                    && g.client_id == client_id
                    && (g.refresh_hash == hash || g.retired_refresh_hashes.contains(&hash))
            })
        };
        if !known {
            return Err(RefreshError::Invalid);
        }
        self.update_if_changed(|store| {
            let now = Utc::now();
            let Some(idx) = store.grants.iter().position(|g| {
                g.id == grant_id && g.client_id == client_id && g.refresh_expires_at > now
            }) else {
                return Ok((Err(RefreshError::Invalid), false));
            };
            let grant = &mut store.grants[idx];
            if grant.refresh_hash != hash {
                if let Some(issued) = self
                    .recent_refresh(&hash, client_id)
                    .filter(|i| hash_token(&i.refresh_token) == grant.refresh_hash)
                {
                    return Ok((Ok(issued), false));
                }
                if grant.retired_refresh_hashes.contains(&hash) {
                    store.grants.remove(idx);
                    return Ok((Err(RefreshError::Reused), true));
                }
                return Ok((Err(RefreshError::Invalid), false));
            }
            let (access, access_token) = new_access_token(now);
            let refresh = new_refresh_token(&grant.id);
            let retired = std::mem::replace(&mut grant.refresh_hash, hash_token(&refresh));
            grant.retired_refresh_hashes.push(retired);
            let excess = grant
                .retired_refresh_hashes
                .len()
                .saturating_sub(RETIRED_REFRESH_TOKENS);
            grant.retired_refresh_hashes.drain(..excess);
            grant.refresh_expires_at = now + Duration::days(REFRESH_TOKEN_TTL_DAYS);
            grant.access_tokens = vec![access_token];
            let issued = IssuedTokens {
                user_id: grant.user_id.clone(),
                scope: grant.scope.clone(),
                access_token: access,
                refresh_token: refresh,
            };
            let mut recent = self
                .recent_refreshes
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            recent.retain(|_, r| r.at.elapsed().as_secs() < REFRESH_REUSE_GRACE_SECS);
            recent.insert(
                hash.clone(),
                RecentRefresh {
                    at: Instant::now(),
                    client_id: client_id.to_string(),
                    issued: issued.clone(),
                },
            );
            Ok((Ok(issued), true))
        })
        .map_err(RefreshError::Storage)?
    }

    pub fn create_pat(
        &self,
        user_id: &str,
        name: &str,
        scope: &str,
    ) -> Result<(String, Pat), &'static str> {
        let raw = format!("tw_pat_{}", random_hex::<32>());
        let pat = Pat {
            id: format!("pat_{}", random_hex::<16>()),
            user_id: user_id.to_string(),
            name: name.to_string(),
            hash: hash_token(&raw),
            scope: scope.to_string(),
            created_at: Utc::now(),
            last_used_at: None,
        };
        self.update(|store| {
            store.pats.push(pat.clone());
            Ok(())
        })?;
        Ok((raw, pat))
    }

    pub fn validate_bearer_token(&self, raw_token: &str) -> Option<Principal> {
        let hash = hash_token(raw_token);
        let now = Utc::now();
        let (id, principal) = {
            let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(g) = store.grants.iter().find(|g| {
                g.access_tokens
                    .iter()
                    .any(|t| t.hash == hash && t.expires_at > now)
            }) {
                let principal = Principal {
                    user_id: g.user_id.clone(),
                    scope: g.scope.clone(),
                };
                (g.id.clone(), principal)
            } else {
                let p = store.pats.iter().find(|p| p.hash == hash)?;
                let principal = Principal {
                    user_id: p.user_id.clone(),
                    scope: p.scope.clone(),
                };
                (p.id.clone(), principal)
            }
        };
        let due = {
            let mut usage = self.usage.lock().unwrap_or_else(|e| e.into_inner());
            let entry = usage.entry(id).or_insert(Usage {
                seen: now,
                saved: None,
            });
            entry.seen = now;
            let due = entry
                .saved
                .is_none_or(|saved| now - saved > Duration::minutes(USAGE_PERSIST_MINUTES));
            if due {
                entry.saved = Some(now);
            }
            due
        };
        if due {
            self.persist_in_background();
        }
        Some(principal)
    }

    pub fn grants_and_pats(&self) -> (Vec<OAuthClient>, Vec<Grant>, Vec<Pat>) {
        let store = self.snapshot();
        (store.clients, store.grants, store.pats)
    }

    pub fn owner_of(&self, id: &str) -> Option<String> {
        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        store
            .grants
            .iter()
            .find(|g| g.id == id)
            .map(|g| g.user_id.clone())
            .or_else(|| {
                store
                    .pats
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.user_id.clone())
            })
    }

    pub fn revoke(&self, id: &str) -> Result<bool, &'static str> {
        self.update(|store| {
            let before = store.grants.len() + store.pats.len();
            store.grants.retain(|g| g.id != id);
            store.pats.retain(|p| p.id != id);
            Ok(store.grants.len() + store.pats.len() < before)
        })
    }

    pub fn revoke_user(&self, user_id: &str) -> Result<(), &'static str> {
        self.update(|store| {
            store.grants.retain(|g| g.user_id != user_id);
            store.pats.retain(|p| p.user_id != user_id);
            Ok(())
        })
    }
}

pub fn normalize_scope(raw: &str) -> Option<String> {
    let mut requested: Vec<&str> = raw.split_whitespace().collect();
    if requested.is_empty() {
        requested = SCOPES.to_vec();
    }
    if !requested.iter().all(|s| SCOPES.contains(s)) {
        return None;
    }
    Some(
        SCOPES
            .iter()
            .filter(|s| requested.contains(s))
            .copied()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

pub fn has_scope(granted: &str, required: &str) -> bool {
    granted
        .split_whitespace()
        .any(|s| s == required || (required == "mcp:read" && s == "mcp:write"))
}

fn is_loopback_host(h: &str) -> bool {
    h == "127.0.0.1" || h == "localhost" || h == "[::1]"
}

pub fn is_registrable_redirect_uri(uri: &str) -> bool {
    let Ok(url) = Url::parse(uri) else {
        return false;
    };
    if url.fragment().is_some() || uri.contains('#') {
        return false;
    }
    match url.scheme() {
        "https" => true,
        "http" => url.host_str().is_some_and(is_loopback_host),
        scheme => scheme.contains('.') || APP_SCHEMES.contains(&scheme),
    }
}

fn is_loopback_http(url: &Url) -> bool {
    url.scheme() == "http" && url.host_str().is_some_and(is_loopback_host)
}

pub fn is_redirect_uri_allowed(registered_uris: &[String], requested_uri: &str) -> bool {
    if !is_registrable_redirect_uri(requested_uri) {
        return false;
    }
    if registered_uris.iter().any(|u| u == requested_uri) {
        return true;
    }
    let Ok(req) = Url::parse(requested_uri) else {
        return false;
    };
    is_loopback_http(&req)
        && registered_uris
            .iter()
            .filter_map(|u| Url::parse(u).ok())
            .any(|reg| {
                is_loopback_http(&reg) && reg.path() == req.path() && reg.query() == req.query()
            })
}

pub fn verify_pkce(code_verifier: &str, code_challenge: &str) -> bool {
    if code_verifier.len() < 43 || code_verifier.len() > 128 {
        return false;
    }
    if !code_verifier
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_' || b == b'~')
    {
        return false;
    }
    URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes())) == code_challenge
}

pub fn is_valid_code_challenge(challenge: &str) -> bool {
    challenge.len() == 43
        && challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn extract_origin(headers: &HeaderMap, config: &Config) -> String {
    if let Some(public_url) = &config.public_url {
        return public_url.clone();
    }
    let scheme = if config.trust_proxy {
        match headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
        {
            Some(s) if s.eq_ignore_ascii_case("https") => "https",
            Some(s) if s.eq_ignore_ascii_case("http") => "http",
            _ => {
                if config.cookie_secure {
                    "https"
                } else {
                    "http"
                }
            }
        }
    } else if config.cookie_secure {
        "https"
    } else {
        "http"
    };

    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok());
    let host = if config.trust_proxy {
        header("x-forwarded-host")
            .and_then(|v| v.rsplit(',').next())
            .or_else(|| header("host"))
    } else {
        header("host")
    };

    let is_valid_host = |h: &str| -> bool {
        !h.is_empty()
            && h.len() <= 253
            && h.bytes().all(|b| {
                b.is_ascii_alphanumeric()
                    || b == b'.'
                    || b == b'-'
                    || b == b':'
                    || b == b'['
                    || b == b']'
            })
    };

    match host.map(str::trim) {
        Some(h) if is_valid_host(h) => format!("{scheme}://{}", h.to_ascii_lowercase()),
        _ => {
            let host_str = if config.host.contains(':') && !config.host.starts_with('[') {
                format!("[{}]", config.host)
            } else {
                config.host.clone()
            };
            format!("{scheme}://{host_str}:{}", config.port)
        }
    }
}

pub fn resource_url(origin: &str) -> String {
    format!("{origin}/mcp")
}

pub fn is_own_resource(requested: &str, origin: &str) -> bool {
    match (Url::parse(requested), Url::parse(&resource_url(origin))) {
        (Ok(req), Ok(own)) => {
            req.origin() == own.origin()
                && req.path().trim_end_matches('/') == own.path()
                && req.query().is_none()
                && req.fragment().is_none()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_manager() -> (McpAuthManager, PathBuf) {
        let temp_dir = std::env::temp_dir().join(format!("mcp_auth_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        (
            McpAuthManager::load(temp_dir.join("mcp_auth.json")),
            temp_dir,
        )
    }

    fn register_with(manager: &McpAuthManager, name: &str, grant_types: &[&str]) -> OAuthClient {
        manager.register_client(
            name.to_string(),
            vec!["http://127.0.0.1:54321/callback".to_string()],
            grant_types.iter().map(|g| g.to_string()).collect(),
        )
    }

    fn register(manager: &McpAuthManager, name: &str) -> OAuthClient {
        register_with(manager, name, &["authorization_code", "refresh_token"])
    }

    fn grant_ids(manager: &McpAuthManager) -> Vec<String> {
        manager
            .grants_and_pats()
            .1
            .into_iter()
            .map(|g| g.id)
            .collect()
    }

    #[test]
    fn test_pkce_validation() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));

        assert!(verify_pkce(verifier, &expected_challenge));
        assert!(is_valid_code_challenge(&expected_challenge));
        assert!(!verify_pkce("wrong_verifier", &expected_challenge));
        assert!(!verify_pkce(&"a".repeat(43), &expected_challenge));
        assert!(!is_valid_code_challenge("short"));
    }

    #[test]
    fn test_redirect_uri_matching_rfc8252() {
        let registered = vec![
            "http://127.0.0.1:54321/callback".to_string(),
            "https://app.example.com/oauth/callback".to_string(),
            "cursor://anysphere.cursor-mcp/oauth/callback".to_string(),
        ];

        assert!(is_redirect_uri_allowed(
            &registered,
            "http://127.0.0.1:54321/callback"
        ));
        assert!(is_redirect_uri_allowed(
            &registered,
            "http://127.0.0.1:59123/callback"
        ));
        assert!(is_redirect_uri_allowed(
            &registered,
            "http://localhost:59123/callback"
        ));
        assert!(is_redirect_uri_allowed(
            &registered,
            "http://[::1]:59123/callback"
        ));
        assert!(!is_redirect_uri_allowed(
            &registered,
            "http://127.0.0.1:54321/callback#fragment"
        ));
        assert!(!is_redirect_uri_allowed(
            &registered,
            "http://127.0.0.1:59123/other"
        ));
        assert!(!is_redirect_uri_allowed(
            &registered,
            "http://127.0.0.1:59123/callback?x=1"
        ));
        assert!(!is_redirect_uri_allowed(
            &registered,
            "http://127.0.0.1@evil.example/callback"
        ));
        assert!(!is_redirect_uri_allowed(
            &registered,
            "http://localhost.evil.example/callback"
        ));
        assert!(is_redirect_uri_allowed(
            &registered,
            "https://app.example.com/oauth/callback"
        ));
        assert!(!is_redirect_uri_allowed(
            &registered,
            "https://app.example.com:8443/oauth/callback"
        ));
        assert!(!is_redirect_uri_allowed(
            &registered,
            "http://evil.com/callback"
        ));
        assert!(is_redirect_uri_allowed(
            &registered,
            "cursor://anysphere.cursor-mcp/oauth/callback"
        ));

        assert!(is_registrable_redirect_uri("http://localhost:3000/cb"));
        assert!(is_registrable_redirect_uri("http://[::1]:3000/cb"));
        assert!(is_registrable_redirect_uri(
            "https://claude.ai/api/mcp/auth_callback"
        ));
        assert!(is_registrable_redirect_uri("com.example.app:/oauth"));
        assert!(is_registrable_redirect_uri(
            "vscode://saoudrizwan.claude-dev/mcp-auth/callback/abc"
        ));
        assert!(!is_registrable_redirect_uri("http://example.com/cb"));
        assert!(!is_registrable_redirect_uri("javascript:alert(1)"));
        assert!(!is_registrable_redirect_uri("data:text/html,hi"));
        assert!(!is_registrable_redirect_uri(
            "search-ms:query=x&crumb=location:\\\\evil.example\\share"
        ));
        assert!(!is_registrable_redirect_uri("ms-msdt:/id"));
        assert!(!is_registrable_redirect_uri("https://example.com/cb#x"));
    }

    #[test]
    fn test_scope_normalization() {
        assert_eq!(normalize_scope("").as_deref(), Some("mcp:read mcp:write"));
        assert_eq!(
            normalize_scope("mcp:write  mcp:read").as_deref(),
            Some("mcp:read mcp:write")
        );
        assert_eq!(normalize_scope("mcp:read").as_deref(), Some("mcp:read"));
        assert_eq!(normalize_scope("mcp:read admin"), None);
        assert!(has_scope("mcp:read mcp:write", "mcp:write"));
        assert!(has_scope("mcp:write", "mcp:read"));
        assert!(!has_scope("mcp:read", "mcp:write"));
    }

    #[test]
    fn test_resource_matching() {
        assert!(is_own_resource(
            "http://127.0.0.1:8080/mcp",
            "http://127.0.0.1:8080"
        ));
        assert!(is_own_resource(
            "http://127.0.0.1:8080/mcp/",
            "http://127.0.0.1:8080"
        ));
        assert!(is_own_resource(
            "HTTPS://Music.Example.com:443/mcp",
            "https://music.example.com"
        ));
        assert!(!is_own_resource(
            "http://127.0.0.1:8080/",
            "http://127.0.0.1:8080"
        ));
        assert!(!is_own_resource(
            "https://other.example/mcp",
            "http://127.0.0.1:8080"
        ));
    }

    #[test]
    fn test_origin_prefers_public_url() {
        let mut config = Config {
            data_dir: PathBuf::new(),
            state_dir: None,
            static_dir: PathBuf::new(),
            port: 8080,
            host: "::1".to_string(),
            cookie_secure: false,
            trust_proxy: true,
            setup_token: None,
            public_url: None,
        };
        let mut headers = HeaderMap::new();
        assert_eq!(extract_origin(&headers, &config), "http://[::1]:8080");
        headers.insert("host", "Music.Example:8080".parse().unwrap());
        headers.insert(
            "x-forwarded-host",
            "spoofed.example, music.example.com".parse().unwrap(),
        );
        headers.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(
            extract_origin(&headers, &config),
            "https://music.example.com"
        );
        config.public_url = Some("https://tunes.example.org".to_string());
        assert_eq!(
            extract_origin(&headers, &config),
            "https://tunes.example.org"
        );
    }

    #[test]
    fn test_registration_is_in_memory_until_approved() {
        let (manager, temp_dir) = temp_manager();
        let path = temp_dir.join("mcp_auth.json");
        let client = register(&manager, "Claude Code");
        assert!(client.client_id.starts_with("tw_client_"));
        assert!(manager.get_client(&client.client_id).is_some());
        assert!(
            !path.exists(),
            "an unapproved registration must not touch disk"
        );

        manager.issue_tokens("usr_1", &client, "mcp:read").unwrap();
        let loaded = McpAuthManager::load(path);
        let found = loaded.get_client(&client.client_id).unwrap();
        assert_eq!(found.client_name, "Claude Code");
        assert_eq!(
            found.redirect_uris,
            vec!["http://127.0.0.1:54321/callback".to_string()]
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pending_clients_are_bounded() {
        let (manager, temp_dir) = temp_manager();
        let approved = register(&manager, "approved");
        manager
            .issue_tokens("usr_1", &approved, "mcp:read")
            .unwrap();
        let first_pending = register(&manager, "first pending");
        for i in 0..MAX_PENDING_CLIENTS {
            register(&manager, &format!("flood{i}"));
        }
        assert!(manager.get_client(&first_pending.client_id).is_none());
        assert!(manager.get_client(&approved.client_id).is_some());
        assert_eq!(
            manager.pending_clients.lock().unwrap().len(),
            MAX_PENDING_CLIENTS
        );
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_corrupt_store_is_moved_aside() {
        let temp_dir = std::env::temp_dir().join(format!("mcp_auth_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let path = temp_dir.join("mcp_auth.json");
        std::fs::write(&path, "{ not json").unwrap();

        let manager = McpAuthManager::load(path.clone());
        manager.create_pat("usr_1", "after", "mcp:read").unwrap();
        let kept: Vec<_> = std::fs::read_dir(&temp_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".invalid-"))
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(
            std::fs::read_to_string(kept[0].path()).unwrap(),
            "{ not json"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_registration_rate_limit_per_network() {
        let (manager, temp_dir) = temp_manager();
        let ip: IpAddr = "203.0.113.7".parse().unwrap();
        for _ in 0..REGISTRATIONS_PER_WINDOW {
            assert!(manager.allow_registration(ip));
        }
        assert!(!manager.allow_registration(ip));
        assert!(manager.allow_registration("203.0.113.8".parse().unwrap()));

        for i in 0..REGISTRATIONS_PER_WINDOW {
            let ip: IpAddr = format!("2001:db8:1:{i:x}::1").parse().unwrap();
            assert!(manager.allow_registration(ip));
        }
        assert!(
            !manager.allow_registration("2001:db8:1:ffff::1".parse().unwrap()),
            "one /48 shares a bucket"
        );
        assert!(manager.allow_registration("2001:db8:2::1".parse().unwrap()));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_pending_codes_capped_per_user() {
        let (manager, temp_dir) = temp_manager();
        let entry = |user: &str| AuthCodeEntry {
            client_id: "c".to_string(),
            user_id: user.to_string(),
            redirect_uri: "http://localhost/cb".to_string(),
            scope: "mcp:read".to_string(),
            code_challenge: "x".to_string(),
            created_at: Instant::now(),
        };
        for _ in 0..MAX_PENDING_CODES_PER_USER {
            manager.issue_authorization_code(entry("greedy")).unwrap();
        }
        assert!(manager.issue_authorization_code(entry("greedy")).is_err());
        let code = manager.issue_authorization_code(entry("other")).unwrap();
        assert!(manager.take_authorization_code(&code).is_some());
        assert!(manager.take_authorization_code(&code).is_none());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_refresh_rotation_and_reuse_detection() {
        let (manager, temp_dir) = temp_manager();
        let client = register(&manager, "Test Client");

        let (at1, rt1) = manager
            .issue_tokens("usr_123", &client, "mcp:read mcp:write")
            .unwrap();
        let rt1 = rt1.unwrap();
        assert_eq!(
            manager.validate_bearer_token(&at1).unwrap().user_id,
            "usr_123"
        );
        assert!(manager.validate_bearer_token(&rt1).is_none());
        let grant = grant_ids(&manager)[0].clone();
        assert_eq!(grant_id_of(&rt1), Some(grant.as_str()));

        assert_eq!(
            manager.refresh_tokens(&rt1, "tw_client_other").err(),
            Some(RefreshError::Invalid)
        );
        let second = manager.refresh_tokens(&rt1, &client.client_id).unwrap();
        assert!(manager.validate_bearer_token(&at1).is_none());
        assert!(manager
            .validate_bearer_token(&second.access_token)
            .is_some());

        let replay = manager.refresh_tokens(&rt1, &client.client_id).unwrap();
        assert_eq!(
            replay.refresh_token, second.refresh_token,
            "a retry within the grace window gets the same tokens"
        );

        let forged = format!("{REFRESH_PREFIX}{grant}.{}", "0".repeat(64));
        assert_eq!(
            manager.refresh_tokens(&forged, &client.client_id).err(),
            Some(RefreshError::Invalid)
        );
        assert_eq!(
            manager
                .refresh_tokens(&format!("{rt1}\n"), &client.client_id)
                .err(),
            Some(RefreshError::Invalid)
        );
        assert_eq!(
            grant_ids(&manager),
            vec![grant.clone()],
            "noise must not revoke"
        );

        for entry in manager.recent_refreshes.lock().unwrap().values_mut() {
            entry.at -= std::time::Duration::from_secs(REFRESH_REUSE_GRACE_SECS + 1);
        }

        let third = manager
            .refresh_tokens(&second.refresh_token, &client.client_id)
            .unwrap();
        assert_eq!(
            manager.recent_refreshes.lock().unwrap().len(),
            1,
            "expired replay entries are pruned on insert"
        );
        manager.recent_refreshes.lock().unwrap().clear();
        assert_eq!(
            manager.refresh_tokens(&rt1, &client.client_id).err(),
            Some(RefreshError::Reused)
        );
        assert!(grant_ids(&manager).is_empty());
        assert!(manager.validate_bearer_token(&third.access_token).is_none());
        assert!(manager
            .refresh_tokens(&third.refresh_token, &client.client_id)
            .is_err());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_simultaneous_refresh_with_one_token_keeps_the_grant() {
        let (manager, temp_dir) = temp_manager();
        let client = register(&manager, "Racer");
        for _ in 0..20 {
            let (_, rt) = manager.issue_tokens("usr_1", &client, "mcp:read").unwrap();
            let rt = rt.unwrap();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    let (manager, barrier, rt, client_id) = (
                        manager.clone(),
                        barrier.clone(),
                        rt.clone(),
                        client.client_id.clone(),
                    );
                    std::thread::spawn(move || {
                        barrier.wait();
                        manager.refresh_tokens(&rt, &client_id)
                    })
                })
                .collect();
            let outcomes: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
            let issued: Vec<_> = outcomes
                .into_iter()
                .map(|o| o.expect("neither racer may be refused or revoke the grant"))
                .collect();
            assert_eq!(issued[0].refresh_token, issued[1].refresh_token);
            assert!(manager
                .validate_bearer_token(&issued[0].access_token)
                .is_some());
        }
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_client_without_refresh_grant_gets_no_refresh_token() {
        let (manager, temp_dir) = temp_manager();
        let client = register_with(&manager, "Code only", &["authorization_code"]);
        let (access, refresh) = manager.issue_tokens("usr_1", &client, "mcp:read").unwrap();
        assert!(refresh.is_none());
        assert!(manager.validate_bearer_token(&access).is_some());
        let grant = &manager.grants_and_pats().1[0];
        assert!(grant.refresh_expires_at <= Utc::now() + Duration::seconds(ACCESS_TOKEN_TTL_SECS));
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_grants_are_independent_and_revocable() {
        let (manager, temp_dir) = temp_manager();
        let client = register(&manager, "Shared");
        let (at_a, rt_a) = manager.issue_tokens("usr_1", &client, "mcp:read").unwrap();
        let (at_b, _) = manager.issue_tokens("usr_1", &client, "mcp:read").unwrap();

        let refreshed_a = manager
            .refresh_tokens(&rt_a.unwrap(), &client.client_id)
            .unwrap();
        assert!(manager.validate_bearer_token(&at_a).is_none());
        assert!(manager.validate_bearer_token(&at_b).is_some());

        let grant_a = grant_id_of(&refreshed_a.refresh_token).unwrap().to_string();
        assert_eq!(manager.owner_of(&grant_a).as_deref(), Some("usr_1"));
        assert!(manager.revoke(&grant_a).unwrap());
        assert!(manager
            .validate_bearer_token(&refreshed_a.access_token)
            .is_none());
        assert!(manager.validate_bearer_token(&at_b).is_some());
        assert!(!manager.revoke(&grant_a).unwrap());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_usage_is_persisted_and_survives_other_writes() {
        let (manager, temp_dir) = temp_manager();
        let (raw, pat) = manager.create_pat("usr_1", "CLI", "mcp:read").unwrap();
        assert!(manager.validate_bearer_token(&raw).is_some());
        manager
            .create_pat("usr_1", "unrelated write", "mcp:read")
            .unwrap();
        let reloaded = McpAuthManager::load(temp_dir.join("mcp_auth.json"));
        let stored = reloaded.grants_and_pats().2;
        assert!(stored
            .iter()
            .find(|p| p.id == pat.id)
            .unwrap()
            .last_used_at
            .is_some());

        manager
            .usage
            .lock()
            .unwrap()
            .get_mut(&pat.id)
            .unwrap()
            .saved = Some(Utc::now() - Duration::minutes(USAGE_PERSIST_MINUTES + 1));
        let before = std::fs::metadata(temp_dir.join("mcp_auth.json"))
            .unwrap()
            .modified()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(manager.validate_bearer_token(&raw).is_some());
        let after = std::fs::metadata(temp_dir.join("mcp_auth.json"))
            .unwrap()
            .modified()
            .unwrap();
        assert!(
            after > before,
            "a token in constant use is still saved periodically"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_create_and_revoke_pat() {
        let (manager, temp_dir) = temp_manager();
        let (raw_pat, pat) = manager.create_pat("usr_123", "Cursor", "mcp:read").unwrap();
        assert!(raw_pat.starts_with("tw_pat_"));
        assert_eq!(pat.name, "Cursor");

        let validated = manager.validate_bearer_token(&raw_pat).unwrap();
        assert_eq!(validated.user_id, "usr_123");
        assert_eq!(validated.scope, "mcp:read");

        assert!(manager.revoke(&pat.id).unwrap());
        assert!(manager.validate_bearer_token(&raw_pat).is_none());

        let (second, _) = manager.create_pat("usr_123", "Other", "mcp:read").unwrap();
        let client = register(&manager, "c");
        let (at, _) = manager
            .issue_tokens("usr_123", &client, "mcp:read")
            .unwrap();
        manager.revoke_user("usr_123").unwrap();
        assert!(manager.validate_bearer_token(&second).is_none());
        assert!(manager.validate_bearer_token(&at).is_none());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
