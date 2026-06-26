use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    routes::projects::require_project_access,
    state::AppState,
};
use common::models::{MemberRole, ProjectInviteLink, UserRole};

#[derive(Deserialize)]
struct CreateInviteLinkRequest {
    role: MemberRole,
    expires_at: Option<DateTime<Utc>>,
}

async fn list_invite_links(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

    let links = sqlx::query_as!(
        ProjectInviteLink,
        r#"SELECT id, project_id, role as "role: MemberRole",
                  created_by, expires_at, revoked_at, created_at
           FROM project_invite_links
           WHERE project_id = $1 AND revoked_at IS NULL
           ORDER BY created_at DESC"#,
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(links))
}

async fn create_invite_link(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateInviteLinkRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

    let link = sqlx::query_as!(
        ProjectInviteLink,
        r#"INSERT INTO project_invite_links (project_id, role, created_by, expires_at)
           VALUES ($1, $2, $3, $4)
           RETURNING id, project_id, role as "role: MemberRole",
                     created_by, expires_at, revoked_at, created_at"#,
        project_id,
        body.role as MemberRole,
        user.id,
        body.expires_at,
    )
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(link)))
}

async fn revoke_invite_link(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, link_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

    let result = sqlx::query!(
        "UPDATE project_invite_links SET revoked_at = NOW()
         WHERE id = $1 AND project_id = $2 AND revoked_at IS NULL",
        link_id,
        project_id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// Top-level handler — mounted at /invite/:token/accept in routes/mod.rs
pub async fn accept_invite_link(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(token): Path<Uuid>,
) -> Result<impl IntoResponse> {
    #[derive(Serialize)]
    struct AcceptResponse {
        project_id: Uuid,
    }

    let link = sqlx::query_as!(
        ProjectInviteLink,
        r#"SELECT id, project_id, role as "role: MemberRole",
                  created_by, expires_at, revoked_at, created_at
           FROM project_invite_links
           WHERE id = $1"#,
        token,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    if link.revoked_at.is_some() {
        return Err(AppError::UnprocessableEntity(
            "invite link has been revoked".into(),
        ));
    }
    if let Some(exp) = link.expires_at {
        if exp < Utc::now() {
            return Err(AppError::UnprocessableEntity(
                "invite link has expired".into(),
            ));
        }
    }

    // If accepting user is already the owner (via owner_id), return success no-op
    let is_owner = sqlx::query_scalar!(
        "SELECT 1 AS one FROM projects WHERE id = $1 AND owner_id = $2",
        link.project_id,
        user.id,
    )
    .fetch_optional(&state.db)
    .await?;

    if is_owner.is_some() {
        return Ok(Json(AcceptResponse {
            project_id: link.project_id,
        }));
    }

    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role)
           VALUES ($1, $2, $3)
           ON CONFLICT (project_id, user_id) DO UPDATE SET role = EXCLUDED.role"#,
        link.project_id,
        user.id,
        link.role as MemberRole,
    )
    .execute(&state.db)
    .await?;

    Ok(Json(AcceptResponse {
        project_id: link.project_id,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_invite_links).post(create_invite_link))
        .route("/{link_id}", delete(revoke_invite_link))
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
                        serde_json::to_vec(&json!({"name": "Test"})).unwrap(),
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_invite_link_lifecycle(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "inv_owner").await;
        let user_cookie = register(&app, "inv_user").await;
        let proj_id = create_project(&app, &owner_cookie).await;

        // Create invite link
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/projects/{proj_id}/invite-links"))
                    .header("content-type", "application/json")
                    .header("cookie", &owner_cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"role": "editor"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let link: Value = serde_json::from_slice(&bytes).unwrap();
        let token = link["id"].as_str().unwrap().to_string();

        // Accept the invite
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/invite/{token}/accept"))
                    .header("cookie", &user_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        // User is now a member
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        let role = sqlx::query_scalar!(
            r#"SELECT role::text as "role: String" FROM project_members
               WHERE project_id = $1 AND user_id = (SELECT id FROM users WHERE email = 'inv_user@test.com')"#,
            proj_uuid
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(role, Some("editor".to_string()));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_revoked_link_cannot_be_accepted(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "rev_owner").await;
        let user_cookie = register(&app, "rev_user").await;
        let proj_id = create_project(&app, &owner_cookie).await;

        // Create and revoke link
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/projects/{proj_id}/invite-links"))
                    .header("content-type", "application/json")
                    .header("cookie", &owner_cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"role": "viewer"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let link: Value = serde_json::from_slice(&bytes).unwrap();
        let link_id = link["id"].as_str().unwrap().to_string();
        let token = link_id.clone();

        app.clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(&format!("/projects/{proj_id}/invite-links/{link_id}"))
                    .header("cookie", &owner_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Accept fails with 422
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/invite/{token}/accept"))
                    .header("cookie", &user_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 422);
    }
}
