use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub static_dir: PathBuf,
    pub port: u16,
    pub host: String,
    pub auth_enabled: bool,
    pub username: String,
    pub password: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            data_dir: PathBuf::from(
                std::env::var("TAGSTUDIO_DATA_DIR").unwrap_or_else(|_| "./data".to_string()),
            ),
            static_dir: PathBuf::from(
                std::env::var("TAGSTUDIO_STATIC_DIR")
                    .unwrap_or_else(|_| "./frontend/build".to_string()),
            ),
            port: std::env::var("TAGSTUDIO_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            host: std::env::var("TAGSTUDIO_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            auth_enabled: std::env::var("TAGSTUDIO_AUTH_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            username: std::env::var("TAGSTUDIO_USERNAME").unwrap_or_else(|_| "admin".to_string()),
            password: std::env::var("TAGSTUDIO_PASSWORD")
                .unwrap_or_else(|_| "changeme".to_string()),
        }
    }
}
