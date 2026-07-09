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
    routes::auth::{AuthUser, OptionalAuthUser},
    routes::projects::{require_project_access, require_project_read_access},
    routes::tournaments::get_player_tournaments,
    state::AppState,
};
use common::models::{Player, StartggAccount, UserRole};

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
pub struct StartggAccountResponse {
    pub id: Uuid,
    pub player_id: Uuid,
    pub startgg_user_id: i64,
    pub handle: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
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

impl PlayerResponse {
    fn from_player(p: Player, accounts: Vec<AccountResponse>) -> Self {
        PlayerResponse {
            id: p.id,
            project_id: p.project_id,
            name: p.name,
            created_at: p.created_at,
            accounts,
        }
    }
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
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;

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
            PlayerResponse::from_player(p, accounts)
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
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity(
            "name must not be empty".into(),
        ));
    }

    let player = sqlx::query_as!(
        Player,
        "INSERT INTO players (project_id, name)
         VALUES ($1, $2)
         RETURNING id, project_id, name, created_at",
        project_id,
        body.name,
    )
    .fetch_one(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(PlayerResponse::from_player(player, vec![])),
    ))
}

async fn delete_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<ProjectPlayerPath>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, path.id, user.id, UserRole::Editor).await?;

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
    Path((project_id, player_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<LinkAccountRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    sqlx::query!(
        "SELECT id FROM players WHERE id = $1 AND project_id = $2",
        player_id,
        project_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let handle = body.handle.trim_start_matches("user/");

    let gp = sqlx::query!(
        "SELECT startgg_user_id, handle, display_name FROM global_players WHERE handle ILIKE $1 AND startgg_user_id IS NOT NULL LIMIT 1",
        handle,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound)?; // 404 = not yet indexed

    let user_id = gp.startgg_user_id.unwrap(); // safe: filtered above

    let row = sqlx::query!(
        "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (player_id, startgg_user_id) DO NOTHING
         RETURNING id, player_id, startgg_user_id, handle, display_name, created_at",
        player_id,
        user_id,
        gp.handle,
        gp.display_name,
    )
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => Ok((
            StatusCode::CREATED,
            Json(StartggAccountResponse {
                id: r.id,
                player_id: r.player_id,
                startgg_user_id: r.startgg_user_id,
                handle: r.handle,
                display_name: r.display_name,
                created_at: r.created_at,
            }),
        )),
        None => Err(AppError::UnprocessableEntity(
            "Account already linked".into(),
        )),
    }
}

async fn unlink_account(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<ProjectPlayerAccountPath>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, path.id, user.id, UserRole::Editor).await?;

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

// ── Shared helpers ────────────────────────────────────────────────────────────

async fn create_player_with_account(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    name: &str,
    user_id: i64,
    handle: &str,
    display_name: Option<&str>,
) -> crate::error::Result<Uuid> {
    let player = sqlx::query!(
        "INSERT INTO players (project_id, name)
         VALUES ($1, $2)
         RETURNING id",
        project_id,
        name,
    )
    .fetch_one(pool)
    .await?;

    sqlx::query!(
        "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
         VALUES ($1, $2, $3, $4)",
        player.id,
        user_id,
        handle,
        display_name,
    )
    .execute(pool)
    .await?;

    Ok(player.id)
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
    require_project_access(&state.db, id, user.id, UserRole::Editor).await?;

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
            results.push(BulkAddResult {
                name,
                handle,
                status: "skipped",
            });
            continue;
        }

        create_player_with_account(&state.db, id, &name, user_id, &handle, None).await?;
        results.push(BulkAddResult {
            name,
            handle,
            status: "created",
        });
    }

    Ok(Json(results))
}

// ── Add players by handles ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ByHandlesRequest {
    pub handles: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AddPlayerResult {
    pub handle: String,
    pub status: String,
    pub player_id: Option<Uuid>,
}

