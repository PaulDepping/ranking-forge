use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    state::AppState,
};

#[derive(Deserialize)]
pub struct GamesQuery {
    pub q: String,
}

#[derive(Serialize)]
pub struct GameResponse {
    pub id: i64,
    pub name: String,
    pub display_name: Option<String>,
}

pub async fn search_games(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Query(params): Query<GamesQuery>,
) -> Result<impl IntoResponse> {
    if params.q.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("q must not be empty".into()));
    }

    let games = state
        .startgg
        .search_games(&params.q)
        .await?
        .into_iter()
        .map(|g| GameResponse { id: g.id, name: g.name, display_name: g.display_name })
        .collect::<Vec<_>>();

    Ok(Json(games))
}
