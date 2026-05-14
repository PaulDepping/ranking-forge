use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    routes::projects::require_project,
    state::AppState,
};
use common::models::{Player, StartggAccount};

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreatePlayerRequest {
    name: String,
}

#[derive(Deserialize)]
struct LinkAccountRequest {
    slug: String,
}

#[derive(Serialize)]
pub struct AccountResponse {
    pub id: Uuid,
    pub startgg_user_id: i64,
    pub slug: String,
    pub display_name: Option<String>,
}

impl From<StartggAccount> for AccountResponse {
    fn from(a: StartggAccount) -> Self {
        AccountResponse {
            id: a.id,
            startgg_user_id: a.startgg_user_id,
            slug: a.slug.unwrap_or_default(),
            display_name: a.display_name,
        }
    }
}

#[derive(Serialize)]
pub struct PlayerResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub accounts: Vec<AccountResponse>,
}

// ── Path param structs ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ProjectPlayerPath {
    id: Uuid,
    pid: Uuid,
}

#[derive(Deserialize)]
struct ProjectPlayerAccountPath {
    id: Uuid,
    pid: Uuid,
    aid: Uuid,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn list_players(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

    let players = sqlx::query_as!(
        Player,
        "SELECT id, project_id, name, created_at
         FROM players
         WHERE project_id = $1
         ORDER BY created_at ASC",
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    let player_ids: Vec<Uuid> = players.iter().map(|p| p.id).collect();

    let accounts = if player_ids.is_empty() {
        vec![]
    } else {
        sqlx::query_as!(
            StartggAccount,
            "SELECT id, player_id, startgg_user_id, slug, display_name, created_at
             FROM startgg_accounts
             WHERE player_id = ANY($1)",
            &player_ids as &[Uuid],
        )
        .fetch_all(&state.db)
        .await?
    };

    let mut accounts_map: HashMap<Uuid, Vec<AccountResponse>> = HashMap::new();
    for account in accounts {
        accounts_map
            .entry(account.player_id)
            .or_default()
            .push(AccountResponse::from(account));
    }

    let resp: Vec<PlayerResponse> = players
        .into_iter()
        .map(|p| {
            let accounts = accounts_map.remove(&p.id).unwrap_or_default();
            PlayerResponse {
                id: p.id,
                project_id: p.project_id,
                name: p.name,
                created_at: p.created_at,
                accounts,
            }
        })
        .collect();

    Ok(Json(resp))
}

async fn add_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreatePlayerRequest>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity(
            "name must not be empty".into(),
        ));
    }

    let player = sqlx::query_as!(
        Player,
        "INSERT INTO players (project_id, name) VALUES ($1, $2)
         RETURNING id, project_id, name, created_at",
        project_id,
        body.name,
    )
    .fetch_one(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(PlayerResponse {
            id: player.id,
            project_id: player.project_id,
            name: player.name,
            created_at: player.created_at,
            accounts: vec![],
        }),
    ))
}

async fn delete_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<ProjectPlayerPath>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, path.id, user.id).await?;

    let result = sqlx::query!(
        "DELETE FROM players WHERE id = $1 AND project_id = $2",
        path.pid,
        path.id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn link_account(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<ProjectPlayerPath>,
    Json(body): Json<LinkAccountRequest>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, path.id, user.id).await?;

    sqlx::query!(
        "SELECT id FROM players WHERE id = $1 AND project_id = $2",
        path.pid,
        path.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let sg_user = state
        .startgg
        .user_by_slug(&body.slug)
        .await?
        .ok_or_else(|| AppError::UnprocessableEntity("user not found on start.gg".into()))?;

    let account = sqlx::query_as!(
        StartggAccount,
        "INSERT INTO startgg_accounts (player_id, startgg_user_id, slug, display_name)
         VALUES ($1, $2, $3, $4)
         RETURNING id, player_id, startgg_user_id, slug, display_name, created_at",
        path.pid,
        sg_user.id,
        body.slug,
        sg_user.gamer_tag(),
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err)
            if db_err.constraint() == Some("startgg_accounts_player_id_startgg_user_id_key") =>
        {
            AppError::UnprocessableEntity("account already linked to this player".into())
        }
        other => AppError::Db(other),
    })?;

    Ok((StatusCode::CREATED, Json(AccountResponse::from(account))))
}

async fn unlink_account(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<ProjectPlayerAccountPath>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, path.id, user.id).await?;

    let result = sqlx::query!(
        "DELETE FROM startgg_accounts
         WHERE id = $1
           AND player_id = $2
           AND player_id IN (SELECT id FROM players WHERE project_id = $3)",
        path.aid,
        path.pid,
        path.id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_players).post(add_player))
        .route("/{pid}", delete(delete_player))
        .route("/{pid}/accounts", post(link_account))
        .route("/{pid}/accounts/{aid}", delete(unlink_account))
}
