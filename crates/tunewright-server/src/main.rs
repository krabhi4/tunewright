mod auth;
mod config;
mod error;
mod routes;
mod state;
mod users;

use config::Config;
use state::AppState;
use tracing_subscriber::EnvFilter;
use users::UserManager;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let config = Config::from_env();

    tracing::info!("Tunewright v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Data directory: {:?}", config.data_dir);
    tracing::info!("Static directory: {:?}", config.static_dir);

    // Ensure data directory exists
    if !config.data_dir.exists() {
        tracing::warn!("Data directory does not exist: {:?}", config.data_dir);
        std::fs::create_dir_all(&config.data_dir).expect("Failed to create data directory");
    }

    let users_path = config.data_dir.join("users.json");
    let users = UserManager::load(users_path);
    tracing::info!("Setup required: {}", !users.has_users());

    // Setup-window exposure warning: with no users yet, /auth/setup hands the
    // first caller the super-admin account. Binding beyond loopback without a
    // setup token leaves that window open to the whole network.
    let is_loopback = matches!(
        config.host.as_str(),
        "127.0.0.1" | "localhost" | "::1" | "[::1]"
    );
    if !users.has_users() && !is_loopback && config.setup_token.is_none() {
        tracing::warn!(
            "SECURITY: listening on non-loopback address {} while initial setup is incomplete. \
             Anyone who can reach this port can claim the admin account via /api/v1/auth/setup. \
             Set TUNEWRIGHT_SETUP_TOKEN to require a token during setup, or bind TUNEWRIGHT_HOST \
             to 127.0.0.1 until setup is complete.",
            config.host
        );
    }

    let host = if config.host.contains(':') && !config.host.starts_with('[') {
        format!("[{}]", config.host)
    } else {
        config.host.clone()
    };
    let bind_addr = format!("{}:{}", host, config.port);
    let state = AppState::new(config, users);
    let app = routes::create_router(state);

    let listener = match tokio::net::TcpListener::bind(&bind_addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind to address {}: {}", bind_addr, e);
            std::process::exit(1);
        }
    };

    tracing::info!("Listening on http://{}", bind_addr);

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