pub async fn add_players_by_handles(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ByHandlesRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    let mut results = Vec::new();

    for handle in &body.handles {
        let handle_str = handle.trim().trim_start_matches("user/");
        let gp = match sqlx::query!(
            "SELECT startgg_user_id, handle, display_name FROM global_players WHERE handle ILIKE $1 AND startgg_user_id IS NOT NULL LIMIT 1",
            handle_str,
        )
        .fetch_optional(&state.db)
        .await?
        {
            Some(r) => r,
            None => {
                results.push(AddPlayerResult {
                    handle: handle_str.to_string(),
                    status: "not_indexed".into(),
                    player_id: None,
                });
                continue;
            }
        };

        let startgg_user_id = gp.startgg_user_id.unwrap();

        // Check if a player with this account already exists in the project
        let existing = sqlx::query!(
            "SELECT p.id FROM players p
             JOIN startgg_accounts sa ON sa.player_id = p.id
             WHERE p.project_id = $1 AND sa.startgg_user_id = $2",
            project_id,
            startgg_user_id,
        )
        .fetch_optional(&state.db)
        .await?;

        if let Some(row) = existing {
            results.push(AddPlayerResult {
                handle: gp.handle.clone(),
                status: "duplicate".into(),
                player_id: Some(row.id),
            });
            continue;
        }

        // Create player + link account
        let player = sqlx::query!(
            "INSERT INTO players (project_id, name) VALUES ($1, $2) RETURNING id",
            project_id,
            gp.handle,
        )
        .fetch_one(&state.db)
        .await?;

        sqlx::query!(
            "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (player_id, startgg_user_id) DO NOTHING",
            player.id,
            startgg_user_id,
            gp.handle,
            gp.display_name,
        )
        .execute(&state.db)
        .await?;

        results.push(AddPlayerResult {
            handle: gp.handle,
            status: "created".into(),
            player_id: Some(player.id),
        });
    }

    Ok((StatusCode::CREATED, Json(results)))
}

// ── Tournament entrants ───────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct EntrantResp {
    pub startgg_user_id: Option<i64>,
    pub handle: String,
    pub display_name: Option<String>,
    pub placement: Option<i32>,
    pub seed: Option<i32>,
}

#[derive(Serialize)]
pub struct TournamentEventResp {
    pub name: String,
    pub entrants: Vec<EntrantResp>,
}

pub async fn list_tournament_entrants(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, tournament_handle)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, Some(user.id)).await?;

    let slug = format!(
        "tournament/{}",
        tournament_handle.trim_start_matches("tournament/")
    );

    struct EntrantRow {
        startgg_user_id: Option<i64>,
        handle: String,
        display_name: Option<String>,
        event_name: String,
        placement: Option<i32>,
        seed: Option<i32>,
    }

    let rows = sqlx::query_as!(
        EntrantRow,
        r#"
        SELECT
            gp.startgg_user_id,
            gp.handle,
            gp.display_name,
            ge.name AS event_name,
            gee.placement,
            gee.seed
        FROM global_tournaments gt
        JOIN global_events ge ON ge.tournament_id = gt.id
        JOIN global_event_entries gee ON gee.event_id = ge.id
        JOIN global_players gp ON gp.id = gee.player_id
        WHERE gt.slug = $1
          AND gp.startgg_user_id IS NOT NULL
        ORDER BY ge.name, gee.placement NULLS LAST
        "#,
        slug,
    )
    .fetch_all(&state.db)
    .await?;

    // Group by event_name, preserving insertion order
    let mut events: indexmap::IndexMap<String, Vec<EntrantResp>> = indexmap::IndexMap::new();
    for row in rows {
        let entrant = EntrantResp {
            startgg_user_id: row.startgg_user_id,
            handle: row.handle,
            display_name: row.display_name,
            placement: row.placement,
            seed: row.seed,
        };
        events.entry(row.event_name).or_default().push(entrant);
    }

    let result: Vec<TournamentEventResp> = events
        .into_iter()
        .map(|(name, entrants)| TournamentEventResp { name, entrants })
        .collect();

    Ok(Json(result))
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
    require_project_access(&state.db, path.id, user.id, UserRole::Editor).await?;

    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("name cannot be empty".into()));
    }

    let player = sqlx::query_as!(
        Player,
        "UPDATE players SET name = $1 WHERE id = $2 AND project_id = $3
         RETURNING id, project_id, name, created_at",
        body.name.trim(),
        path.pid,
        path.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(PlayerResponse::from_player(player, vec![])))
}

// ── Handle normalization ──────────────────────────────────────────────────────

fn strip_startgg_url_prefix(s: &str) -> &str {
    s.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.start.gg/")
        .trim_start_matches("start.gg/")
}

