mod config;
mod api;
mod indexer;
mod state;

use axum::{http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    let config = config::AppConfig::from_env()?;
    tracing::info!(
        chain_id = config.chain_id,
        randomness_provider_address = ?config.randomness_provider_address,
        "config loaded"
    );
    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;

    let addr: SocketAddr = config.bind_addr.parse()?;
    let app_state = AppState {
        db: db_pool,
        config,
    };
    let indexer_db = app_state.db.clone();
    let indexer_config = app_state.config.clone();
    // Run the indexer in the background alongside the API server.
    tokio::spawn(async move {
        if let Err(err) = indexer::run(indexer_db, indexer_config).await {
            tracing::error!(error = %err, "indexer stopped");
        }
    });
    let app = Router::<AppState>::new()
        .route("/health", get(health))
        .nest("/v1", api::router())
        .with_state(app_state);

    let listener = TcpListener::bind(addr).await?;
    tracing::info!(%addr, "backend listening");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> impl IntoResponse {
    let body = json!({ "status": "ok" });
    (StatusCode::OK, Json(body))
}
