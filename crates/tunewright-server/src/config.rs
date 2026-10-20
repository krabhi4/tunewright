use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: PathBuf,
    pub state_dir: Option<PathBuf>,
    pub static_dir: PathBuf,
    pub port: u16,
    pub host: String,
    pub cookie_secure: bool,
    pub trust_proxy: bool,
    /// Optional token required by /auth/setup to claim the first admin
    /// account. Protects the setup window on network-exposed deployments.
    pub setup_token: Option<String>,
    pub public_url: Option<String>,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            data_dir: PathBuf::from(
                std::env::var("TUNEWRIGHT_DATA_DIR").unwrap_or_else(|_| "./data".to_string()),
            ),
            state_dir: std::env::var("TUNEWRIGHT_STATE_DIR")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from),
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
            cookie_secure: env_flag("TUNEWRIGHT_COOKIE_SECURE"),
            trust_proxy: env_flag("TUNEWRIGHT_TRUST_PROXY"),
            setup_token: std::env::var("TUNEWRIGHT_SETUP_TOKEN")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            public_url: public_url_from_env(),
        }
    }
}

fn public_url_from_env() -> Option<String> {
    let raw = std::env::var("TUNEWRIGHT_PUBLIC_URL").ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    match reqwest::Url::parse(raw) {
        Ok(url)
            if matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none() =>
        {
            Some(url.origin().ascii_serialization())
        }
        _ => {
            tracing::warn!(
                "TUNEWRIGHT_PUBLIC_URL '{raw}' must be an http(s) origin such as https://music.example.com, ignoring it"
            );
            None
        }
    }
}

fn env_flag(name: &str) -> bool {
    let Ok(v) = std::env::var(name) else {
        return false;
    };
    let v = v.trim();
    let on = v == "1" || v.eq_ignore_ascii_case("true");
    if !on && !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false") {
        tracing::warn!("{name} value '{v}' is not true/false/1/0, treating it as false");
    }
    on
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        // Clear env vars to test defaults
        std::env::remove_var("TUNEWRIGHT_HOST");
        std::env::remove_var("TUNEWRIGHT_COOKIE_SECURE");
        std::env::remove_var("TUNEWRIGHT_TRUST_PROXY");

        let config = Config::from_env();
        assert_eq!(config.host, "127.0.0.1");
        assert!(!config.cookie_secure);
        assert!(!config.trust_proxy);

        // Test custom values from env
        std::env::set_var("TUNEWRIGHT_HOST", "192.168.1.50");
        std::env::set_var("TUNEWRIGHT_COOKIE_SECURE", "true");
        std::env::set_var("TUNEWRIGHT_TRUST_PROXY", "1");
        let config2 = Config::from_env();
        assert_eq!(config2.host, "192.168.1.50");
        assert!(config2.cookie_secure);
        assert!(config2.trust_proxy);

        // Cleanup env
        std::env::remove_var("TUNEWRIGHT_HOST");
        std::env::remove_var("TUNEWRIGHT_COOKIE_SECURE");
        std::env::remove_var("TUNEWRIGHT_TRUST_PROXY");
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

    #[test]
    fn test_public_url_from_env() {
        for (raw, expected) in [
            (
                "https://Music.Example.com/",
                Some("https://music.example.com"),
            ),
            ("http://192.168.1.5:8080", Some("http://192.168.1.5:8080")),
            (
                "https://music.example.com:443",
                Some("https://music.example.com"),
            ),
            ("https://music.example.com/tunewright", None),
            ("ftp://music.example.com", None),
            ("not a url", None),
            ("  ", None),
        ] {
            std::env::set_var("TUNEWRIGHT_PUBLIC_URL", raw);
            assert_eq!(public_url_from_env().as_deref(), expected, "{raw}");
        }
        std::env::remove_var("TUNEWRIGHT_PUBLIC_URL");
    }
}
