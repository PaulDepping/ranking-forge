pub mod auth;
pub mod games;
pub mod import;
pub mod invite_links;
pub mod members;
pub mod players;
pub mod projects;
pub mod tournaments;

use axum::{Router, routing::{get, post}};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/projects", projects::router())
        .route("/games", get(games::search_games))
        .route("/invite/{token}/accept", post(invite_links::accept_invite_link))
}
