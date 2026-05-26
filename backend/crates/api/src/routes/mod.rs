pub mod account;
pub mod auth;
pub mod games;
pub mod health;
pub mod import;
pub mod invite_links;
pub mod members;
pub mod players;
pub mod projects;
pub mod tournaments;

use crate::state::AppState;
use axum::{
    Router,
    routing::{get, post},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .nest("/auth", auth::router())
        .nest("/account", account::router())
        .nest("/projects", projects::router())
        .route("/games", get(games::search_games))
        .route(
            "/invite/{token}/accept",
            post(invite_links::accept_invite_link),
        )
}
