//! Ticket Arcade Backend
//!
//! A Rust backend that indexes on-chain events from the Arc L1 blockchain
//! and exposes REST APIs for the React frontend.
//!
//! # Architecture
//! - **Indexer**: Polls RPC for contract events, stores in PostgreSQL
//! - **API Server**: Axum-based REST API serving indexed data
//!
//! # Running
//! ```bash
//! # Set up environment
//! cp .env.example .env
//! # Start Postgres
//! docker compose up -d
//! # Run migrations
//! sqlx migrate run --source migrations
//! # Run the backend
//! cargo run
//! ```

mod api;
mod config;
mod indexer;
mod state;

use axum::{Json, Router, http::StatusCode, response::IntoResponse, routing::get};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use state::AppState;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::EnvFilter;

/// Database connection pool timeout
const DB_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file (ignore errors if not present)
    dotenvy::dotenv().ok();

    // Initialize tracing with environment filter
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    // Load and validate configuration
    let config = config::AppConfig::from_env()?;
    tracing::info!(
        chain_id = config.chain_id,
        start_block = config.start_block,
        "configuration loaded"
    );

    // Create database connection pool with timeout
    let db_pool = tokio::time::timeout(
        DB_CONNECT_TIMEOUT,
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&config.database_url),
    )
    .await
    .map_err(|_| anyhow::anyhow!("database connection timed out"))?
    .map_err(|e| anyhow::anyhow!("failed to connect to database: {}", e))?;

    tracing::info!("database connection established");

    // Parse bind address
    let addr: SocketAddr = config
        .bind_addr
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid BIND_ADDR: {}", e))?;

    // Create shared application state
    let app_state = AppState {
        db: db_pool.clone(),
        config: config.clone(),
    };

    // Spawn indexer in background task
    let indexer_db = db_pool.clone();
    let indexer_config = config.clone();
    let indexer_handle = tokio::spawn(async move {
        if let Err(err) = indexer::run(indexer_db, indexer_config).await {
            tracing::error!(error = %err, "indexer stopped with error");
        }
    });

    // Build API router
    let app = Router::<AppState>::new()
        .route("/health", get(health_check))
        .nest("/v1", api::router())
        .with_state(app_state);

    // Start HTTP server
    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "backend listening");

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Clean shutdown
    tracing::info!("shutting down...");
    indexer_handle.abort();
    db_pool.close().await;
    tracing::info!("shutdown complete");

    Ok(())
}

/// Health check endpoint
///
/// Returns 200 OK with JSON body `{"status": "ok"}`.
/// Used by load balancers and monitoring systems.
async fn health_check() -> impl IntoResponse {
    let body = json!({ "status": "ok" });
    (StatusCode::OK, Json(body))
}

/// Waits for shutdown signals (Ctrl+C or SIGTERM)
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

    tracing::info!("shutdown signal received");
}
