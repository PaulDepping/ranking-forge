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
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    state::AppState,
};
use common::models::User;

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        UserResponse {
            id: u.id,
            username: u.username,
            created_at: u.created_at,
        }
    }
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
            "SELECT u.id, u.username, u.password_hash, u.created_at
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

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn hash_password(password: String) -> Result<String> {
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

async fn verify_password(password: String, hash: String) -> Result<()> {
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

fn session_cookie(id: Uuid) -> Cookie<'static> {
    Cookie::build(("session_id", id.to_string()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(true)
        .max_age(cookie::time::Duration::seconds(2_592_000))
        .build()
}

fn clear_cookie() -> Cookie<'static> {
    Cookie::build(("session_id", ""))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(true)
        .max_age(cookie::time::Duration::seconds(0))
        .build()
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<AuthRequest>,
) -> Result<impl IntoResponse> {
    if body.username.len() < 3 {
        return Err(AppError::UnprocessableEntity(
            "username must be at least 3 characters".into(),
        ));
    }
    if body.username.len() > 50 {
        return Err(AppError::UnprocessableEntity(
            "username must be at most 50 characters".into(),
        ));
    }
    if body.password.len() < 8 {
        return Err(AppError::UnprocessableEntity(
            "password must be at least 8 characters".into(),
        ));
    }
    if body.password.len() > 128 {
        return Err(AppError::UnprocessableEntity(
            "password must be at most 128 characters".into(),
        ));
    }

    let password_hash = hash_password(body.password).await?;

    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (username, password_hash) VALUES ($1, $2)
         RETURNING id, username, password_hash, created_at",
        body.username,
        password_hash,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint() == Some("users_username_key") => {
            AppError::UnprocessableEntity("username already taken".into())
        }
        other => AppError::Db(other),
    })?;

    let session_id = create_session(&state.db, user.id).await?;
    let jar = jar.add(session_cookie(session_id));

    Ok((StatusCode::CREATED, jar, Json(UserResponse::from(user))))
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<AuthRequest>,
) -> Result<impl IntoResponse> {
    let user = sqlx::query_as!(
        User,
        "SELECT id, username, password_hash, created_at FROM users WHERE username = $1",
        body.username,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Unauthorized)?;

    verify_password(body.password, user.password_hash.clone()).await?;

    let session_id = create_session(&state.db, user.id).await?;
    let jar = jar.add(session_cookie(session_id));

    Ok((jar, Json(UserResponse::from(user))))
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> Result<impl IntoResponse> {
    if let Some(cookie) = jar.get("session_id") {
        if let Ok(session_id) = cookie.value().parse::<Uuid>() {
            sqlx::query!("DELETE FROM sessions WHERE id = $1", session_id)
                .execute(&state.db)
                .await?;
        }
    }

    let jar = jar.add(clear_cookie());
    Ok((StatusCode::NO_CONTENT, jar))
}

async fn me(auth: AuthUser) -> impl IntoResponse {
    Json(UserResponse::from(auth.0))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/me", get(me))
}
