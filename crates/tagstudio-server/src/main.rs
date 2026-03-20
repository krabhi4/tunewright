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

    tracing::info!("TagStudio v{}", env!("CARGO_PKG_VERSION"));
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

    let bind_addr = format!("{}:{}", config.host, config.port);
    let state = AppState::new(config, users);
    let app = routes::create_router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("Listening on http://{}", bind_addr);

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
