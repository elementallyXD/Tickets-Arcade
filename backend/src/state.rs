//! Application State
//!
//! Shared state passed to all Axum handlers via the [`axum::extract::State`] extractor.
//! Contains the database pool and validated configuration.

use crate::config::AppConfig;

/// Shared application state for Axum handlers.
///
/// This struct is cloned for each request handler. Both [`sqlx::PgPool`] and
/// [`AppConfig`] are internally reference-counted, so cloning is cheap.
///
/// # Example
/// ```ignore
/// async fn my_handler(State(state): State<AppState>) -> impl IntoResponse {
///     let rows = sqlx::query("SELECT 1").fetch_all(&state.db).await?;
///     // ...
/// }
/// ```
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool.
    pub db: sqlx::PgPool,

    /// Application configuration loaded from environment.
    pub config: AppConfig,
}
