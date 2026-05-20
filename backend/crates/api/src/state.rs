use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cors_origin: String,
    pub startgg_base_url: String,
}
