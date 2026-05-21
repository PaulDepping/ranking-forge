use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, patch, put},
};
use serde::Deserialize;

use common::startgg::{StartggClient, StartggError};

use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, hash_password, verify_password},
    state::AppState,
};

#[derive(Deserialize)]
struct UpdateProfileRequest {
    display_name: Option<String>,
    email: Option<String>,
}

#[derive(Deserialize)]
struct UpdatePasswordRequest {
    current_password: String,
    new_password: String,
}

fn is_valid_email(s: &str) -> bool {
    let parts: Vec<&str> = s.splitn(2, '@').collect();
    parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.')
}

async fn update_profile(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse> {
    if body.display_name.is_none() && body.email.is_none() {
        return Err(AppError::UnprocessableEntity(
            "at least one of display_name or email must be provided".into(),
        ));
    }

    if let Some(ref name) = body.display_name {
        if name.chars().count() < 1 {
            return Err(AppError::UnprocessableEntity(
                "display name must not be empty".into(),
            ));
        }
        if name.chars().count() > 50 {
            return Err(AppError::UnprocessableEntity(
                "display name must be at most 50 characters".into(),
            ));
        }
    }

    if let Some(ref email) = body.email {
        if !is_valid_email(email) {
            return Err(AppError::UnprocessableEntity(
                "invalid email address".into(),
            ));
        }
        if email.chars().count() > 255 {
            return Err(AppError::UnprocessableEntity(
                "email must be at most 255 characters".into(),
            ));
        }
    }

    let new_display_name = body.display_name.as_deref().unwrap_or(&user.display_name);
    let new_email = body
        .email
        .as_deref()
        .map(|e| e.to_lowercase())
        .unwrap_or_else(|| user.email.clone());

    sqlx::query!(
        "UPDATE users SET display_name = $1, email = $2 WHERE id = $3",
        new_display_name,
        new_email,
        user.id,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint() == Some("users_email_key") => {
            AppError::UnprocessableEntity("email already registered".into())
        }
        other => AppError::Db(other),
    })?;

    Ok(StatusCode::NO_CONTENT)
}

