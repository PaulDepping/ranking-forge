use axum::{
    Json, Router,
    extract::{Path, Query, State},
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
    handle: String,
}

#[derive(Serialize)]
pub struct AccountResponse {
    pub id: Uuid,
    pub startgg_user_id: i64,
    pub handle: String,
    pub display_name: Option<String>,
}

impl From<StartggAccount> for AccountResponse {
    fn from(a: StartggAccount) -> Self {
        AccountResponse {
            id: a.id,
            startgg_user_id: a.startgg_user_id,
            handle: a.handle,
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
        "SELECT id, project_id, name, rank_position, created_at
         FROM players
         WHERE project_id = $1
         ORDER BY rank_position ASC, created_at ASC",
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
            "SELECT id, player_id, startgg_user_id, handle, display_name, created_at
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
        "INSERT INTO players (project_id, name, rank_position)
         VALUES (
             $1, $2,
             (SELECT COALESCE(MAX(rank_position), 0) + 1 FROM players WHERE project_id = $1)
         )
         RETURNING id, project_id, name, rank_position, created_at",
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

    let handle = normalize_handle(&body.handle);

    let sg_user = state
        .startgg
        .user_by_slug(&handle)
        .await?
        .ok_or_else(|| AppError::UnprocessableEntity("user not found on start.gg".into()))?;

    let account = sqlx::query_as!(
        StartggAccount,
        "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
         VALUES ($1, $2, $3, $4)
         RETURNING id, player_id, startgg_user_id, handle, display_name, created_at",
        path.pid,
        sg_user.id,
        handle,
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

// ── Bulk add players ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BulkAddEntry {
    pub name: String,
    pub startgg_user_id: i64,
    pub handle: String,
}

#[derive(Debug, Deserialize)]
pub struct BulkAddRequest {
    pub players: Vec<BulkAddEntry>,
}

#[derive(Debug, Serialize)]
pub struct BulkAddResult {
    pub name: String,
    pub handle: String,
    pub status: &'static str, // "created" or "skipped"
}

pub async fn bulk_add_players(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<BulkAddRequest>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, id, user.id).await?;

    let mut results = Vec::new();

    for entry in body.players {
        let handle = normalize_handle(&entry.handle);
        let name = entry.name;
        let user_id = entry.startgg_user_id;

        // Check if this startgg_user_id is already linked in this project
        let existing = sqlx::query!(
            "SELECT 1 AS one FROM startgg_accounts sa
             JOIN players p ON sa.player_id = p.id
             WHERE p.project_id = $1 AND sa.startgg_user_id = $2",
            id,
            user_id,
        )
        .fetch_optional(&state.db)
        .await?;

        if existing.is_some() {
            results.push(BulkAddResult { name, handle, status: "skipped" });
            continue;
        }

        // Insert player
        let player = sqlx::query!(
            "INSERT INTO players (project_id, name, rank_position)
             VALUES (
                 $1, $2,
                 (SELECT COALESCE(MAX(rank_position), 0) + 1 FROM players WHERE project_id = $1)
             )
             RETURNING id",
            id,
            name,
        )
        .fetch_one(&state.db)
        .await?;

        // Insert startgg account
        sqlx::query!(
            "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle)
             VALUES ($1, $2, $3)",
            player.id,
            user_id,
            handle,
        )
        .execute(&state.db)
        .await?;

        results.push(BulkAddResult { name, handle, status: "created" });
    }

    Ok(Json(results))
}

// ── Add players by handles ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ByHandlesRequest {
    pub handles: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ByHandlesResult {
    pub handle: String,
    pub name: Option<String>,
    pub status: String, // "created", "skipped", "not_found"
}

pub async fn add_players_by_handles(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ByHandlesRequest>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, id, user.id).await?;

    let mut results = Vec::new();

    for raw_handle in body.handles {
        let handle = normalize_handle(&raw_handle);

        // Resolve on start.gg
        let sg_user = match state.startgg.user_by_slug(&handle).await {
            Ok(Some(u)) => u,
            Ok(None) => {
                results.push(ByHandlesResult {
                    handle,
                    name: None,
                    status: "not_found".to_string(),
                });
                continue;
            }
            Err(e) => return Err(AppError::from(e)),
        };

        let user_id = sg_user.id;
        let gamer_tag = sg_user
            .gamer_tag()
            .unwrap_or(&handle)
            .to_string();

        // Check if already linked in this project
        let existing = sqlx::query!(
            "SELECT 1 AS one FROM startgg_accounts sa
             JOIN players p ON sa.player_id = p.id
             WHERE p.project_id = $1 AND sa.startgg_user_id = $2",
            id,
            user_id,
        )
        .fetch_optional(&state.db)
        .await?;

        if existing.is_some() {
            results.push(ByHandlesResult {
                handle,
                name: Some(gamer_tag),
                status: "skipped".to_string(),
            });
            continue;
        }

        // Insert player
        let player = sqlx::query!(
            "INSERT INTO players (project_id, name, rank_position)
             VALUES (
                 $1, $2,
                 (SELECT COALESCE(MAX(rank_position), 0) + 1 FROM players WHERE project_id = $1)
             )
             RETURNING id",
            id,
            &gamer_tag,
        )
        .fetch_one(&state.db)
        .await?;

        // Insert startgg account
        sqlx::query!(
            "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
             VALUES ($1, $2, $3, $4)",
            player.id,
            user_id,
            handle,
            &gamer_tag,
        )
        .execute(&state.db)
        .await?;

        results.push(ByHandlesResult {
            handle,
            name: Some(gamer_tag),
            status: "created".to_string(),
        });
    }

    Ok(Json(results))
}

// ── Tournament entrants ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TournamentEntrantResponse {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct TournamentEntrantsQuery {
    pub tournament: String,
}

