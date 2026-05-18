use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post, put},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, OptionalAuthUser},
    state::AppState,
};
use common::models::{Project, ProjectMemberRole};

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
}

#[derive(Deserialize)]
pub struct PatchProjectRequest {
    pub name: Option<String>,
    pub published: Option<bool>,
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub published: bool,
    pub created_at: DateTime<Utc>,
    pub user_role: Option<ProjectMemberRole>,
}

impl ProjectResponse {
    fn from_project(p: Project, user_role: Option<ProjectMemberRole>) -> Self {
        ProjectResponse {
            id: p.id,
            name: p.name,
            game_id: p.game_id,
            game_name: p.game_name,
            published: p.published,
            created_at: p.created_at,
            user_role,
        }
    }
}

// ── Access helpers ────────────────────────────────────────────────────────────

/// Requires the user to be a project member with at least `min_role`.
/// Returns 404 if not a member (avoids leaking existence to non-members), 403 if role is too low.
pub async fn require_project_access(
    db: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
    min_role: ProjectMemberRole,
) -> Result<(Project, ProjectMemberRole)> {
    struct Row {
        id: Uuid,
        name: String,
        game_id: Option<i64>,
        game_name: Option<String>,
        published: bool,
        created_at: DateTime<Utc>,
        role: ProjectMemberRole,
    }

    let row = sqlx::query_as!(
        Row,
        r#"SELECT p.id, p.name, p.game_id, p.game_name, p.published, p.created_at,
                  pm.role as "role: ProjectMemberRole"
           FROM ranking_projects p
           JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $2
           WHERE p.id = $1"#,
        project_id,
        user_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    if !row.role.satisfies(&min_role) {
        return Err(AppError::Forbidden);
    }

    Ok((
        Project {
            id: row.id,
            name: row.name,
            game_id: row.game_id,
            game_name: row.game_name,
            published: row.published,
            created_at: row.created_at,
        },
        row.role,
    ))
}

/// Grants access if the user is a member (any role) OR the project is published.
/// Returns 404 for private projects with no membership (same response for non-existent projects).
pub async fn require_project_read_access(
    db: &PgPool,
    project_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<(Project, Option<ProjectMemberRole>)> {
    let project = sqlx::query_as!(
        Project,
        "SELECT id, name, game_id, game_name, published, created_at
         FROM ranking_projects WHERE id = $1",
        project_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    if project.published {
        let role = if let Some(uid) = user_id {
            sqlx::query_scalar!(
                r#"SELECT role as "role: ProjectMemberRole" FROM project_members
                   WHERE project_id = $1 AND user_id = $2"#,
                project_id,
                uid,
            )
            .fetch_optional(db)
            .await?
        } else {
            None
        };
        return Ok((project, role));
    }

    // Not published — require membership
    if let Some(uid) = user_id {
        let role = sqlx::query_scalar!(
            r#"SELECT role as "role: ProjectMemberRole" FROM project_members
               WHERE project_id = $1 AND user_id = $2"#,
            project_id,
            uid,
        )
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)?;
        return Ok((project, Some(role)));
    }

    Err(AppError::NotFound)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn list_projects(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<impl IntoResponse> {
    struct Row {
        id: Uuid,
        name: String,
        game_id: Option<i64>,
        game_name: Option<String>,
        published: bool,
        created_at: DateTime<Utc>,
        role: ProjectMemberRole,
    }

    let rows = sqlx::query_as!(
        Row,
        r#"SELECT p.id, p.name, p.game_id, p.game_name, p.published, p.created_at,
                  pm.role as "role: ProjectMemberRole"
           FROM ranking_projects p
           JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $1
           ORDER BY p.created_at DESC"#,
        user.id,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<ProjectResponse> = rows
        .into_iter()
        .map(|r| ProjectResponse {
            id: r.id,
            name: r.name,
            game_id: r.game_id,
            game_name: r.game_name,
            published: r.published,
            created_at: r.created_at,
            user_role: Some(r.role),
        })
        .collect();
    Ok(Json(resp))
}

async fn create_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<CreateProjectRequest>,
) -> Result<impl IntoResponse> {
    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("name must not be empty".into()));
    }
    if body.name.trim().chars().count() > 100 {
        return Err(AppError::UnprocessableEntity(
            "name must be at most 100 characters".into(),
        ));
    }

    let mut tx = state.db.begin().await?;

    let project = sqlx::query_as!(
        Project,
        "INSERT INTO ranking_projects (name, game_id, game_name)
         VALUES ($1, $2, $3)
         RETURNING id, name, game_id, game_name, published, created_at",
        body.name.trim(),
        body.game_id,
        body.game_name,
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'owner')",
        project.id,
        user.id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(ProjectResponse::from_project(project, Some(ProjectMemberRole::Owner))),
    ))
}

