use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    extract::{FromRequestParts, State},
    http::{StatusCode, request::Parts},
    response::IntoResponse,
    routing::{get, post},
};
use axum_extra::extract::CookieJar;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::LazyLock;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    extractors::ClientIpExtractor,
    state::AppState,
};
use common::models::User;

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub display_name: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub has_startgg_key: bool,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        UserResponse {
            id: u.id,
            email: u.email,
            display_name: u.display_name,
            has_startgg_key: u.startgg_api_key.is_some(),
            created_at: u.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session_id: Uuid,
    pub user: UserResponse,
}

// ── AuthUser extractor ────────────────────────────────────────────────────────

pub struct AuthUser(pub User);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> std::result::Result<Self, AppError> {
        let jar = CookieJar::from_request_parts(parts, state).await.unwrap();

        let session_id: Uuid = jar
            .get("session_id")
            .and_then(|c| c.value().parse().ok())
            .ok_or(AppError::Unauthorized)?;

        let user = sqlx::query_as!(
            User,
            "SELECT u.id, u.email, u.display_name, u.password_hash, u.startgg_api_key, u.created_at
             FROM sessions s
             JOIN users u ON u.id = s.user_id
             WHERE s.id = $1 AND s.expires_at > NOW()",
            session_id,
        )
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::Unauthorized)?;

        Ok(AuthUser(user))
    }
}

pub struct OptionalAuthUser(pub Option<User>);

impl FromRequestParts<AppState> for OptionalAuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> std::result::Result<Self, AppError> {
        let jar = CookieJar::from_request_parts(parts, state).await.unwrap();

        let session_id = jar
            .get("session_id")
            .and_then(|c| c.value().parse::<Uuid>().ok());

        let user = if let Some(sid) = session_id {
            sqlx::query_as!(
                User,
                "SELECT u.id, u.email, u.display_name, u.password_hash, u.startgg_api_key, u.created_at
                 FROM sessions s
                 JOIN users u ON u.id = s.user_id
                 WHERE s.id = $1 AND s.expires_at > NOW()",
                sid,
            )
            .fetch_optional(&state.db)
            .await?
        } else {
            None
        };

        Ok(OptionalAuthUser(user))
    }
}

// Equalize login response time when an email isn't found: verify_password against
// this hash so the code path matches a real wrong-password attempt (~100ms Argon2).
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(b"dummy_sentinel_never_matches", &salt)
        .unwrap()
        .to_string()
});

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(super) async fn hash_password(password: String) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| AppError::PasswordHash)
    })
    .await
    .map_err(|_| AppError::PasswordHash)?
}

pub(super) async fn verify_password(password: String, hash: String) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&hash).map_err(|_| AppError::PasswordHash)?;
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| AppError::Unauthorized)
    })
    .await
    .map_err(|_| AppError::PasswordHash)?
}

async fn create_session(db: &sqlx::PgPool, user_id: Uuid) -> Result<Uuid> {
    let session_id = Uuid::new_v4();
    let expires_at = Utc::now() + chrono::Duration::days(30);

    sqlx::query!(
        "INSERT INTO sessions (id, user_id, expires_at) VALUES ($1, $2, $3)",
        session_id,
        user_id,
        expires_at,
    )
    .execute(db)
    .await?;

    Ok(session_id)
}

fn is_valid_email(s: &str) -> bool {
    let parts: Vec<&str> = s.splitn(2, '@').collect();
    parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.')
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse> {
    if !is_valid_email(&body.email) {
        return Err(AppError::UnprocessableEntity(
            "invalid email address".into(),
        ));
    }
    if body.email.chars().count() > 255 {
        return Err(AppError::UnprocessableEntity(
            "email must be at most 255 characters".into(),
        ));
    }
    if body.display_name.chars().count() < 1 {
        return Err(AppError::UnprocessableEntity(
            "display name must not be empty".into(),
        ));
    }
    if body.display_name.chars().count() > 50 {
        return Err(AppError::UnprocessableEntity(
            "display name must be at most 50 characters".into(),
        ));
    }
    if body.password.chars().count() < 8 {
        return Err(AppError::UnprocessableEntity(
            "password must be at least 8 characters".into(),
        ));
    }
    if body.password.chars().count() > 128 {
        return Err(AppError::UnprocessableEntity(
            "password must be at most 128 characters".into(),
        ));
    }

    let password_hash = hash_password(body.password).await?;

    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (email, display_name, password_hash) VALUES ($1, $2, $3)
         RETURNING id, email, display_name, password_hash, startgg_api_key, created_at",
        body.email.to_lowercase(),
        body.display_name,
        password_hash,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint() == Some("users_email_key") => {
            AppError::UnprocessableEntity("email already registered".into())
        }
        other => AppError::Db(other),
    })?;

    let session_id = create_session(&state.db, user.id).await?;

    Ok((
        StatusCode::CREATED,
        Json(SessionResponse {
            session_id,
            user: UserResponse::from(user),
        }),
    ))
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse> {
    let user = sqlx::query_as!(
        User,
        "SELECT id, email, display_name, password_hash, startgg_api_key, created_at FROM users WHERE email = $1",
        body.email.to_lowercase(),
    )
    .fetch_optional(&state.db)
    .await?;

    let user = match user {
        Some(u) => u,
        None => {
            let _ = verify_password(body.password, (*DUMMY_HASH).clone()).await;
            return Err(AppError::Unauthorized);
        }
    };

    verify_password(body.password, user.password_hash.clone()).await?;

    let session_id = create_session(&state.db, user.id).await?;

    Ok(Json(SessionResponse {
        session_id,
        user: UserResponse::from(user),
    }))
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> Result<impl IntoResponse> {
    if let Some(cookie) = jar.get("session_id") {
        if let Ok(session_id) = cookie.value().parse::<Uuid>() {
            sqlx::query!("DELETE FROM sessions WHERE id = $1", session_id)
                .execute(&state.db)
                .await?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn me(auth: AuthUser) -> impl IntoResponse {
    Json(UserResponse::from(auth.0))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(ClientIpExtractor)
            .per_second(1)
            .burst_size(5)
            .finish()
            .expect("invalid rate-limit config"),
    );

    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .layer(GovernorLayer::new(governor_conf))
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
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::to_vec(
                            &json!({"email": format!("{name}@test.com"), "display_name": name, "password": "password123"}),
                        )
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_cleanup_deletes_expired_sessions_not_active(pool: PgPool) {
        let app = make_app(pool.clone());

        // Register creates one active session
        let _cookie = register(&app, "cleanup_user").await;

        let user_id =
            sqlx::query_scalar!("SELECT id FROM users WHERE email = 'cleanup_user@test.com'")
                .fetch_one(&pool)
                .await
                .unwrap();

        // Insert an already-expired session
        sqlx::query!(
            "INSERT INTO sessions (id, user_id, expires_at)
             VALUES (gen_random_uuid(), $1, NOW() - INTERVAL '1 hour')",
            user_id
        )
        .execute(&pool)
        .await
        .unwrap();

        // Two sessions now: one active, one expired
        let before = sqlx::query_scalar!("SELECT COUNT(*) FROM sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before, Some(2));

        // Run the cleanup
        let deleted = sqlx::query!("DELETE FROM sessions WHERE expires_at < NOW()")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(deleted.rows_affected(), 1);

        // Only the active session remains
        let after = sqlx::query_scalar!("SELECT COUNT(*) FROM sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(after, Some(1));
    }
}
