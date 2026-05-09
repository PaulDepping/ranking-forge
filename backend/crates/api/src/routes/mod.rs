pub mod auth;
pub mod games;
pub mod players;
pub mod projects;

use axum::{routing::get, Router};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/projects", projects::router())
        .route("/games", get(games::search_games))
}
