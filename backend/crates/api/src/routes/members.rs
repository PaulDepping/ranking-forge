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
use common::models::{MemberRole, ProjectMember, UserRole};

#[derive(Deserialize)]
struct AddMemberRequest {
    email: String,
    role: MemberRole,
}

#[derive(Deserialize)]
struct ChangeMemberRoleRequest {
    role: MemberRole,
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
    require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

    let members = sqlx::query_as!(
        ProjectMember,
        r#"SELECT pm.project_id, pm.user_id, u.display_name, u.email,
                  pm.role as "role: MemberRole", pm.joined_at
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
    let (project, _) =
        require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

    let target = sqlx::query!(
        "SELECT id FROM users WHERE email = $1",
        body.email.to_lowercase(),
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::UnprocessableEntity("user not found".into()))?;

    if target.id == project.owner_id {
        return Err(AppError::UnprocessableEntity(
            "cannot add the project owner as a member; they already have full access".into(),
        ));
    }

    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role)
           VALUES ($1, $2, $3)
           ON CONFLICT (project_id, user_id) DO UPDATE SET role = EXCLUDED.role"#,
        project_id,
        target.id,
        body.role as MemberRole,
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
    let (project, _) =
        require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

    if target_user_id == project.owner_id {
        return Err(AppError::UnprocessableEntity(
            "cannot change the owner's role; use transfer-ownership".into(),
        ));
    }

    let result = sqlx::query!(
        r#"UPDATE project_members SET role = $1
           WHERE project_id = $2 AND user_id = $3"#,
        body.role as MemberRole,
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
    let (project, _) =
        require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

    if target_user_id == project.owner_id {
        return Err(AppError::UnprocessableEntity(
            "owner cannot be removed; transfer ownership first".into(),
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
    require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

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

    // Transfer ownership: update owner_id on the project
    sqlx::query!(
        "UPDATE projects SET owner_id = $1 WHERE id = $2",
        body.user_id,
        project_id,
    )
    .execute(&mut *tx)
    .await?;

    // Remove the new owner from project_members (they are now the owner via owner_id)
    sqlx::query!(
        "DELETE FROM project_members WHERE project_id = $1 AND user_id = $2",
        project_id,
        body.user_id,
    )
    .execute(&mut *tx)
    .await?;

    // Add the old owner as an editor member
    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role)
           VALUES ($1, $2, 'editor')
           ON CONFLICT (project_id, user_id) DO UPDATE SET role = 'editor'"#,
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

    async fn with_api_key(pool: &PgPool, email: &str) {
        sqlx::query!(
            "UPDATE users SET startgg_api_key = 'test-key' WHERE email = $1",
            email
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn create_project(app: &Router, cookie: &str, name: &str) -> String {
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_add_member_and_list(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "mem_owner").await;
        with_api_key(&pool, "mem_owner@test.com").await;
        let _ = register(&app, "mem_user").await;
        let proj_id = create_project(&app, &owner_cookie, "Collab Project").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/projects/{proj_id}/members"))
                    .header("content-type", "application/json")
                    .header("cookie", &owner_cookie)
                    .body(Body::from(
                        serde_json::to_vec(
                            &json!({"email": "mem_user@test.com", "role": "editor"}),
                        )
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(&format!("/projects/{proj_id}/members"))
                    .header("cookie", &owner_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let members: Value = serde_json::from_slice(&bytes).unwrap();
        // Owner is identified by ranking_projects.owner_id and is not in project_members,
        // so the list only contains the added editor member.
        assert_eq!(members.as_array().unwrap().len(), 1);
        assert_eq!(members[0]["email"], "mem_user@test.com");
        assert_eq!(members[0]["display_name"], "mem_user");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_remove_member(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "rem_owner").await;
        with_api_key(&pool, "rem_owner@test.com").await;
        let _ = register(&app, "rem_user").await;
        let proj_id = create_project(&app, &owner_cookie, "Remove Test").await;

        // Add member via SQL
        let user_id = sqlx::query_scalar!("SELECT id FROM users WHERE email = 'rem_user@test.com'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'editor')",
            proj_uuid,
            user_id
        )
        .execute(&pool)
        .await
        .unwrap();

        // Remove the member
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(&format!("/projects/{proj_id}/members/{user_id}"))
                    .header("cookie", &owner_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);

        // Verify removed from DB
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM project_members WHERE project_id = $1 AND user_id = $2",
            proj_uuid,
            user_id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, Some(0));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_transfer_ownership(pool: PgPool) {
        let app = make_app(pool.clone());
        let old_owner_cookie = register(&app, "old_owner").await;
        with_api_key(&pool, "old_owner@test.com").await;
        let _ = register(&app, "new_owner").await;
        let proj_id = create_project(&app, &old_owner_cookie, "Transfer Project").await;

        let new_owner_id =
            sqlx::query_scalar!("SELECT id FROM users WHERE email = 'new_owner@test.com'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'editor')",
            proj_uuid,
            new_owner_id
        )
        .execute(&pool)
        .await
        .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(&format!("/projects/{proj_id}/members/transfer-ownership"))
                    .header("content-type", "application/json")
                    .header("cookie", &old_owner_cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"user_id": new_owner_id})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);

        // After transfer: old owner becomes an editor member
        let old_role = sqlx::query_scalar!(
            r#"SELECT role::text FROM project_members
               WHERE project_id = $1 AND user_id = (SELECT id FROM users WHERE email = 'old_owner@test.com')"#,
            proj_uuid
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(old_role, Some("editor".to_string()));

        // New owner is now owner via owner_id, not in project_members
        let new_owner_id_check =
            sqlx::query_scalar!("SELECT owner_id FROM projects WHERE id = $1", proj_uuid)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(new_owner_id_check, new_owner_id);
    }
}
