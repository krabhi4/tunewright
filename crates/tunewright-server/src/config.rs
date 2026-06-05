use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub static_dir: PathBuf,
    pub port: u16,
    pub host: String,
    pub cookie_secure: bool,
    /// Optional token required by /auth/setup to claim the first admin
    /// account. Protects the setup window on network-exposed deployments.
    pub setup_token: Option<String>,
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
            port: {
                if let Ok(p_str) = std::env::var("TUNEWRIGHT_PORT") {
                    match p_str.parse::<u16>() {
                        Ok(p) => p,
                        Err(e) => {
                            tracing::warn!(
                                "TUNEWRIGHT_PORT environment variable '{}' is invalid ({}), falling back to 8080",
                                p_str,
                                e
                            );
                            8080
                        }
                    }
                } else {
                    8080
                }
            },
            host: std::env::var("TUNEWRIGHT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            cookie_secure: std::env::var("TUNEWRIGHT_COOKIE_SECURE")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            setup_token: std::env::var("TUNEWRIGHT_SETUP_TOKEN")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        // Clear env vars to test defaults
        std::env::remove_var("TUNEWRIGHT_HOST");
        std::env::remove_var("TUNEWRIGHT_COOKIE_SECURE");

        let config = Config::from_env();
        assert_eq!(config.host, "127.0.0.1");
        assert!(!config.cookie_secure);

        // Test custom values from env
        std::env::set_var("TUNEWRIGHT_HOST", "192.168.1.50");
        std::env::set_var("TUNEWRIGHT_COOKIE_SECURE", "true");
        let config2 = Config::from_env();
        assert_eq!(config2.host, "192.168.1.50");
        assert!(config2.cookie_secure);

        // Cleanup env
        std::env::remove_var("TUNEWRIGHT_HOST");
        std::env::remove_var("TUNEWRIGHT_COOKIE_SECURE");
    }

    #[test]
    fn test_setup_token_from_env() {
        std::env::remove_var("TUNEWRIGHT_SETUP_TOKEN");
        let config = Config::from_env();
        assert!(config.setup_token.is_none());

        std::env::set_var("TUNEWRIGHT_SETUP_TOKEN", "sekret123");
        let config2 = Config::from_env();
        assert_eq!(config2.setup_token.as_deref(), Some("sekret123"));

        // Empty/whitespace value behaves as unset
        std::env::set_var("TUNEWRIGHT_SETUP_TOKEN", "  ");
        let config3 = Config::from_env();
        assert!(config3.setup_token.is_none());

        std::env::remove_var("TUNEWRIGHT_SETUP_TOKEN");
    }

    #[test]
    fn test_invalid_port_falls_back_to_default() {
        // Garbage string
        std::env::set_var("TUNEWRIGHT_PORT", "not_a_port");
        let config = Config::from_env();
        assert_eq!(config.port, 8080);

        // Out-of-range value
        std::env::set_var("TUNEWRIGHT_PORT", "99999");
        let config2 = Config::from_env();
        assert_eq!(config2.port, 8080);

        // Cleanup
        std::env::remove_var("TUNEWRIGHT_PORT");
    }
}
