use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
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
use common::models::{MemberRole, Project, UserRole};

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
}

#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub user_role: Option<UserRole>,
}

impl ProjectResponse {
    fn from_project(p: Project, user_role: Option<UserRole>) -> Self {
        ProjectResponse {
            id: p.id,
            name: p.name,
            game_id: p.game_id,
            game_name: p.game_name,
            created_at: p.created_at,
            user_role,
        }
    }
}

// ── Access helpers ────────────────────────────────────────────────────────────

/// Requires the user to be the owner or a project member with at least `min_role`.
/// Returns 404 if not a member/owner (avoids leaking existence to non-members), 403 if role is too low.
pub async fn require_project_access(
    db: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
    min_role: UserRole,
) -> Result<(Project, UserRole)> {
    struct Row {
        id: Uuid,
        owner_id: Uuid,
        name: String,
        game_id: Option<i64>,
        game_name: Option<String>,
        created_at: DateTime<Utc>,
        is_owner: Option<bool>,
        member_role: Option<MemberRole>,
    }

    let row = sqlx::query_as!(
        Row,
        r#"SELECT p.id, p.owner_id, p.name, p.game_id, p.game_name, p.created_at,
                  (p.owner_id = $2) AS is_owner,
                  CASE WHEN pm.user_id IS NOT NULL THEN pm.role END AS "member_role: MemberRole"
           FROM projects p
           LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $2
           WHERE p.id = $1
             AND (p.owner_id = $2 OR pm.user_id = $2)"#,
        project_id,
        user_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    let role = if row.is_owner == Some(true) {
        UserRole::Owner
    } else {
        row.member_role
            .map(UserRole::from)
            .ok_or(AppError::NotFound)?
    };

    if !role.satisfies(&min_role) {
        return Err(AppError::Forbidden);
    }

    Ok((
        Project {
            id: row.id,
            owner_id: row.owner_id,
            name: row.name,
            game_id: row.game_id,
            game_name: row.game_name,
            created_at: row.created_at,
        },
        role,
    ))
}

/// Grants access if the user is the owner, a member (any role), OR any ranking in the project is published.
/// Returns 404 for private projects with no membership (same response for non-existent projects).
pub async fn require_project_read_access(
    db: &PgPool,
    project_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<(Project, Option<UserRole>)> {
    let project = sqlx::query_as!(
        Project,
        "SELECT id, owner_id, name, game_id, game_name, created_at
         FROM projects WHERE id = $1",
        project_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    let has_published_ranking: bool = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM rankings WHERE project_id = $1 AND published = true)",
        project_id,
    )
    .fetch_one(db)
    .await?
    .unwrap_or(false);

    if has_published_ranking {
        let role = if let Some(uid) = user_id {
            if project.owner_id == uid {
                Some(UserRole::Owner)
            } else {
                sqlx::query_scalar!(
                    r#"SELECT role AS "role: MemberRole" FROM project_members
                       WHERE project_id = $1 AND user_id = $2"#,
                    project_id,
                    uid,
                )
                .fetch_optional(db)
                .await?
                .map(UserRole::from)
            }
        } else {
            None
        };
        return Ok((project, role));
    }

    // No published ranking — require ownership or membership
    if let Some(uid) = user_id {
        if project.owner_id == uid {
            return Ok((project, Some(UserRole::Owner)));
        }
        let role = sqlx::query_scalar!(
            r#"SELECT role AS "role: MemberRole" FROM project_members
               WHERE project_id = $1 AND user_id = $2"#,
            project_id,
            uid,
        )
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)?;
        return Ok((project, Some(UserRole::from(role))));
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
        owner_id: Uuid,
        name: String,
        game_id: Option<i64>,
        game_name: Option<String>,
        created_at: DateTime<Utc>,
        is_owner: Option<bool>,
        member_role: Option<MemberRole>,
    }

    let rows = sqlx::query_as!(
        Row,
        r#"SELECT p.id, p.owner_id, p.name, p.game_id, p.game_name, p.created_at,
                  (p.owner_id = $1) AS is_owner,
                  CASE WHEN pm.user_id IS NOT NULL THEN pm.role END AS "member_role: MemberRole"
           FROM projects p
           LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $1
           WHERE p.owner_id = $1 OR pm.user_id = $1
           ORDER BY p.created_at DESC"#,
        user.id,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<ProjectResponse> = rows
        .into_iter()
        .map(|r| {
            let role = if r.is_owner == Some(true) {
                UserRole::Owner
            } else {
                r.member_role
                    .map(UserRole::from)
                    .unwrap_or(UserRole::Viewer)
            };
            ProjectResponse::from_project(
                Project {
                    id: r.id,
                    owner_id: r.owner_id,
                    name: r.name,
                    game_id: r.game_id,
                    game_name: r.game_name,
                    created_at: r.created_at,
                },
                Some(role),
            )
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
        "INSERT INTO projects (owner_id, name, game_id, game_name)
         VALUES ($1, $2, $3, $4)
         RETURNING id, owner_id, name, game_id, game_name, created_at",
        user.id,
        body.name.trim(),
        body.game_id,
        body.game_name,
    )
    .fetch_one(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ProjectResponse::from_project(
            project,
            Some(UserRole::Owner),
        )),
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
        require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

    let new_name = if let Some(ref n) = body.name {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            return Err(AppError::UnprocessableEntity(
                "name must not be empty".into(),
            ));
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

    let updated = sqlx::query_as!(
        Project,
        "UPDATE projects SET name = $1
         WHERE id = $2
         RETURNING id, owner_id, name, game_id, game_name, created_at",
        new_name,
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
    require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;
    sqlx::query!("DELETE FROM projects WHERE id = $1", project_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route(
            "/{id}",
            get(get_project).delete(delete_project).patch(patch_project),
        )
        .nest("/{id}/players", crate::routes::players::router())
        .route(
            "/{id}/import",
            get(crate::routes::import::get_import_status),
        )
        .merge(crate::routes::import::rate_limited_post_router())
        .merge(crate::routes::import::router())
        .route(
            "/{id}/tournament-entrants/{handle}",
            get(crate::routes::players::list_tournament_entrants),
        )
        .route(
            "/{id}/tournaments/{tid}",
            delete(crate::routes::tournaments::delete_tournament),
        )
        .nest("/{id}/rankings", crate::routes::rankings::router())
        .nest("/{id}/members", crate::routes::members::router())
        .nest("/{id}/invite-links", crate::routes::invite_links::router())
}

#[cfg(test)]
mod tests {
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

    async fn register(app: &Router, name: &str) -> String {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &json!({"email": format!("{name}@test.com"), "display_name": name, "password": "password123"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        format!("session_id={}", body["session_id"].as_str().unwrap())
    }

    pub async fn create_project(app: &Router, cookie: &str, name: &str) -> String {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
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

        let editor_id =
            sqlx::query_scalar!("SELECT id FROM users WHERE email = 'editor1@test.com'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'editor')",
            proj_uuid,
            editor_id
        )
        .execute(&pool)
        .await
        .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/projects")
                    .header("cookie", &owner_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body[0]["user_role"], "owner");

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/projects")
                    .header("cookie", &editor_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body[0]["user_role"], "editor");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_create_project_sets_owner_id(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner2").await;
        let proj_id = create_project(&app, &cookie, "My Project").await;

        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        let owner_id =
            sqlx::query_scalar!("SELECT owner_id FROM projects WHERE id = $1", proj_uuid)
                .fetch_one(&pool)
                .await
                .unwrap();

        let user_id = sqlx::query_scalar!("SELECT id FROM users WHERE email = 'owner2@test.com'")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(owner_id, user_id);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_non_member_gets_404(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "owner3").await;
        let other_cookie = register(&app, "other3").await;
        let proj_id = create_project(&app, &owner_cookie, "Private Project").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/projects/{proj_id}"))
                    .header("cookie", &other_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_unauthenticated_cannot_access_private_project(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner5").await;
        let proj_id = create_project(&app, &cookie, "Private Project").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/projects/{proj_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 404);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_only_owner_can_delete(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "owner6").await;
        let editor_cookie = register(&app, "editor6").await;
        let proj_id = create_project(&app, &owner_cookie, "Project").await;

        let editor_id =
            sqlx::query_scalar!("SELECT id FROM users WHERE email = 'editor6@test.com'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'editor')",
            proj_uuid,
            editor_id
        )
        .execute(&pool)
        .await
        .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(&format!("/projects/{proj_id}"))
                    .header("cookie", &editor_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 403);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(&format!("/projects/{proj_id}"))
                    .header("cookie", &owner_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_get_project_response_shape(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "keyowner").await;
        let proj_id = create_project(&app, &cookie, "Key Project").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/projects/{proj_id}"))
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert!(body.get("owner_has_startgg_key").is_none());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_viewer_cannot_add_player(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "owner_pl").await;
        let viewer_cookie = register(&app, "viewer_pl").await;
        let proj_id = create_project(&app, &owner_cookie, "Player Project").await;

        let viewer_id =
            sqlx::query_scalar!("SELECT id FROM users WHERE email = 'viewer_pl@test.com'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'viewer')",
            proj_uuid,
            viewer_id
        )
        .execute(&pool)
        .await
        .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/projects/{proj_id}/players"))
                    .header("content-type", "application/json")
                    .header("cookie", &viewer_cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": "Alice"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 403);
    }
}
