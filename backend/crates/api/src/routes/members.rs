use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post},
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    routes::projects::require_project_access,
    state::AppState,
};
use common::models::{ProjectMember, ProjectMemberRole};

#[derive(Deserialize)]
struct AddMemberRequest {
    username: String,
    role: ProjectMemberRole,
}

#[derive(Deserialize)]
struct ChangeMemberRoleRequest {
    role: ProjectMemberRole,
}

#[derive(Deserialize)]
struct TransferOwnershipRequest {
    user_id: Uuid,
}

async fn list_members(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;

    let members = sqlx::query_as!(
        ProjectMember,
        r#"SELECT pm.project_id, pm.user_id, u.username,
                  pm.role as "role: ProjectMemberRole", pm.joined_at
           FROM project_members pm
           JOIN users u ON u.id = pm.user_id
           WHERE pm.project_id = $1
           ORDER BY pm.joined_at ASC"#,
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(members))
}

async fn add_member(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<AddMemberRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;

    if body.role == ProjectMemberRole::Owner {
        return Err(AppError::UnprocessableEntity(
            "cannot assign owner role via add-member; use transfer-ownership".into(),
        ));
    }

    let target = sqlx::query!(
        "SELECT id FROM users WHERE username = $1",
        body.username,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::UnprocessableEntity("user not found".into()))?;

    let is_owner = sqlx::query_scalar!(
        r#"SELECT 1 AS one FROM project_members
           WHERE project_id = $1 AND user_id = $2 AND role = 'owner'"#,
        project_id,
        target.id,
    )
    .fetch_optional(&state.db)
    .await?;

    if is_owner.is_some() {
        return Err(AppError::UnprocessableEntity(
            "cannot change the owner's role; use transfer-ownership".into(),
        ));
    }

    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role)
           VALUES ($1, $2, $3)
           ON CONFLICT (project_id, user_id) DO UPDATE SET role = EXCLUDED.role"#,
        project_id,
        target.id,
        body.role as ProjectMemberRole,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn change_member_role(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, target_user_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ChangeMemberRoleRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;

    if body.role == ProjectMemberRole::Owner {
        return Err(AppError::UnprocessableEntity(
            "cannot assign owner role; use transfer-ownership".into(),
        ));
    }

    let is_owner = sqlx::query_scalar!(
        r#"SELECT 1 AS one FROM project_members
           WHERE project_id = $1 AND user_id = $2 AND role = 'owner'"#,
        project_id,
        target_user_id,
    )
    .fetch_optional(&state.db)
    .await?;

    if is_owner.is_some() {
        return Err(AppError::UnprocessableEntity(
            "cannot change the owner's role; use transfer-ownership".into(),
        ));
    }

    let result = sqlx::query!(
        r#"UPDATE project_members SET role = $1
           WHERE project_id = $2 AND user_id = $3"#,
        body.role as ProjectMemberRole,
        project_id,
        target_user_id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn remove_member(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;

    if target_user_id == user.id {
        return Err(AppError::UnprocessableEntity(
            "owner cannot remove themselves; transfer ownership first".into(),
        ));
    }

    let result = sqlx::query!(
        "DELETE FROM project_members WHERE project_id = $1 AND user_id = $2",
        project_id,
        target_user_id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn transfer_ownership(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<TransferOwnershipRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;

    if body.user_id == user.id {
        return Err(AppError::UnprocessableEntity(
            "cannot transfer ownership to yourself".into(),
        ));
    }

    let target_exists = sqlx::query_scalar!(
        "SELECT 1 AS one FROM project_members WHERE project_id = $1 AND user_id = $2",
        project_id,
        body.user_id,
    )
    .fetch_optional(&state.db)
    .await?;

    if target_exists.is_none() {
        return Err(AppError::UnprocessableEntity(
            "target user is not a member of this project".into(),
        ));
    }

    let mut tx = state.db.begin().await?;

    sqlx::query!(
        "UPDATE project_members SET role = 'owner' WHERE project_id = $1 AND user_id = $2",
        project_id,
        body.user_id,
    )
    .execute(&mut *tx)
    .await?;

    sqlx::query!(
        "UPDATE project_members SET role = 'editor' WHERE project_id = $1 AND user_id = $2",
        project_id,
        user.id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_members).post(add_member))
        .route("/{uid}", patch(change_member_role).delete(remove_member))
        .route("/transfer-ownership", post(transfer_ownership))
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

    async fn create_project(app: &Router, cookie: &str, name: &str) -> String {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/projects")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_vec(&json!({"name": name})).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        v["id"].as_str().unwrap().to_string()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_add_member_and_list(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "mem_owner").await;
        let _ = register(&app, "mem_user").await;
        let proj_id = create_project(&app, &owner_cookie, "Collab Project").await;

        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri(&format!("/projects/{proj_id}/members"))
                .header("content-type", "application/json")
                .header("cookie", &owner_cookie)
                .body(Body::from(serde_json::to_vec(
                    &json!({"username": "mem_user", "role": "editor"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 204);

        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri(&format!("/projects/{proj_id}/members"))
                .header("cookie", &owner_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let members: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(members.as_array().unwrap().len(), 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_remove_member(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "rem_owner").await;
        let _ = register(&app, "rem_user").await;
        let proj_id = create_project(&app, &owner_cookie, "Remove Test").await;

        // Add member via SQL
        let user_id = sqlx::query_scalar!("SELECT id FROM users WHERE username = 'rem_user'")
            .fetch_one(&pool).await.unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'editor')",
            proj_uuid, user_id
        ).execute(&pool).await.unwrap();

        // Remove the member
        let resp = app.clone().oneshot(
            Request::builder()
                .method("DELETE")
                .uri(&format!("/projects/{proj_id}/members/{user_id}"))
                .header("cookie", &owner_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 204);

        // Verify removed from DB
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = $2",
            proj_uuid, user_id
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(count, Some(0));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_transfer_ownership(pool: PgPool) {
        let app = make_app(pool.clone());
        let old_owner_cookie = register(&app, "old_owner").await;
        let _ = register(&app, "new_owner").await;
        let proj_id = create_project(&app, &old_owner_cookie, "Transfer Project").await;

        let new_owner_id = sqlx::query_scalar!("SELECT id FROM users WHERE username = 'new_owner'")
            .fetch_one(&pool).await.unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'editor')",
            proj_uuid, new_owner_id
        ).execute(&pool).await.unwrap();

        let resp = app.clone().oneshot(
            Request::builder().method("POST")
                .uri(&format!("/projects/{proj_id}/members/transfer-ownership"))
                .header("content-type", "application/json")
                .header("cookie", &old_owner_cookie)
                .body(Body::from(serde_json::to_vec(
                    &json!({"user_id": new_owner_id})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 204);

        let old_role = sqlx::query_scalar!(
            r#"SELECT role::text FROM project_members
               WHERE project_id = $1 AND user_id = (SELECT id FROM users WHERE username = 'old_owner')"#,
            proj_uuid
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(old_role, Some("editor".to_string()));

        let new_role = sqlx::query_scalar!(
            r#"SELECT role::text FROM project_members
               WHERE project_id = $1 AND user_id = $2"#,
            proj_uuid, new_owner_id
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(new_role, Some("owner".to_string()));
    }
}
