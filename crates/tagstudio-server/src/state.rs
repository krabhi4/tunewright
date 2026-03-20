use crate::config::Config;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const SESSION_TTL_SECS: u64 = 86400; // 24 hours

#[derive(Debug, Clone)]
pub struct AppState {
    pub config: Config,
    pub data_root: PathBuf,
    pub sessions: Arc<Mutex<HashMap<String, Instant>>>,
    pub musicbrainz_next_allowed: Arc<Mutex<Instant>>,
    pub failed_logins: Arc<Mutex<(u32, Instant)>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let data_root = config.data_dir.clone();
        Self {
            config,
            data_root,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            musicbrainz_next_allowed: Arc::new(Mutex::new(Instant::now())),
            failed_logins: Arc::new(Mutex::new((0, Instant::now()))),
        }
    }

    pub fn is_session_valid(&self, token: &str) -> bool {
        if let Ok(mut sessions) = self.sessions.lock() {
            // Lazy prune expired tokens
            sessions.retain(|_, created| created.elapsed().as_secs() < SESSION_TTL_SECS);

            sessions
                .get(token)
                .map(|created| created.elapsed().as_secs() < SESSION_TTL_SECS)
                .unwrap_or(false)
        } else {
            false
        }
    }

    pub fn add_session(&self, token: String) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.insert(token, Instant::now());
        }
    }

    pub fn remove_session(&self, token: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(token);
        }
    }
}
