pub mod auth;

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().nest("/auth", auth::router())
}