pub async fn list_tournament_entrants(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<TournamentEntrantsQuery>,
) -> Result<impl IntoResponse> {
    let project = require_project(&state.db, id, user.id).await?;

    let game_id = project.game_id.ok_or_else(|| {
        AppError::UnprocessableEntity("project has no game configured".into())
    })?;

    let handle = normalize_tournament_handle(&q.tournament);

    let entrants = state
        .startgg
        .tournament_entrants(&handle, game_id)
        .await
        .map_err(AppError::from)?;

    let response: Vec<TournamentEntrantResponse> = entrants
        .into_iter()
        .map(|e| TournamentEntrantResponse {
            startgg_user_id: e.startgg_user_id,
            handle: e.handle,
            name: e.name,
        })
        .collect();

    Ok(Json(response))
}

// ── Rename player ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RenamePlayerRequest {
    pub name: String,
}

async fn rename_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<ProjectPlayerPath>,
    Json(body): Json<RenamePlayerRequest>,
) -> Result<impl IntoResponse> {
    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("name cannot be empty".into()));
    }

    require_project(&state.db, path.id, user.id).await?;

    let player = sqlx::query_as!(
        Player,
        "UPDATE players SET name = $1 WHERE id = $2 AND project_id = $3
         RETURNING id, project_id, name, rank_position, created_at",
        body.name.trim(),
        path.pid,
        path.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(PlayerResponse {
        id: player.id,
        project_id: player.project_id,
        name: player.name,
        created_at: player.created_at,
        accounts: vec![],
    }))
}

// ── Handle normalization ──────────────────────────────────────────────────────

fn normalize_handle(input: &str) -> String {
    let s = input.trim();
    let s = s
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.start.gg/")
        .trim_start_matches("start.gg/")
        .trim_start_matches("user/");
    s.to_string()
}

fn normalize_tournament_handle(input: &str) -> String {
    let s = input.trim();
    let s = s
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.start.gg/")
        .trim_start_matches("start.gg/")
        .trim_start_matches("tournament/");
    s.split('/').next().unwrap_or(s).to_string()
}

// ── Reorder players ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ReorderRequest {
    pub player_ids: Vec<Uuid>,
}

pub async fn reorder_players(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ReorderRequest>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

    let existing_ids: Vec<Uuid> = sqlx::query_scalar!(
        "SELECT id FROM players WHERE project_id = $1",
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    let existing_set: std::collections::HashSet<Uuid> =
        existing_ids.into_iter().collect();

    if body.player_ids.len() != existing_set.len() {
        return Err(AppError::UnprocessableEntity(
            "player_ids must contain exactly all players in this project".into(),
        ));
    }

    let input_set: std::collections::HashSet<Uuid> = body.player_ids.iter().copied().collect();
    if input_set.len() != body.player_ids.len() {
        return Err(AppError::UnprocessableEntity(
            "player_ids contains duplicate ids".into(),
        ));
    }

    for &pid in &body.player_ids {
        if !existing_set.contains(&pid) {
            return Err(AppError::UnprocessableEntity(
                "player_ids contains an id not in this project".into(),
            ));
        }
    }

    let mut tx = state.db.begin().await?;
    for (i, &player_id) in body.player_ids.iter().enumerate() {
        sqlx::query!(
            "UPDATE players SET rank_position = $1 WHERE id = $2 AND project_id = $3",
            (i + 1) as i32,
            player_id,
            project_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(StatusCode::OK)
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_players).post(add_player))
        .route("/bulk", post(bulk_add_players))
        .route("/by-handles", post(add_players_by_handles))
        .route("/{pid}", delete(delete_player).patch(rename_player))
        .route("/{pid}/accounts", post(link_account))
        .route("/{pid}/accounts/{aid}", delete(unlink_account))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_handle_bare() {
        assert_eq!(normalize_handle("mang0"), "mang0");
    }

    #[test]
    fn normalize_handle_user_prefix() {
        assert_eq!(normalize_handle("user/mang0"), "mang0");
    }

    #[test]
    fn normalize_handle_full_url() {
        assert_eq!(normalize_handle("https://www.start.gg/user/mang0"), "mang0");
    }

    #[test]
    fn normalize_handle_full_url_http() {
        assert_eq!(normalize_handle("http://start.gg/user/mang0"), "mang0");
    }

    #[test]
    fn normalize_handle_trims_whitespace() {
        assert_eq!(normalize_handle("  mang0  "), "mang0");
    }

    #[test]
    fn normalize_tournament_handle_bare() {
        assert_eq!(normalize_tournament_handle("some-weekly"), "some-weekly");
    }

    #[test]
    fn normalize_tournament_handle_full_url() {
        assert_eq!(
            normalize_tournament_handle("https://www.start.gg/tournament/some-weekly/event/melee-singles"),
            "some-weekly"
        );
    }

    #[test]
    fn normalize_tournament_handle_with_tournament_prefix() {
        assert_eq!(normalize_tournament_handle("tournament/some-weekly"), "some-weekly");
    }

    #[test]
    fn normalize_tournament_handle_trims_whitespace() {
        assert_eq!(normalize_tournament_handle("  some-weekly  "), "some-weekly");
    }
}
