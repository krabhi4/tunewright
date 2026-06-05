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

pub fn is_allowed_cover_host(host: &str) -> bool {
    let host = host.trim_end_matches('.');
    host == "coverartarchive.org"
        || host == "archive.org"
        || host.ends_with(".archive.org")
        || host == "mzstatic.com"
        || host.ends_with(".mzstatic.com")
}

pub fn is_allowed_cover_host_safe(host: &str) -> bool {
    if host.is_empty() {
        return false;
    }
    if host.parse::<std::net::IpAddr>().is_ok() || (host.starts_with('[') && host.ends_with(']')) {
        return false;
    }
    is_allowed_cover_host(host)
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
    /// Shared dedicated client for cover art requests, with restricted redirects.
    pub coverart_client: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config, users: UserManager) -> Self {
        let data_root = config.data_dir.clone();
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let coverart_client = reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (compatible; Tunewright/0.5.1; +https://github.com/tunewright)",
            )
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let host = attempt.url().host_str().unwrap_or("");
                if attempt.previous().len() > 5 {
                    attempt.stop()
                } else if is_allowed_cover_host_safe(host) {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            config,
            data_root,
            users,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            musicbrainz_next_allowed: Arc::new(Mutex::new(Instant::now())),
            failed_logins: Arc::new(Mutex::new(HashMap::new())),
            http_client,
            coverart_client,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::users::UserManager;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    #[tokio::test]
    async fn test_http_client_timeout() {
        // Start a local TCP listener that accepts connection but never responds
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        // Spawn a thread to accept the connection but hang
        thread::spawn(move || {
            if let Ok((_stream, _)) = listener.accept() {
                // Just sleep and do nothing, keeping connection open
                thread::sleep(Duration::from_secs(12));
            }
        });

        // Initialize state
        let config = Config {
            data_dir: std::env::temp_dir(),
            static_dir: std::env::temp_dir(),
            port: 8080,
            host: "127.0.0.1".to_string(),
            cookie_secure: false,
            setup_token: None,
        };
        let users =
            UserManager::load(std::env::temp_dir().join(format!("users_{}.json", rand_num())));
        let state = AppState::new(config, users);

        let url = format!("http://127.0.0.1:{}", port);
        let start = Instant::now();
        let result = state.http_client.get(&url).send().await;

        assert!(result.is_err(), "Expected request to fail due to timeout");
        let elapsed = start.elapsed();
        // Since timeout is 10s, it should fail around 10s, and definitely before 12s.
        assert!(
            elapsed >= Duration::from_secs(9),
            "Elapsed time too short: {:?}",
            elapsed
        );
        assert!(
            elapsed < Duration::from_secs(12),
            "Elapsed time too long: {:?}",
            elapsed
        );
    }

    fn rand_num() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    #[test]
    fn test_cover_host_allowlist() {
        assert!(is_allowed_cover_host_safe("coverartarchive.org"));
        assert!(is_allowed_cover_host_safe("ia800201.us.archive.org"));
        assert!(is_allowed_cover_host_safe("mzstatic.com"));
        assert!(is_allowed_cover_host_safe("is1-ssl.mzstatic.com"));
        // IP literals must be rejected
        assert!(!is_allowed_cover_host_safe("192.168.1.1"));
        assert!(!is_allowed_cover_host_safe("127.0.0.1"));
        assert!(!is_allowed_cover_host_safe("::1"));
        assert!(!is_allowed_cover_host_safe("[::1]"));
        assert!(!is_allowed_cover_host_safe(""));
        // Arbitrary domains not on allowlist
        assert!(!is_allowed_cover_host_safe("evil.com"));
        assert!(!is_allowed_cover_host_safe("archive.org.evil.com"));
    }
}
