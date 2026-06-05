use crate::config::Config;
use crate::users::{Role, UserManager};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const SESSION_TTL_SECS: u64 = 86400; // 24 hours

#[derive(Debug, Clone)]
pub struct Session {
    pub user_id: String,
    pub username: String,
    pub role: Role,
    pub created_at: Instant,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Config,
    pub data_root: PathBuf,
    pub users: UserManager,
    pub sessions: Arc<Mutex<HashMap<String, Session>>>,
    pub musicbrainz_next_allowed: Arc<Mutex<Instant>>,
    pub failed_logins: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// Shared HTTP client for external lookups; reuses the connection pool
    /// across MusicBrainz/Apple Music requests (it is internally `Arc`-backed).
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config, users: UserManager) -> Self {
        let data_root = config.data_dir.clone();
        Self {
            config,
            data_root,
            users,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            musicbrainz_next_allowed: Arc::new(Mutex::new(Instant::now())),
            failed_logins: Arc::new(Mutex::new(HashMap::new())),
            http_client: reqwest::Client::new(),
        }
    }

    pub fn add_session(&self, token: String, session: Session) {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.insert(token, session);
    }

    pub fn get_session(&self, token: &str) -> Option<Session> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.retain(|_, s| s.created_at.elapsed().as_secs() < SESSION_TTL_SECS);
        sessions.get(token).cloned()
    }

    pub fn remove_session(&self, token: &str) {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        sessions.remove(token);
    }
}