async fn update_password(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<UpdatePasswordRequest>,
) -> Result<impl IntoResponse> {
    if body.new_password.chars().count() < 8 {
        return Err(AppError::UnprocessableEntity(
            "password must be at least 8 characters".into(),
        ));
    }
    if body.new_password.chars().count() > 128 {
        return Err(AppError::UnprocessableEntity(
            "password must be at most 128 characters".into(),
        ));
    }

    verify_password(body.current_password, user.password_hash).await?;

    let new_hash = hash_password(body.new_password).await?;
    sqlx::query!(
        "UPDATE users SET password_hash = $1 WHERE id = $2",
        new_hash,
        user.id,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_account(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<impl IntoResponse> {
    sqlx::query!("DELETE FROM users WHERE id = $1", user.id)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SetStartggKeyRequest {
    api_key: String,
}

async fn set_startgg_key(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(body): Json<SetStartggKeyRequest>,
) -> Result<impl IntoResponse> {
    let client =
        StartggClient::new_with_base_url(body.api_key.clone(), state.startgg_base_url.clone());
    client.validate_key().await.map_err(|e| match e {
        StartggError::Http(re) => AppError::ExternalApi(re),
        StartggError::ComplexityTooHigh { .. } => AppError::ExternalApiError,
        _ => AppError::UnprocessableEntity("Invalid start.gg API key".into()),
    })?;

    sqlx::query!(
        "UPDATE users SET startgg_api_key = $1 WHERE id = $2",
        body.api_key,
        user.id,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_startgg_key(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<impl IntoResponse> {
    sqlx::query!(
        "UPDATE users SET startgg_api_key = NULL WHERE id = $1",
        user.id,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/profile", patch(update_profile))
        .route("/password", patch(update_password))
        .route(
            "/startgg-key",
            put(set_startgg_key).delete(delete_startgg_key),
        )
        .route("/", delete(delete_account))
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
    use wiremock::{Mock, MockServer, ResponseTemplate, matchers::method};

    fn make_app(pool: PgPool) -> Router {
        let state = AppState {
            db: pool,
            cors_origin: "http://localhost".into(),
            startgg_base_url: "http://localhost:1".into(),
        };
        routes::router().with_state(state)
    }

    fn make_app_with_startgg(pool: PgPool, startgg_url: &str) -> Router {
        let state = AppState {
            db: pool,
            cors_origin: "http://localhost".into(),
            startgg_base_url: startgg_url.into(),
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

    async fn json_body(resp: axum::response::Response) -> Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_update_profile_display_name(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "profuser").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/account/profile")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"display_name": "New Name"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);

        let name =
            sqlx::query_scalar!("SELECT display_name FROM users WHERE email = 'profuser@test.com'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(name, "New Name");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_update_profile_duplicate_email(pool: PgPool) {
        let app = make_app(pool.clone());
        let _c1 = register(&app, "dupuser1").await;
        let c2 = register(&app, "dupuser2").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/account/profile")
                    .header("content-type", "application/json")
                    .header("cookie", &c2)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"email": "dupuser1@test.com"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 422);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_update_password_wrong_current(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "pwuser").await;

        let resp = app.clone().oneshot(
            Request::builder().method("PATCH").uri("/account/password")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(
                    &json!({"current_password": "wrongpassword", "new_password": "newpassword123"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 401);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_update_password_success(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "pwuser2").await;

        let resp = app.clone().oneshot(
            Request::builder().method("PATCH").uri("/account/password")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(
                    &json!({"current_password": "password123", "new_password": "newpassword456"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 204);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(
                            &json!({"email": "pwuser2@test.com", "password": "newpassword456"}),
                        )
                        .unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_delete_account_cascades_projects(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "deluser").await;

        // Set key so project creation succeeds
        sqlx::query!("UPDATE users SET startgg_api_key = 'k' WHERE email = 'deluser@test.com'")
            .execute(&pool)
            .await
            .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"name": "My Project"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 201);
        let body: Value = json_body(resp).await;
        let project_id: uuid::Uuid = body["id"].as_str().unwrap().parse().unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/account")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);

        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM ranking_projects WHERE id = $1",
            project_id
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, Some(0));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_delete_account_clears_session(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "sessuser").await;

        app.clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/account")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 401);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_set_startgg_key_valid_stores_key(pool: PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"data": {"currentUser": {"id": 1}}})),
            )
            .mount(&mock)
            .await;

        let app = make_app_with_startgg(pool.clone(), &mock.uri());
        let cookie = register(&app, "keyuser").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/account/startgg-key")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"api_key": "my-valid-key"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);

        let key = sqlx::query_scalar!(
            "SELECT startgg_api_key FROM users WHERE email = 'keyuser@test.com'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(key, Some("my-valid-key".to_string()));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_set_startgg_key_invalid_returns_422(pool: PgPool) {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({"data": null, "errors": [{"message": "not authorized"}]}),
                ),
            )
            .mount(&mock)
            .await;

        let app = make_app_with_startgg(pool.clone(), &mock.uri());
        let cookie = register(&app, "badkeyuser").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/account/startgg-key")
                    .header("content-type", "application/json")
                    .header("cookie", &cookie)
                    .body(Body::from(
                        serde_json::to_vec(&json!({"api_key": "bad-key"})).unwrap(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 422);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_delete_startgg_key_clears_it(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "delkeyuser").await;

        sqlx::query!(
            "UPDATE users SET startgg_api_key = 'some-key' WHERE email = 'delkeyuser@test.com'"
        )
        .execute(&pool)
        .await
        .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/account/startgg-key")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 204);

        let key = sqlx::query_scalar!(
            "SELECT startgg_api_key FROM users WHERE email = 'delkeyuser@test.com'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(key.is_none());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_me_reflects_has_startgg_key(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "meuser").await;

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body["has_startgg_key"], false);

        sqlx::query!("UPDATE users SET startgg_api_key = 'k' WHERE email = 'meuser@test.com'")
            .execute(&pool)
            .await
            .unwrap();

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/auth/me")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = json_body(resp).await;
        assert_eq!(body["has_startgg_key"], true);
    }
}
