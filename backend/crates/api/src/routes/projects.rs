use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    state::AppState,
};
use common::models::Project;

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
}

#[derive(Deserialize)]
pub struct RenameProjectRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        ProjectResponse {
            id: p.id,
            name: p.name,
            game_id: p.game_id,
            game_name: p.game_name,
            created_at: p.created_at,
        }
    }
}

// ── Ownership helper ──────────────────────────────────────────────────────────

pub async fn require_project(db: &PgPool, project_id: Uuid, user_id: Uuid) -> Result<Project> {
    sqlx::query_as!(
        Project,
        "SELECT id, user_id, name, game_id, game_name, created_at
         FROM ranking_projects
         WHERE id = $1 AND user_id = $2",
        project_id,
        user_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn list_projects(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<impl IntoResponse> {
    let projects = sqlx::query_as!(
        Project,
        "SELECT id, user_id, name, game_id, game_name, created_at
         FROM ranking_projects
         WHERE user_id = $1
         ORDER BY created_at DESC",
        user.id,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<ProjectResponse> = projects.into_iter().map(ProjectResponse::from).collect();
    Ok(Json(resp))
}

async fn create_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse> {
    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity(
            "name must not be empty".into(),
        ));
    }
    if body.name.trim().chars().count() > 100 {
        return Err(AppError::UnprocessableEntity(
            "name must be at most 100 characters".into(),
        ));
    }

    let project = sqlx::query_as!(
        Project,
        "INSERT INTO ranking_projects (user_id, name, game_id, game_name)
         VALUES ($1, $2, $3, $4)
         RETURNING id, user_id, name, game_id, game_name, created_at",
        user.id,
        body.name,
        body.game_id,
        body.game_name,
    )
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(ProjectResponse::from(project))))
}

async fn get_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let project = require_project(&state.db, project_id, user.id).await?;
    Ok(Json(ProjectResponse::from(project)))
}

async fn rename_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<RenameProjectRequest>,
) -> Result<impl IntoResponse> {
    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity(
            "name must not be empty".into(),
        ));
    }
    if body.name.trim().chars().count() > 100 {
        return Err(AppError::UnprocessableEntity(
            "name must be at most 100 characters".into(),
        ));
    }

    let project = sqlx::query_as!(
        Project,
        "UPDATE ranking_projects SET name = $1 WHERE id = $2 AND user_id = $3
         RETURNING id, user_id, name, game_id, game_name, created_at",
        body.name.trim(),
        project_id,
        user.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ProjectResponse::from(project)))
}

async fn delete_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

    sqlx::query!("DELETE FROM ranking_projects WHERE id = $1", project_id,)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    use crate::routes::tournaments as t;
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route("/{id}", get(get_project).delete(delete_project).patch(rename_project))
        .nest("/{id}/players", crate::routes::players::router())
        .route(
            "/{id}/import",
            post(crate::routes::import::start_import).get(crate::routes::import::get_import_status),
        )
        .route(
            "/{id}/tournament-entrants",
            get(crate::routes::players::list_tournament_entrants),
        )
        .route("/{id}/tournaments", get(t::list_tournaments))
        .route("/{id}/events/{eid}", patch(t::patch_event))
        .route("/{id}/stats", get(t::get_stats))
        .route("/{id}/head-to-head", get(t::get_head_to_head))
        .route(
            "/{id}/head-to-head/{pid_a}/{pid_b}/sets",
            get(t::get_h2h_sets),
        )
}
