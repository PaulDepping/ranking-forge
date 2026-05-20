use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    state::AppState,
};
use common::startgg::StartggClient;

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
    AuthUser(user): AuthUser,
    Query(params): Query<GamesQuery>,
) -> Result<impl IntoResponse> {
    if params.q.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("q must not be empty".into()));
    }
    let api_key = user.startgg_api_key.ok_or_else(|| {
        AppError::UnprocessableEntity(
            "Configure a start.gg API key in account settings before searching".into(),
        )
    })?;
    let client = StartggClient::new_with_base_url(api_key, state.startgg_base_url.clone());
    let games = client
        .search_games(&params.q)
        .await?
        .into_iter()
        .map(|g| GameResponse {
            id: g.id,
            name: g.name,
            display_name: g.display_name,
        })
        .collect::<Vec<_>>();
    Ok(Json(games))
}
