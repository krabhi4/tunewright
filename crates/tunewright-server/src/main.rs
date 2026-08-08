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

    let mut config = Config::from_env();

    tracing::info!("Tunewright v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Data directory: {:?}", config.data_dir);
    tracing::info!("Static directory: {:?}", config.static_dir);

    // Ensure data directory exists
    if !config.data_dir.exists() {
        tracing::warn!("Data directory does not exist: {:?}", config.data_dir);
        std::fs::create_dir_all(&config.data_dir).expect("Failed to create data directory");
    }

    // Canonicalize the data root once; every path-safety check compares
    // resolved paths against this canonical root.
    config.data_dir = config
        .data_dir
        .canonicalize()
        .expect("Failed to canonicalize data directory");

    let users_path = config.data_dir.join("users.json");
    let users = UserManager::load(users_path);
    tracing::info!("Setup required: {}", !users.has_users());

    let is_loopback = matches!(
        config.host.as_str(),
        "127.0.0.1" | "localhost" | "::1" | "[::1]"
    );
    if !users.has_users() && !is_loopback && config.setup_token.is_none() {
        let generated: [u8; 16] = rand::random();
        let generated = hex::encode(generated);
        // Printed directly as well as logged: at RUST_LOG=error the tracing
        // line is dropped and the operator would have no way to finish setup.
        eprintln!(
            "\nSECURITY: initial setup is incomplete and {} is not a loopback address.\n\
             A one-time setup token has been generated. Enter it on the setup screen:\n\
             \n    Setup token: {}\n\n\
             Set TUNEWRIGHT_SETUP_TOKEN to choose your own, or bind TUNEWRIGHT_HOST to\n\
             127.0.0.1 to disable the requirement.\n",
            config.host, generated
        );
        tracing::warn!(
            "SECURITY: listening on non-loopback address {} while initial setup is incomplete. \
             No TUNEWRIGHT_SETUP_TOKEN was set, so a one-time token has been generated to stop \
             anyone on the network claiming the admin account via /api/v1/auth/setup.\n\
             \n    Setup token: {}\n\n\
             Enter it on the setup screen. Set TUNEWRIGHT_SETUP_TOKEN explicitly to choose your \
             own, or bind TUNEWRIGHT_HOST to 127.0.0.1 to disable the requirement.",
            config.host,
            generated
        );
        config.setup_token = Some(generated);
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
