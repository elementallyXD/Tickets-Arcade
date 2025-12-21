use crate::config::AppConfig;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: AppConfig,
}