fn normalize_handle(input: &str) -> String {
    strip_startgg_url_prefix(input.trim())
        .trim_start_matches("user/")
        .to_string()
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
        .route("/{pid}/tournaments", get(get_player_tournaments))
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

    // ── Integration tests ─────────────────────────────────────────────────────

    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;

    use crate::{routes, state::AppState};

    fn make_app(pool: PgPool) -> Router {
        let state = AppState {
            db: pool,
            cors_origin: "http://localhost".into(),
        };
        routes::router().with_state(state)
    }

    async fn register(app: &Router, name: &str, password: &str) -> String {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&json!({
                            "email": format!("{name}@test.com"),
                            "display_name": name,
                            "password": password,
                        }))
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        format!("session_id={}", body["session_id"].as_str().unwrap())
    }

    async fn create_project(app: &Router, cookie: &str) -> Uuid {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .header("cookie", cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": "Test Project"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        v["id"].as_str().unwrap().parse().unwrap()
    }

    async fn create_player(app: &Router, cookie: &str, project_id: Uuid, name: &str) -> Uuid {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/projects/{project_id}/players"))
                    .header("content-type", "application/json")
                    .header("cookie", cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": name})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        v["id"].as_str().unwrap().parse().unwrap()
    }

    async fn read_json(resp: axum::response::Response) -> Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_link_account(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner", "password123").await;
        let project_id = create_project(&app, &cookie).await;
        let player_id = create_player(&app, &cookie, project_id, "Mango").await;

        // Seed global player
        sqlx::query!(
            "INSERT INTO global_players (startgg_user_id, handle, display_name) VALUES (99999, 'Mango', 'Juan')"
        )
        .execute(&pool)
        .await
        .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/projects/{project_id}/players/{player_id}/accounts"
                    ))
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"handle": "Mango"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let account = sqlx::query!(
            "SELECT startgg_user_id FROM startgg_accounts WHERE player_id = $1",
            player_id,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(account.startgg_user_id, 99999);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_link_account_not_found(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner", "password123").await;
        let project_id = create_project(&app, &cookie).await;
        let player_id = create_player(&app, &cookie, project_id, "Mango").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/projects/{project_id}/players/{player_id}/accounts"
                    ))
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"handle": "unknown_handle_xyz"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_add_players_by_handles(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner", "password123").await;
        let project_id = create_project(&app, &cookie).await;

        // Seed global players
        sqlx::query!(
            "INSERT INTO global_players (startgg_user_id, handle, display_name) VALUES (1001, 'Mango', 'Juan'), (1002, 'Armada', 'Adam')"
        )
        .execute(&pool)
        .await
        .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/projects/{project_id}/players/by-handles"))
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"handles": ["Mango", "Armada"]})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM players WHERE project_id = $1",
            project_id,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, Some(2));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_list_tournament_entrants(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner", "password123").await;
        let project_id = create_project(&app, &cookie).await;

        // Seed global tournament + event + players + entries
        sqlx::query!("INSERT INTO global_games (startgg_id, name) VALUES (1, 'SSBM')")
            .execute(&pool)
            .await
            .unwrap();

        let game_id: uuid::Uuid =
            sqlx::query_scalar!("SELECT id FROM global_games WHERE startgg_id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query!(
            "INSERT INTO global_tournaments (startgg_id, name, slug, online) VALUES (101, 'Genesis', 'tournament/genesis', false)"
        )
        .execute(&pool)
        .await
        .unwrap();

        let tournament_id: uuid::Uuid =
            sqlx::query_scalar!("SELECT id FROM global_tournaments WHERE startgg_id = 101")
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query!(
            "INSERT INTO global_events (startgg_id, tournament_id, game_id, name, slug, num_entrants) VALUES (201, $1, $2, 'Melee Singles', 'tournament/genesis/event/melee', 64)",
            tournament_id,
            game_id,
        )
        .execute(&pool)
        .await
        .unwrap();

        let event_id: uuid::Uuid =
            sqlx::query_scalar!("SELECT id FROM global_events WHERE startgg_id = 201")
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query!(
            "INSERT INTO global_players (startgg_user_id, handle) VALUES (1001, 'Mango'), (1002, 'Armada')"
        )
        .execute(&pool)
        .await
        .unwrap();

        let mango_id: uuid::Uuid =
            sqlx::query_scalar!("SELECT id FROM global_players WHERE handle = 'Mango'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let armada_id: uuid::Uuid =
            sqlx::query_scalar!("SELECT id FROM global_players WHERE handle = 'Armada'")
                .fetch_one(&pool)
                .await
                .unwrap();

        sqlx::query!(
            "INSERT INTO global_event_entries (event_id, player_id, placement) VALUES ($1, $2, 1), ($1, $3, 2)",
            event_id,
            mango_id,
            armada_id,
        )
        .execute(&pool)
        .await
        .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!(
                        "/projects/{project_id}/tournament-entrants/genesis"
                    ))
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = read_json(resp).await;
        let events = body.as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["name"], "Melee Singles");
        let entrants = events[0]["entrants"].as_array().unwrap();
        assert_eq!(entrants.len(), 2);
        assert_eq!(entrants[0]["handle"], "Mango");
    }
}
