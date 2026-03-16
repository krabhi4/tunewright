mod auth;
mod config;
mod error;
mod routes;
mod state;

use config::Config;
use state::AppState;
use tracing_subscriber::EnvFilter;

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
    tracing::info!("Auth enabled: {}", config.auth_enabled);

    // Ensure data directory exists
    if !config.data_dir.exists() {
        tracing::warn!("Data directory does not exist: {:?}", config.data_dir);
        std::fs::create_dir_all(&config.data_dir).expect("Failed to create data directory");
    }

    let bind_addr = format!("{}:{}", config.host, config.port);
    let state = AppState::new(config);
    let app = routes::create_router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("Listening on http://{}", bind_addr);

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
