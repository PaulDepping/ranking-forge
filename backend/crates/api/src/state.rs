use common::startgg::StartggClient;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub startgg: StartggClient,
    pub cors_origin: String,
}
