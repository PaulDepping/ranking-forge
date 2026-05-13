pub mod auth;
pub mod games;
pub mod import;
pub mod players;
pub mod projects;
pub mod tournaments;

use axum::{Router, routing::get};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/projects", projects::router())
        .route("/games", get(games::search_games))
}