async fn get_project(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let (project, role) =
        require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;
    Ok(Json(ProjectResponse::from_project(project, role)))
}

async fn patch_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<PatchProjectRequest>,
) -> Result<impl IntoResponse> {
    let (project, role) =
        require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;

    let new_name = if let Some(ref n) = body.name {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            return Err(AppError::UnprocessableEntity("name must not be empty".into()));
        }
        if trimmed.chars().count() > 100 {
            return Err(AppError::UnprocessableEntity(
                "name must be at most 100 characters".into(),
            ));
        }
        trimmed.to_string()
    } else {
        project.name.clone()
    };

    let new_published = body.published.unwrap_or(project.published);

    let updated = sqlx::query_as!(
        Project,
        "UPDATE ranking_projects SET name = $1, published = $2
         WHERE id = $3
         RETURNING id, name, game_id, game_name, published, created_at",
        new_name,
        new_published,
        project_id,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ProjectResponse::from_project(updated, Some(role))))
}

async fn delete_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;
    sqlx::query!("DELETE FROM ranking_projects WHERE id = $1", project_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    use crate::routes::tournaments as t;
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route(
            "/{id}",
            get(get_project).delete(delete_project).patch(patch_project),
        )
        .nest("/{id}/players", crate::routes::players::router())
        .route(
            "/{id}/import",
            post(crate::routes::import::start_import)
                .get(crate::routes::import::get_import_status),
        )
        .route(
            "/{id}/tournament-entrants",
            get(crate::routes::players::list_tournament_entrants),
        )
        .route("/{id}/tournaments", get(t::list_tournaments))
        .route("/{id}/events/{eid}", patch(t::patch_event))
        .route("/{id}/stats", get(t::get_stats))
        .route("/{id}/stats/{player_id}", get(t::get_player_stats))
        .route("/{id}/head-to-head", get(t::get_head_to_head))
        .route(
            "/{id}/head-to-head/{pid_a}/{pid_b}/sets",
            get(t::get_h2h_sets),
        )
        .route("/{id}/ranking", put(crate::routes::players::reorder_players))
        .nest("/{id}/members", crate::routes::members::router())
        .nest("/{id}/invite-links", crate::routes::invite_links::router())
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, http::{Request, StatusCode}};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;

    use crate::{routes, state::AppState};
    use common::startgg::StartggClient;

    fn make_app(pool: PgPool) -> Router {
        let startgg = StartggClient::new_with_base_url("test".into(), "http://localhost:1".into());
        let state = AppState { db: pool, startgg, cors_origin: "http://localhost".into() };
        routes::router().with_state(state)
    }

    async fn register(app: &Router, username: &str) -> String {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &json!({"username": username, "password": "password123"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        resp.headers().get("set-cookie").unwrap().to_str().unwrap()
            .split(';').next().unwrap().to_string()
    }

    pub async fn create_project(app: &Router, cookie: &str, name: &str) -> String {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/projects")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_vec(
                    &json!({"name": name})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        v["id"].as_str().unwrap().to_string()
    }

    async fn json_body(resp: axum::response::Response) -> Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_list_projects_shows_all_member_roles(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "owner1").await;
        let editor_cookie = register(&app, "editor1").await;

        let proj_id = create_project(&app, &owner_cookie, "Test Project").await;

        let editor_id = sqlx::query_scalar!("SELECT id FROM users WHERE username = 'editor1'")
            .fetch_one(&pool).await.unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'editor')",
            proj_uuid, editor_id
        ).execute(&pool).await.unwrap();

        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri("/projects")
                .header("cookie", &owner_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body[0]["user_role"], "owner");

        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri("/projects")
                .header("cookie", &editor_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body[0]["user_role"], "editor");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_create_project_inserts_owner_membership(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner2").await;
        let proj_id = create_project(&app, &cookie, "My Project").await;

        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        let row = sqlx::query!(
            "SELECT role as \"role: String\" FROM project_members WHERE project_id = $1",
            proj_uuid
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(row.role, "owner");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_non_member_gets_404(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "owner3").await;
        let other_cookie = register(&app, "other3").await;
        let proj_id = create_project(&app, &owner_cookie, "Private Project").await;

        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri(&format!("/projects/{proj_id}"))
                .header("cookie", &other_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_unauthenticated_can_access_published_project(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner4").await;
        let proj_id = create_project(&app, &cookie, "Public Project").await;

        app.clone().oneshot(
            Request::builder().method("PATCH").uri(&format!("/projects/{proj_id}"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(&json!({"published": true})).unwrap())).unwrap()
        ).await.unwrap();

        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri(&format!("/projects/{proj_id}"))
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body["published"], true);
        assert!(body["user_role"].is_null());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_unauthenticated_cannot_access_private_project(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner5").await;
        let proj_id = create_project(&app, &cookie, "Private Project").await;

        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri(&format!("/projects/{proj_id}"))
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_unauthenticated_can_read_stats_of_published_project(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner_stats").await;
        let proj_id = create_project(&app, &cookie, "Stats Project").await;

        // Unpublished: unauthenticated stats returns 404
        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri(&format!("/projects/{proj_id}/stats"))
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 404);

        // Publish
        app.clone().oneshot(
            Request::builder().method("PATCH").uri(&format!("/projects/{proj_id}"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(&json!({"published": true})).unwrap())).unwrap()
        ).await.unwrap();

        // Published: unauthenticated stats returns 200 (empty, but 200)
        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri(&format!("/projects/{proj_id}/stats"))
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_only_owner_can_delete(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "owner6").await;
        let editor_cookie = register(&app, "editor6").await;
        let proj_id = create_project(&app, &owner_cookie, "Project").await;

        let editor_id = sqlx::query_scalar!("SELECT id FROM users WHERE username = 'editor6'")
            .fetch_one(&pool).await.unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'editor')",
            proj_uuid, editor_id
        ).execute(&pool).await.unwrap();

        let resp = app.clone().oneshot(
            Request::builder().method("DELETE").uri(&format!("/projects/{proj_id}"))
                .header("cookie", &editor_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 403);

        let resp = app.clone().oneshot(
            Request::builder().method("DELETE").uri(&format!("/projects/{proj_id}"))
                .header("cookie", &owner_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 204);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_viewer_cannot_add_player(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "owner_pl").await;
        let viewer_cookie = register(&app, "viewer_pl").await;
        let proj_id = create_project(&app, &owner_cookie, "Player Project").await;

        let viewer_id = sqlx::query_scalar!("SELECT id FROM users WHERE username = 'viewer_pl'")
            .fetch_one(&pool).await.unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'viewer')",
            proj_uuid, viewer_id
        ).execute(&pool).await.unwrap();

        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri(&format!("/projects/{proj_id}/players"))
                .header("content-type", "application/json")
                .header("cookie", &viewer_cookie)
                .body(Body::from(serde_json::to_vec(&json!({"name": "Alice"})).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 403);
    }
}
