use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub static_dir: PathBuf,
    pub port: u16,
    pub host: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            data_dir: PathBuf::from(
                std::env::var("TUNEWRIGHT_DATA_DIR").unwrap_or_else(|_| "./data".to_string()),
            ),
            static_dir: PathBuf::from(
                std::env::var("TUNEWRIGHT_STATIC_DIR")
                    .unwrap_or_else(|_| "./frontend/build".to_string()),
            ),
            port: std::env::var("TUNEWRIGHT_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            host: std::env::var("TUNEWRIGHT_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
        }
    }
}
