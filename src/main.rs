mod app;
mod config;
mod handlers;
mod models;
mod services;
mod utils;

use std::time::Instant;
use tokio::signal;
use tracing::{error, info};

use crate::app::create_app;
use crate::config::Config;
use crate::services::create_app_state;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize logging
    utils::logger::init_logger()?;

    info!("Starting S3 CDN Network server...");

    let start_time = Instant::now();

    // Load and validate configuration
    let config = Config::from_env().map_err(|e| {
        error!("Configuration error: {}", e);
        e
    })?;

    // Create application state
    let state = create_app_state(&config.cache, &config.database, &config.auth).await?;

    // Create the application
    let app = create_app(state);

    // Bind to address
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
        error!("Failed to bind to {}: {}", addr, e);
        e
    })?;

    info!("Server bound to {}", addr);
    let _local_addr = listener.local_addr().map_err(|e| {
        error!("Failed to get local address: {}", e);
        Box::<dyn std::error::Error + Send + Sync>::from(e)
    })?;
    info!("Server startup completed in {:?}", start_time.elapsed());

    // Start the server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| {
            error!("Server error: {}", e);
            Box::<dyn std::error::Error + Send + Sync>::from(e)
        })?;

    info!("Server shutdown complete");
    Ok(())
}

/// Handle graceful shutdown signals
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown...");
}
