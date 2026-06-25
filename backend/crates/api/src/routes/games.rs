use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::{error::Result, routes::auth::AuthUser, state::AppState};

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
}

pub async fn search_games(
    State(state): State<AppState>,
    _user: AuthUser,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse> {
    if params.q.trim().is_empty() {
        return Err(crate::error::AppError::UnprocessableEntity(
            "query must not be empty".into(),
        ));
    }

    let pattern = format!("%{}%", params.q);
    let games = sqlx::query!(
        "SELECT startgg_id AS id, name FROM global_games WHERE name ILIKE $1 ORDER BY name LIMIT 20",
        pattern,
    )
    .fetch_all(&state.db)
    .await?;

    #[derive(Serialize)]
    struct GameResult {
        id: i64,
        name: String,
    }

    let results: Vec<GameResult> = games
        .into_iter()
        .map(|r| GameResult {
            id: r.id,
            name: r.name,
        })
        .collect();
    Ok(Json(results))
}
