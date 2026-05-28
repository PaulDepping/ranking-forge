use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, put},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, OptionalAuthUser},
    routes::projects::require_project_access,
    state::AppState,
};
use common::models::{MemberRole, Project, Ranking, UserRole};

// ── Path param structs ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RankingPath {
    pub id: Uuid,
    pub rid: Uuid,
}

#[derive(Deserialize)]
pub struct RankingPlayerPath {
    pub id: Uuid,
    pub rid: Uuid,
    pub pid: Uuid,
}

// ── Access helpers ────────────────────────────────────────────────────────────

/// Checks project membership with at least `min_role`.
/// Returns (project, ranking, role). 404 if ranking doesn't belong to project.
pub async fn require_ranking_access(
    db: &PgPool,
    project_id: Uuid,
    ranking_id: Uuid,
    user_id: Uuid,
    min_role: UserRole,
) -> Result<(Project, Ranking, UserRole)> {
    let (project, role) = require_project_access(db, project_id, user_id, min_role).await?;
    let ranking = sqlx::query_as!(
        Ranking,
        "SELECT id, project_id, name, description, published, created_at
         FROM rankings WHERE id = $1 AND project_id = $2",
        ranking_id,
        project_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok((project, ranking, role))
}

/// Grants access if the user is a project member OR the specific ranking is published.
pub async fn require_ranking_read_access(
    db: &PgPool,
    project_id: Uuid,
    ranking_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<(Project, Ranking, Option<UserRole>)> {
    struct Row {
        id: Uuid,
        owner_id: Uuid,
        name: String,
        game_id: Option<i64>,
        game_name: Option<String>,
        created_at: DateTime<Utc>,
        ranking_id: Uuid,
        ranking_name: String,
        ranking_description: Option<String>,
        ranking_published: bool,
        ranking_created_at: DateTime<Utc>,
        member_role: Option<MemberRole>,
    }

    let row = sqlx::query_as!(
        Row,
        r#"SELECT p.id, p.owner_id, p.name, p.game_id, p.game_name, p.created_at,
                  r.id AS ranking_id, r.name AS ranking_name,
                  r.description AS ranking_description,
                  r.published AS ranking_published,
                  r.created_at AS ranking_created_at,
                  CASE WHEN pm.user_id IS NOT NULL THEN pm.role END AS "member_role: MemberRole"
           FROM projects p
           JOIN rankings r ON r.id = $2 AND r.project_id = p.id
           LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $3
           WHERE p.id = $1"#,
        project_id,
        ranking_id,
        user_id.unwrap_or(Uuid::nil()),
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    let project = Project {
        id: row.id,
        owner_id: row.owner_id,
        name: row.name,
        game_id: row.game_id,
        game_name: row.game_name,
        created_at: row.created_at,
    };
    let ranking = Ranking {
        id: row.ranking_id,
        project_id,
        name: row.ranking_name,
        description: row.ranking_description,
        published: row.ranking_published,
        created_at: row.ranking_created_at,
    };

    if ranking.published {
        let role = if let Some(uid) = user_id {
            if project.owner_id == uid {
                Some(UserRole::Owner)
            } else if row.member_role.is_some() {
                row.member_role.map(UserRole::from)
            } else {
                None
            }
        } else {
            None
        };
        return Ok((project, ranking, role));
    }

    if let Some(uid) = user_id {
        if project.owner_id == uid {
            return Ok((project, ranking, Some(UserRole::Owner)));
        }
        if let Some(role) = row.member_role {
            return Ok((project, ranking, Some(UserRole::from(role))));
        }
    }

    Err(AppError::NotFound)
}

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateRankingRequest {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct PatchRankingRequest {
    name: Option<String>,
    description: Option<String>,
    published: Option<bool>,
}

#[derive(Serialize)]
struct RankingResponse {
    id: Uuid,
    project_id: Uuid,
    name: String,
    description: Option<String>,
    published: bool,
    created_at: DateTime<Utc>,
    user_role: Option<UserRole>,
}

impl RankingResponse {
    fn from_ranking(r: Ranking, role: Option<UserRole>) -> Self {
        RankingResponse {
            id: r.id,
            project_id: r.project_id,
            name: r.name,
            description: r.description,
            published: r.published,
            created_at: r.created_at,
            user_role: role,
        }
    }
}

#[derive(Serialize)]
struct RankingPlayerResponse {
    player_id: Uuid,
    name: String,
    rank_position: i32,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct AddRankingPlayerRequest {
    player_id: Uuid,
}

#[derive(Deserialize)]
struct PatchRankingPlayerRequest {
    notes: Option<String>,
}

#[derive(Deserialize)]
struct ReorderRequest {
    player_ids: Vec<Uuid>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn list_rankings(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    use crate::routes::projects::require_project_read_access;
    let (_, role) = require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;

    let rankings = sqlx::query_as!(
        Ranking,
        "SELECT id, project_id, name, description, published, created_at
         FROM rankings WHERE project_id = $1 ORDER BY created_at ASC",
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<RankingResponse> = rankings
        .into_iter()
        .map(|r| RankingResponse::from_ranking(r, role.clone()))
        .collect();
    Ok(Json(resp))
}

async fn create_ranking(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateRankingRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity(
            "name must not be empty".into(),
        ));
    }

    let ranking = sqlx::query_as!(
        Ranking,
        "INSERT INTO rankings (project_id, name, description)
         VALUES ($1, $2, $3)
         RETURNING id, project_id, name, description, published, created_at",
        project_id,
        body.name.trim(),
        body.description.as_deref(),
    )
    .fetch_one(&state.db)
    .await?;

    // Backfill ranking_events for all events already imported for this project.
    sqlx::query!(
        r#"
        INSERT INTO ranking_events (ranking_id, event_id, included)
        SELECT DISTINCT $1::uuid, e.id, true
        FROM events e
        JOIN entrants ent ON ent.event_id = e.id
        JOIN players pl ON pl.id = ent.player_id AND pl.project_id = $2
        ON CONFLICT DO NOTHING
        "#,
        ranking.id,
        project_id,
    )
    .execute(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(RankingResponse::from_ranking(
            ranking,
            Some(UserRole::Owner),
        )),
    ))
}

async fn get_ranking(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    let (_, ranking, role) =
        require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;
    Ok(Json(RankingResponse::from_ranking(ranking, role)))
}

async fn patch_ranking(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
    Json(body): Json<PatchRankingRequest>,
) -> Result<impl IntoResponse> {
    let (_, ranking, role) =
        require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;

    if body.published == Some(true) && !matches!(role, UserRole::Owner) {
        return Err(AppError::Forbidden);
    }

    let new_name = if let Some(ref n) = body.name {
        let t = n.trim();
        if t.is_empty() {
            return Err(AppError::UnprocessableEntity(
                "name must not be empty".into(),
            ));
        }
        t.to_string()
    } else {
        ranking.name.clone()
    };

    let updated = sqlx::query_as!(
        Ranking,
        "UPDATE rankings SET name = $1, description = $2, published = $3
         WHERE id = $4
         RETURNING id, project_id, name, description, published, created_at",
        new_name,
        body.description
            .as_deref()
            .or(ranking.description.as_deref()),
        body.published.unwrap_or(ranking.published),
        path.rid,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(RankingResponse::from_ranking(updated, Some(role))))
}

async fn delete_ranking(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Owner).await?;
    sqlx::query!("DELETE FROM rankings WHERE id = $1", path.rid)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Ranking player membership ─────────────────────────────────────────────────

async fn list_ranking_players(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    struct Row {
        player_id: Uuid,
        name: String,
        rank_position: i32,
        notes: Option<String>,
    }

    let rows = sqlx::query_as!(
        Row,
        "SELECT rp.player_id, pl.name, rp.rank_position, rp.notes
         FROM ranking_players rp
         JOIN players pl ON pl.id = rp.player_id
         WHERE rp.ranking_id = $1
         ORDER BY rp.rank_position ASC, pl.created_at ASC",
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<RankingPlayerResponse> = rows
        .into_iter()
        .map(|r| RankingPlayerResponse {
            player_id: r.player_id,
            name: r.name,
            rank_position: r.rank_position,
            notes: r.notes,
        })
        .collect();
    Ok(Json(resp))
}

async fn add_ranking_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
    Json(body): Json<AddRankingPlayerRequest>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;

    sqlx::query!(
        "SELECT id FROM players WHERE id = $1 AND project_id = $2",
        body.player_id,
        path.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let max_pos: i32 = sqlx::query_scalar!(
        "SELECT COALESCE(MAX(rank_position), 0) FROM ranking_players WHERE ranking_id = $1",
        path.rid,
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);

    sqlx::query!(
        "INSERT INTO ranking_players (ranking_id, player_id, rank_position)
         VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
        path.rid,
        body.player_id,
        max_pos + 1,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::CREATED)
}

async fn remove_ranking_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPlayerPath>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;
    let result = sqlx::query!(
        "DELETE FROM ranking_players WHERE ranking_id = $1 AND player_id = $2",
        path.rid,
        path.pid,
    )
    .execute(&state.db)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn patch_ranking_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPlayerPath>,
    Json(body): Json<PatchRankingPlayerRequest>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;
    let result = sqlx::query!(
        "UPDATE ranking_players SET notes = $1 WHERE ranking_id = $2 AND player_id = $3",
        body.notes.as_deref(),
        path.rid,
        path.pid,
    )
    .execute(&state.db)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::OK)
}

async fn reorder_ranking_players(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
    Json(body): Json<ReorderRequest>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;

    let existing_ids: Vec<Uuid> = sqlx::query_scalar!(
        "SELECT player_id FROM ranking_players WHERE ranking_id = $1",
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    let existing_set: std::collections::HashSet<Uuid> = existing_ids.into_iter().collect();
    let input_set: std::collections::HashSet<Uuid> = body.player_ids.iter().copied().collect();

    if body.player_ids.len() != existing_set.len() || input_set.len() != body.player_ids.len() {
        return Err(AppError::UnprocessableEntity(
            "player_ids must contain exactly all players in this ranking, no duplicates".into(),
        ));
    }
    for &pid in &body.player_ids {
        if !existing_set.contains(&pid) {
            return Err(AppError::UnprocessableEntity(
                "player_ids contains an id not in this ranking".into(),
            ));
        }
    }

    let mut tx = state.db.begin().await?;
    for (i, &player_id) in body.player_ids.iter().enumerate() {
        sqlx::query!(
            "UPDATE ranking_players SET rank_position = $1
             WHERE ranking_id = $2 AND player_id = $3",
            (i + 1) as i32,
            path.rid,
            player_id,
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
        .route("/", get(list_rankings).post(create_ranking))
        .route(
            "/{rid}",
            get(get_ranking).patch(patch_ranking).delete(delete_ranking),
        )
        .route(
            "/{rid}/players",
            get(list_ranking_players).post(add_ranking_player),
        )
        .route(
            "/{rid}/players/{pid}",
            delete(remove_ranking_player).patch(patch_ranking_player),
        )
        .route("/{rid}/ranking", put(reorder_ranking_players))
        .nest("/{rid}", crate::routes::tournaments::router())
}

#[cfg(test)]
mod tests {
    use crate::{routes, state::AppState};
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;

    fn make_app(pool: PgPool) -> Router {
        let state = AppState {
            db: pool,
            cors_origin: "http://localhost".into(),
            startgg_base_url: "http://localhost:1".into(),
        };
        routes::router().with_state(state)
    }

    async fn register(app: &Router, name: &str) -> String {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &json!({"email": format!("{name}@test.com"), "display_name": name, "password": "password123"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        format!("session_id={}", body["session_id"].as_str().unwrap())
    }

    async fn with_api_key(pool: &PgPool, email: &str) {
        sqlx::query!(
            "UPDATE users SET startgg_api_key = 'test-key' WHERE email = $1",
            email
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn create_project(app: &Router, cookie: &str) -> String {
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
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        v["id"].as_str().unwrap().to_string()
    }

    async fn json_body(resp: axum::response::Response) -> Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_create_and_list_rankings(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner_rank").await;
        with_api_key(&pool, "owner_rank@test.com").await;
        let proj_id = create_project(&app, &cookie).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/projects/{proj_id}/rankings"))
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": "2025 Season"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = json_body(resp).await;
        assert_eq!(body["name"], "2025 Season");
        let ranking_id = body["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/projects/{proj_id}/rankings"))
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body.as_array().unwrap().len(), 1);
        assert_eq!(body[0]["id"], ranking_id);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_published_ranking_accessible_without_auth(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "pub_owner").await;
        with_api_key(&pool, "pub_owner@test.com").await;
        let proj_id = create_project(&app, &cookie).await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/projects/{proj_id}/rankings"))
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": "Public"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let ranking_id = json_body(resp).await["id"].as_str().unwrap().to_string();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/projects/{proj_id}/rankings/{ranking_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);

        app.clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri(&format!("/projects/{proj_id}/rankings/{ranking_id}"))
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"published": true})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/projects/{proj_id}/rankings/{ranking_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body["published"], true);
        assert!(body["user_role"].is_null());
    }
}
