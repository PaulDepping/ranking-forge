# Account Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `/account` settings page (change email, display name, password; delete account) and redesign the user model so email is the login identifier and username becomes a non-unique display name.

**Architecture:** Rewrite the sole migration to add `email`/`display_name` to `users` and `owner_id` to `ranking_projects`, replacing the `project_members`-based ownership model. Add a new `routes/account.rs` module with three endpoints. Frontend gains a `/account` page with three shadcn Card sections.

**Tech Stack:** Rust/Axum, sqlx (Postgres), SvelteKit 5 (Svelte runes), shadcn-svelte, TypeScript.

---

## File Map

**Rewrite:**
- `backend/migrations/001_initial.sql` — schema rewrite (email, display_name, owner_id, enum fix, FK fix)

**Modify (backend):**
- `backend/crates/common/src/models/mod.rs` — User struct, split role enums
- `backend/crates/api/src/routes/auth.rs` — new request types, updated handlers
- `backend/crates/api/src/routes/projects.rs` — owner_id-based access, updated queries/tests
- `backend/crates/api/src/routes/members.rs` — owner_id transfer, email-based lookup, updated tests
- `backend/crates/api/src/routes/invite_links.rs` — MemberRole, owner_id check, updated tests
- `backend/crates/api/src/routes/mod.rs` — mount account router
- `backend/openapi.yaml` — UserResponse, auth request schemas, new account endpoints

**Create (backend):**
- `backend/crates/api/src/routes/account.rs` — profile, password, delete account handlers + tests

**Modify (frontend):**
- `web/src/lib/types.ts` — User, ProjectMember types
- `web/src/routes/login/+page.server.ts` — email field
- `web/src/routes/login/+page.svelte` — email input
- `web/src/routes/register/+page.server.ts` — email + display_name fields
- `web/src/routes/register/+page.svelte` — add email input, rename username
- `web/src/routes/+layout.svelte` — display_name, link to /account
- `web/tests/mock-api.js` — updated MOCK_USER, new account endpoints
- `web/tests/auth.test.ts` — update for email login
- `backend/crates/e2e/tests/full_flow.rs` — update register helper

**Create (frontend):**
- `web/src/routes/account/+page.server.ts`
- `web/src/routes/account/+page.svelte`

---

## Task 1: Rewrite the migration

**Files:**
- Modify: `backend/migrations/001_initial.sql`

- [ ] **Step 1: Rewrite the migration file**

Replace the entire contents of `backend/migrations/001_initial.sql`. Key changes: `users` gets `email UNIQUE` + non-unique `display_name`; `ranking_projects` gets `owner_id` with `ON DELETE CASCADE`; `project_member_role` enum drops `owner`; `project_invite_links.created_by` gets `ON DELETE CASCADE`.

```sql
-- Enums
CREATE TYPE job_kind AS ENUM ('import_tournaments');
CREATE TYPE job_status AS ENUM ('pending', 'running', 'done', 'failed');

-- Users and sessions
CREATE TABLE users (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT        NOT NULL UNIQUE,
    display_name  TEXT        NOT NULL,
    password_hash TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sessions (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX sessions_user_id_idx ON sessions(user_id);

-- Project membership roles (owner is stored on ranking_projects.owner_id, not here)
CREATE TYPE project_member_role AS ENUM ('editor', 'viewer');

-- Ranking projects
CREATE TABLE ranking_projects (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    game_id     BIGINT,
    game_name   TEXT,
    published   BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ranking_projects_owner_id_idx ON ranking_projects(owner_id);

-- Project membership (editors and viewers only; owner is in ranking_projects.owner_id)
CREATE TABLE project_members (
    project_id  UUID                NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    user_id     UUID                NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL,
    joined_at   TIMESTAMPTZ         NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, user_id)
);

CREATE INDEX project_members_user_id_idx ON project_members(user_id);

-- Invite links (editor or viewer only)
CREATE TABLE project_invite_links (
    id          UUID                PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID                NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL,
    created_by  UUID                NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at  TIMESTAMPTZ,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ         NOT NULL DEFAULT NOW()
);

CREATE INDEX project_invite_links_project_id_idx ON project_invite_links(project_id);

-- Players (project-scoped)
CREATE TABLE players (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id    UUID        NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    rank_position INTEGER     NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX players_project_id_idx ON players(project_id);
CREATE INDEX players_project_id_rank_idx ON players(project_id, rank_position);

-- start.gg accounts linked to players
CREATE TABLE startgg_accounts (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    player_id       UUID        NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    startgg_user_id BIGINT      NOT NULL,
    handle          TEXT        NOT NULL,
    display_name    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (player_id, startgg_user_id)
);

CREATE INDEX startgg_accounts_player_id_idx ON startgg_accounts(player_id);
CREATE INDEX startgg_accounts_user_id_idx ON startgg_accounts(startgg_user_id);

-- Tournaments (imported from start.gg, shared across projects)
CREATE TABLE tournaments (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id     BIGINT      NOT NULL UNIQUE,
    name           TEXT        NOT NULL,
    handle         TEXT        NOT NULL,
    city           TEXT,
    addr_state     TEXT,
    country_code   TEXT,
    venue_name     TEXT,
    venue_address  TEXT,
    timezone       TEXT,
    online         BOOLEAN     NOT NULL DEFAULT FALSE,
    num_attendees  INTEGER,
    start_at       TIMESTAMPTZ,
    end_at         TIMESTAMPTZ,
    lat            DOUBLE PRECISION,
    lng            DOUBLE PRECISION,
    state          INTEGER,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Events within tournaments
CREATE TABLE events (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID        NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    startgg_id    BIGINT      NOT NULL UNIQUE,
    name          TEXT        NOT NULL,
    handle        TEXT        NOT NULL,
    state         TEXT,
    is_online     BOOLEAN,
    event_type    INTEGER,
    min_team_size INTEGER,
    max_team_size INTEGER,
    game_id       BIGINT,
    game_name     TEXT,
    num_entrants  INTEGER,
    start_at      TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX events_start_at_idx ON events(start_at);
CREATE INDEX events_tournament_id_idx ON events(tournament_id);

-- Bracket phases within an event
CREATE TABLE phases (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT      NOT NULL UNIQUE,
    event_id      UUID        NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    name          TEXT,
    bracket_type  TEXT,
    phase_order   INTEGER,
    num_seeds     INTEGER,
    group_count   INTEGER,
    state         TEXT,
    is_exhibition BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX phases_event_id_idx ON phases(event_id);

-- Individual pools/brackets within a phase
CREATE TABLE phase_groups (
    id                 UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id         BIGINT      NOT NULL UNIQUE,
    phase_id           UUID        NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
    display_identifier TEXT,
    bracket_type       TEXT,
    bracket_url        TEXT,
    num_rounds         INTEGER,
    start_at           TIMESTAMPTZ,
    first_round_time   TIMESTAMPTZ,
    state              INTEGER
);

CREATE INDEX phase_groups_phase_id_idx ON phase_groups(phase_id);

-- Per-project event inclusion
CREATE TABLE project_events (
    project_id UUID    NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    event_id   UUID    NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    included   BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (project_id, event_id)
);

-- Entrants: one player's participation in one event
CREATE TABLE entrants (
    id                  UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id            UUID    NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    player_id           UUID    REFERENCES players(id) ON DELETE SET NULL,
    startgg_entrant_id  BIGINT  NOT NULL,
    startgg_user_id     BIGINT,
    seed                INTEGER,
    display_name        TEXT    NOT NULL,
    is_disqualified     BOOLEAN NOT NULL DEFAULT FALSE,
    final_placement     INTEGER,
    UNIQUE (event_id, startgg_entrant_id)
);

CREATE INDEX entrants_event_id_idx ON entrants(event_id);
CREATE INDEX entrants_player_id_idx ON entrants(player_id);
CREATE INDEX entrants_startgg_user_id_idx ON entrants(startgg_user_id);

-- Sets: match results
CREATE TABLE sets (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id          UUID        NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    phase_group_id    UUID        REFERENCES phase_groups(id),
    startgg_set_id    BIGINT      NOT NULL,
    winner_entrant_id UUID        NOT NULL REFERENCES entrants(id),
    loser_entrant_id  UUID        NOT NULL REFERENCES entrants(id),
    round             INTEGER,
    round_name        TEXT,
    total_games       SMALLINT,
    winner_score      SMALLINT,
    loser_score       SMALLINT,
    is_dq             BOOLEAN     NOT NULL DEFAULT FALSE,
    has_placeholder   BOOLEAN     NOT NULL DEFAULT FALSE,
    state             INTEGER,
    identifier        TEXT,
    vod_url           TEXT,
    completed_at      TIMESTAMPTZ,
    UNIQUE (event_id, startgg_set_id)
);

CREATE INDEX sets_event_id_idx ON sets(event_id);
CREATE INDEX sets_phase_group_id_idx ON sets(phase_group_id);
CREATE INDEX sets_winner_entrant_id_idx ON sets(winner_entrant_id);
CREATE INDEX sets_loser_entrant_id_idx ON sets(loser_entrant_id);
CREATE INDEX sets_completed_at_idx ON sets(completed_at);

-- Job queue for background worker
CREATE TABLE jobs (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    kind       job_kind    NOT NULL,
    project_id UUID        NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    params     JSONB       NOT NULL DEFAULT '{}',
    result     JSONB,
    status     job_status  NOT NULL DEFAULT 'pending',
    error      TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX jobs_status_idx ON jobs(status) WHERE status IN ('pending', 'running');
CREATE INDEX jobs_project_id_idx ON jobs(project_id);
```

- [ ] **Step 2: Commit**

```bash
git add backend/migrations/001_initial.sql
git commit -m "feat: rewrite migration — email login, display_name, owner_id on projects"
```

---

## Task 2: Update common models

**Files:**
- Modify: `backend/crates/common/src/models/mod.rs`

- [ ] **Step 1: Replace models/mod.rs**

Replace the entire file with the updated structs. `User` gains `email`, renames `username` to `display_name`. `ProjectMemberRole` is renamed to `MemberRole` (editor/viewer only, sqlx-mapped). A new `UserRole` enum (owner/editor/viewer, not sqlx-mapped) is added for response types. `ProjectMember` renames `username` to `display_name` and uses `MemberRole`. `ProjectInviteLink` uses `MemberRole`.

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub project_id: Uuid,
    pub params: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Project {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub published: bool,
    pub created_at: DateTime<Utc>,
}

/// DB-mapped role for project_members rows. Only editor and viewer — owner is
/// stored as ranking_projects.owner_id.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "project_member_role", rename_all = "snake_case")]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
    Editor,
    Viewer,
}

/// Role returned in API responses — includes Owner (synthesised from owner_id,
/// never stored in project_members).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    Owner,
    Editor,
    Viewer,
}

impl UserRole {
    pub fn satisfies(&self, min: &UserRole) -> bool {
        match (self, min) {
            (_, UserRole::Viewer) => true,
            (UserRole::Owner | UserRole::Editor, UserRole::Editor) => true,
            (UserRole::Owner, UserRole::Owner) => true,
            _ => false,
        }
    }
}

impl From<MemberRole> for UserRole {
    fn from(r: MemberRole) -> Self {
        match r {
            MemberRole::Editor => UserRole::Editor,
            MemberRole::Viewer => UserRole::Viewer,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectMember {
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub display_name: String,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectInviteLink {
    pub id: Uuid,
    pub project_id: Uuid,
    pub role: MemberRole,
    pub created_by: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Player {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub rank_position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct StartggAccount {
    pub id: Uuid,
    pub player_id: Uuid,
    pub startgg_user_id: i64,
    pub handle: String,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Commit**

```bash
git add backend/crates/common/src/models/mod.rs
git commit -m "feat: update common models — email/display_name on User, MemberRole/UserRole split"
```

---

## Task 3: Update auth.rs

**Files:**
- Modify: `backend/crates/api/src/routes/auth.rs`

- [ ] **Step 1: Update auth.rs**

Key changes:
- Split `AuthRequest` into `RegisterRequest` (email + display_name + password) and `LoginRequest` (email + password)
- Update `UserResponse` to use `email` and `display_name`
- `register`: validate email format, display_name length; insert with email + display_name; 422 on duplicate email
- `login`: look up by email
- Make `hash_password` and `verify_password` `pub(super)` for use in account.rs
- `AuthUser` and `OptionalAuthUser` SELECT queries updated for new User fields

```rust
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
use std::sync::Arc;
use std::sync::LazyLock;
use tower_governor::{GovernorLayer, GovernorError, governor::GovernorConfigBuilder};
use tower_governor::key_extractor::KeyExtractor;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    state::AppState,
};
use common::models::User;

#[derive(Clone)]
struct ClientIpExtractor;

impl KeyExtractor for ClientIpExtractor {
    type Key = std::net::IpAddr;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> std::result::Result<Self::Key, GovernorError> {
        let forwarded = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse::<std::net::IpAddr>().ok());

        if let Some(ip) = forwarded {
            return Ok(ip);
        }

        if let Some(info) = req
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        {
            return Ok(info.0.ip());
        }

        Ok(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
    }
}

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

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        UserResponse {
            id: u.id,
            email: u.email,
            display_name: u.display_name,
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
            "SELECT u.id, u.email, u.display_name, u.password_hash, u.created_at
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
                "SELECT u.id, u.email, u.display_name, u.password_hash, u.created_at
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

pub(super) fn session_cookie(id: Uuid) -> Cookie<'static> {
    Cookie::build(("session_id", id.to_string()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(true)
        .max_age(cookie::time::Duration::seconds(2_592_000))
        .build()
}

pub(super) fn clear_cookie() -> Cookie<'static> {
    Cookie::build(("session_id", ""))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .secure(true)
        .max_age(cookie::time::Duration::seconds(0))
        .build()
}

fn is_valid_email(s: &str) -> bool {
    let parts: Vec<&str> = s.splitn(2, '@').collect();
    parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.')
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn register(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse> {
    if !is_valid_email(&body.email) {
        return Err(AppError::UnprocessableEntity("invalid email address".into()));
    }
    if body.email.chars().count() > 255 {
        return Err(AppError::UnprocessableEntity("email must be at most 255 characters".into()));
    }
    if body.display_name.chars().count() < 1 {
        return Err(AppError::UnprocessableEntity("display name must not be empty".into()));
    }
    if body.display_name.chars().count() > 50 {
        return Err(AppError::UnprocessableEntity("display name must be at most 50 characters".into()));
    }
    if body.password.chars().count() < 8 {
        return Err(AppError::UnprocessableEntity("password must be at least 8 characters".into()));
    }
    if body.password.chars().count() > 128 {
        return Err(AppError::UnprocessableEntity("password must be at most 128 characters".into()));
    }

    let password_hash = hash_password(body.password).await?;

    let user = sqlx::query_as!(
        User,
        "INSERT INTO users (email, display_name, password_hash) VALUES ($1, $2, $3)
         RETURNING id, email, display_name, password_hash, created_at",
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
    let jar = jar.add(session_cookie(session_id));

    Ok((StatusCode::CREATED, jar, Json(UserResponse::from(user))))
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse> {
    let user = sqlx::query_as!(
        User,
        "SELECT id, email, display_name, password_hash, created_at FROM users WHERE email = $1",
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
```

- [ ] **Step 2: Commit**

```bash
git add backend/crates/api/src/routes/auth.rs
git commit -m "feat: update auth — email login, display_name, split register/login request types"
```

---

## Task 4: Update projects.rs

**Files:**
- Modify: `backend/crates/api/src/routes/projects.rs`

- [ ] **Step 1: Update the imports and Project struct usage**

At the top of `projects.rs`, change the import from `ProjectMemberRole` to `MemberRole, UserRole`:

```rust
use common::models::{MemberRole, Project, UserRole};
```

- [ ] **Step 2: Update ProjectResponse**

```rust
#[derive(Serialize)]
pub struct ProjectResponse {
    pub id: Uuid,
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub published: bool,
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
            published: p.published,
            created_at: p.created_at,
            user_role,
        }
    }
}
```

- [ ] **Step 3: Rewrite require_project_access**

Owner is now in `ranking_projects.owner_id`. A LEFT JOIN fetches the member role for non-owners.

```rust
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
        published: bool,
        created_at: DateTime<Utc>,
        is_owner: Option<bool>,
        member_role: Option<MemberRole>,
    }

    let row = sqlx::query_as!(
        Row,
        r#"SELECT p.id, p.owner_id, p.name, p.game_id, p.game_name, p.published, p.created_at,
                  (p.owner_id = $2) AS is_owner,
                  pm.role AS "member_role: MemberRole"
           FROM ranking_projects p
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
        row.member_role.map(UserRole::from).ok_or(AppError::NotFound)?
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
            published: row.published,
            created_at: row.created_at,
        },
        role,
    ))
}
```

- [ ] **Step 4: Rewrite require_project_read_access**

```rust
pub async fn require_project_read_access(
    db: &PgPool,
    project_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<(Project, Option<UserRole>)> {
    let project = sqlx::query_as!(
        Project,
        "SELECT id, owner_id, name, game_id, game_name, published, created_at
         FROM ranking_projects WHERE id = $1",
        project_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    if project.published {
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
```

- [ ] **Step 5: Rewrite list_projects**

```rust
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
        published: bool,
        created_at: DateTime<Utc>,
        is_owner: Option<bool>,
        member_role: Option<MemberRole>,
    }

    let rows = sqlx::query_as!(
        Row,
        r#"SELECT p.id, p.owner_id, p.name, p.game_id, p.game_name, p.published, p.created_at,
                  (p.owner_id = $1) AS is_owner,
                  pm.role AS "member_role: MemberRole"
           FROM ranking_projects p
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
                r.member_role.map(UserRole::from).unwrap_or(UserRole::Viewer)
            };
            ProjectResponse {
                id: r.id,
                name: r.name,
                game_id: r.game_id,
                game_name: r.game_name,
                published: r.published,
                created_at: r.created_at,
                user_role: Some(role),
            }
        })
        .collect();
    Ok(Json(resp))
}
```

- [ ] **Step 6: Rewrite create_project**

Remove the `project_members` insert. Use `owner_id` directly.

```rust
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

    let project = sqlx::query_as!(
        Project,
        "INSERT INTO ranking_projects (owner_id, name, game_id, game_name)
         VALUES ($1, $2, $3, $4)
         RETURNING id, owner_id, name, game_id, game_name, published, created_at",
        user.id,
        body.name.trim(),
        body.game_id,
        body.game_name,
    )
    .fetch_one(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ProjectResponse::from_project(project, Some(UserRole::Owner))),
    ))
}
```

- [ ] **Step 7: Update remaining handlers that call require_project_access**

In `patch_project` and `delete_project`, change `ProjectMemberRole::Owner` to `UserRole::Owner`:

```rust
// patch_project:
let (project, role) =
    require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

// delete_project:
require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;
```

- [ ] **Step 8: Update tests in projects.rs**

The `register` helper changes to use email. Queries that look up users by `username` change to look up by `email` (using `display_name` as the label, `{name}@test.com` as email).

```rust
async fn register(app: &Router, name: &str) -> String {
    let resp = app.clone().oneshot(
        Request::builder().method("POST").uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(
                &json!({"email": format!("{name}@test.com"), "display_name": name, "password": "password123"})
            ).unwrap())).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.headers().get("set-cookie").unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string()
}
```

Update every query in tests that uses `WHERE username = '...'` to `WHERE email = '...@test.com'`:

```rust
// Before:
let editor_id = sqlx::query_scalar!("SELECT id FROM users WHERE username = 'editor1'")
// After:
let editor_id = sqlx::query_scalar!("SELECT id FROM users WHERE email = 'editor1@test.com'")
```

Also update `test_create_project_inserts_owner_membership` — it now checks `ranking_projects.owner_id` instead of `project_members.role`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_create_project_sets_owner_id(pool: PgPool) {
    let app = make_app(pool.clone());
    let cookie = register(&app, "owner2").await;
    let proj_id = create_project(&app, &cookie, "My Project").await;

    let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
    let owner_id = sqlx::query_scalar!(
        "SELECT owner_id FROM ranking_projects WHERE id = $1",
        proj_uuid
    ).fetch_one(&pool).await.unwrap();

    let user_id = sqlx::query_scalar!(
        "SELECT id FROM users WHERE email = 'owner2@test.com'"
    ).fetch_one(&pool).await.unwrap();

    assert_eq!(owner_id, user_id);
}
```

Update `test_list_projects_shows_all_member_roles`: the project is now owned by `owner1` via `owner_id`. The editor SQL INSERT into `project_members` stays the same (role = 'editor'). The assertions stay the same — `owner1`'s list shows `user_role: "owner"`, `editor1`'s list shows `user_role: "editor"`. The only change needed is the `register` helper and the `WHERE username =` query:

```rust
let editor_id = sqlx::query_scalar!("SELECT id FROM users WHERE email = 'editor1@test.com'")
    .fetch_one(&pool).await.unwrap();
```

- [ ] **Step 9: Run backend tests**

```bash
cd backend && bash test.sh
```

Expected: all tests pass (may need DATABASE_URL set; see backend/test.sh).

- [ ] **Step 10: Commit**

```bash
git add backend/crates/api/src/routes/projects.rs
git commit -m "feat: switch project ownership to owner_id column"
```

---

## Task 5: Update members.rs

**Files:**
- Modify: `backend/crates/api/src/routes/members.rs`

- [ ] **Step 1: Update imports and request types**

```rust
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
```

- [ ] **Step 2: Rewrite list_members**

```rust
async fn list_members(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

    let members = sqlx::query_as!(
        ProjectMember,
        r#"SELECT pm.project_id, pm.user_id, u.display_name,
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
```

- [ ] **Step 3: Rewrite add_member**

Look up by email. Check target is not the project owner.

```rust
async fn add_member(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<AddMemberRequest>,
) -> Result<impl IntoResponse> {
    let (project, _) = require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

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
```

- [ ] **Step 4: Rewrite change_member_role**

```rust
async fn change_member_role(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, target_user_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ChangeMemberRoleRequest>,
) -> Result<impl IntoResponse> {
    let (project, _) = require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

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
```

- [ ] **Step 5: Rewrite remove_member**

```rust
async fn remove_member(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse> {
    let (project, _) = require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;

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
```

- [ ] **Step 6: Rewrite transfer_ownership**

Single `UPDATE` on `ranking_projects.owner_id`. Validate target is an existing member.

```rust
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

    let target_is_member = sqlx::query_scalar!(
        "SELECT 1 AS one FROM project_members WHERE project_id = $1 AND user_id = $2",
        project_id,
        body.user_id,
    )
    .fetch_optional(&state.db)
    .await?;

    if target_is_member.is_none() {
        return Err(AppError::UnprocessableEntity(
            "target user is not a member of this project".into(),
        ));
    }

    sqlx::query!(
        "UPDATE ranking_projects SET owner_id = $1 WHERE id = $2",
        body.user_id,
        project_id,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 7: Update tests in members.rs**

Update the `register` helper (same pattern as Task 4 Step 8). Update any `WHERE username = '...'` queries to `WHERE email = '...@test.com'`. Update `test_add_member_and_list` to use `email` in the request body:

```rust
async fn register(app: &Router, name: &str) -> String {
    let resp = app.clone().oneshot(
        Request::builder().method("POST").uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(
                &json!({"email": format!("{name}@test.com"), "display_name": name, "password": "password123"})
            ).unwrap())).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.headers().get("set-cookie").unwrap().to_str().unwrap()
        .split(';').next().unwrap().to_string()
}
```

Update `test_add_member_and_list` to send email instead of username:

```rust
.body(Body::from(serde_json::to_vec(
    &json!({"email": "mem_user@test.com", "role": "editor"})
).unwrap())).unwrap()
```

Update `test_transfer_ownership` — the DB check now queries `ranking_projects.owner_id`:

```rust
let new_owner_id = sqlx::query_scalar!(
    "SELECT id FROM users WHERE email = 'new_owner@test.com'"
).fetch_one(&pool).await.unwrap();

// ... after transfer:
let owner_id = sqlx::query_scalar!(
    "SELECT owner_id FROM ranking_projects WHERE id = $1",
    proj_uuid
).fetch_one(&pool).await.unwrap();
assert_eq!(owner_id, new_owner_id);
```

- [ ] **Step 8: Commit**

```bash
git add backend/crates/api/src/routes/members.rs
git commit -m "feat: update members — owner_id transfer, email lookup, MemberRole"
```

---

## Task 6: Update invite_links.rs

**Files:**
- Modify: `backend/crates/api/src/routes/invite_links.rs`

- [ ] **Step 1: Update imports**

```rust
use common::models::{MemberRole, ProjectInviteLink, UserRole};
```

- [ ] **Step 2: Update CreateInviteLinkRequest**

```rust
#[derive(Deserialize)]
struct CreateInviteLinkRequest {
    role: MemberRole,
    expires_at: Option<DateTime<Utc>>,
}
```

- [ ] **Step 3: Update list_invite_links, create_invite_link, revoke_invite_link**

Change `ProjectMemberRole::Owner` → `UserRole::Owner` in the three `require_project_access` calls. Remove the `body.role == ProjectMemberRole::Owner` guard (no longer possible since `MemberRole` has no `Owner` variant). Update query type annotations from `ProjectMemberRole` to `MemberRole`:

```rust
async fn list_invite_links(...) -> Result<impl IntoResponse> {
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

async fn create_invite_link(...) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Owner).await?;
    // No Owner guard needed — MemberRole has no Owner variant

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
```

- [ ] **Step 4: Update accept_invite_link**

The owner check changes from `project_members WHERE role = 'owner'` to `ranking_projects.owner_id`:

```rust
// Replace the is_owner block:
let is_owner = sqlx::query_scalar!(
    "SELECT 1 AS one FROM ranking_projects WHERE id = $1 AND owner_id = $2",
    link.project_id,
    user.id,
)
.fetch_optional(&state.db)
.await?;

// Also update the role cast in the INSERT:
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
```

- [ ] **Step 5: Update tests in invite_links.rs**

Same `register` helper update as Tasks 4/5. Update the DB verification query:

```rust
let role = sqlx::query_scalar!(
    r#"SELECT role::text as "role: String" FROM project_members
       WHERE project_id = $1 AND user_id = (SELECT id FROM users WHERE email = 'inv_user@test.com')"#,
    proj_uuid
).fetch_one(&pool).await.unwrap();
```

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/invite_links.rs
git commit -m "feat: update invite_links — MemberRole, owner_id check"
```

---

## Task 7: Add account.rs and mount it

**Files:**
- Create: `backend/crates/api/src/routes/account.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`

- [ ] **Step 1: Create account.rs**

```rust
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, patch},
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, clear_cookie, hash_password, verify_password},
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
            return Err(AppError::UnprocessableEntity("display name must not be empty".into()));
        }
        if name.chars().count() > 50 {
            return Err(AppError::UnprocessableEntity(
                "display name must be at most 50 characters".into(),
            ));
        }
    }

    if let Some(ref email) = body.email {
        if !is_valid_email(email) {
            return Err(AppError::UnprocessableEntity("invalid email address".into()));
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
    jar: CookieJar,
) -> Result<impl IntoResponse> {
    sqlx::query!("DELETE FROM users WHERE id = $1", user.id)
        .execute(&state.db)
        .await?;

    let jar = jar.add(clear_cookie());
    Ok((StatusCode::NO_CONTENT, jar))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/profile", patch(update_profile))
        .route("/password", patch(update_password))
        .route("", delete(delete_account))
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

    async fn register(app: &Router, name: &str) -> String {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &json!({"email": format!("{name}@test.com"), "display_name": name, "password": "password123"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        resp.headers().get("set-cookie").unwrap().to_str().unwrap()
            .split(';').next().unwrap().to_string()
    }

    async fn json_body(resp: axum::response::Response) -> Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_update_profile_display_name(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "profuser").await;

        let resp = app.clone().oneshot(
            Request::builder().method("PATCH").uri("/account/profile")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(
                    &json!({"display_name": "New Name"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 204);

        let name = sqlx::query_scalar!(
            "SELECT display_name FROM users WHERE email = 'profuser@test.com'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(name, "New Name");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_update_profile_duplicate_email(pool: PgPool) {
        let app = make_app(pool.clone());
        let _c1 = register(&app, "dupuser1").await;
        let c2 = register(&app, "dupuser2").await;

        let resp = app.clone().oneshot(
            Request::builder().method("PATCH").uri("/account/profile")
                .header("content-type", "application/json")
                .header("cookie", &c2)
                .body(Body::from(serde_json::to_vec(
                    &json!({"email": "dupuser1@test.com"})
                ).unwrap())).unwrap()
        ).await.unwrap();
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

        // Can now login with new password
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(
                    &json!({"email": "pwuser2@test.com", "password": "newpassword456"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_delete_account_cascades_projects(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "deluser").await;

        // Create a project
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/projects")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(&json!({"name": "My Project"})).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 201);
        let body: Value = json_body(resp).await;
        let project_id: uuid::Uuid = body["id"].as_str().unwrap().parse().unwrap();

        // Delete account
        let resp = app.clone().oneshot(
            Request::builder().method("DELETE").uri("/account")
                .header("cookie", &cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 204);

        // Project is gone
        let count = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM ranking_projects WHERE id = $1",
            project_id
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(count, Some(0));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_delete_account_clears_session(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "sessuser").await;

        app.clone().oneshot(
            Request::builder().method("DELETE").uri("/account")
                .header("cookie", &cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();

        // /auth/me now returns 401
        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri("/auth/me")
                .header("cookie", &cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 401);
    }
}
```

- [ ] **Step 2: Mount account router in mod.rs**

```rust
pub mod account;
pub mod auth;
pub mod games;
pub mod import;
pub mod invite_links;
pub mod members;
pub mod players;
pub mod projects;
pub mod tournaments;

use axum::{Router, routing::{get, post}};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/auth", auth::router())
        .nest("/account", account::router())
        .nest("/projects", projects::router())
        .route("/games", get(games::search_games))
        .route("/invite/{token}/accept", post(invite_links::accept_invite_link))
}
```

- [ ] **Step 3: Run backend tests**

```bash
cd backend && bash test.sh
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add backend/crates/api/src/routes/account.rs backend/crates/api/src/routes/mod.rs
git commit -m "feat: add account routes — profile, password, delete account"
```

---

## Task 8: Update e2e tests

**Files:**
- Modify: `backend/crates/e2e/tests/full_flow.rs`

- [ ] **Step 1: Update register helper in full_flow.rs**

Find the `register` async fn and change the request body:

```rust
async fn register(app: &Router, username: &str, password: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "email": format!("{username}@test.com"),
                "display_name": username,
                "password": password
            })).unwrap(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    resp.headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}
```

- [ ] **Step 2: Run e2e tests**

```bash
cd backend && bash test.sh
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add backend/crates/e2e/tests/full_flow.rs
git commit -m "test: update e2e register helper for email-based login"
```

---

## Task 9: Update sqlx offline cache

**Files:**
- Modify: `backend/.sqlx/` (auto-generated)

- [ ] **Step 1: Run prepare-sqlx.sh**

This requires Docker (starts an ephemeral Postgres container):

```bash
cd backend && bash prepare-sqlx.sh
```

Expected: `query data written to .sqlx/` with no errors.

- [ ] **Step 2: Verify build works offline**

```bash
cd backend && SQLX_OFFLINE=true cargo build
```

Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add backend/.sqlx/
git commit -m "chore: update sqlx offline query cache"
```

---

## Task 10: Update openapi.yaml

**Files:**
- Modify: `backend/openapi.yaml`

- [ ] **Step 1: Update UserResponse schema**

Find the `UserResponse` schema and update it:

```yaml
UserResponse:
  type: object
  required: [id, email, display_name, created_at]
  properties:
    id:
      type: string
      format: uuid
    email:
      type: string
      format: email
    display_name:
      type: string
    created_at:
      type: string
      format: date-time
```

- [ ] **Step 2: Update auth request schemas**

Update the register request body to `email`, `display_name`, `password`. Update the login request body to `email`, `password`.

- [ ] **Step 3: Add account endpoints**

Add under `paths`:

```yaml
/account/profile:
  patch:
    summary: Update profile
    security:
      - cookieAuth: []
    requestBody:
      required: true
      content:
        application/json:
          schema:
            type: object
            properties:
              display_name:
                type: string
              email:
                type: string
                format: email
    responses:
      '204':
        description: Updated
      '401':
        description: Unauthorized
      '422':
        description: Validation error

/account/password:
  patch:
    summary: Change password
    security:
      - cookieAuth: []
    requestBody:
      required: true
      content:
        application/json:
          schema:
            type: object
            required: [current_password, new_password]
            properties:
              current_password:
                type: string
              new_password:
                type: string
    responses:
      '204':
        description: Password changed
      '401':
        description: Wrong current password

/account:
  delete:
    summary: Delete account
    security:
      - cookieAuth: []
    responses:
      '204':
        description: Account deleted
      '401':
        description: Unauthorized
```

- [ ] **Step 4: Update ProjectMember schema**

Change `username` to `display_name` in the `ProjectMember` schema. Change `role` enum values to `[editor, viewer]`.

- [ ] **Step 5: Commit**

```bash
git add backend/openapi.yaml
git commit -m "docs: update openapi — email login, account endpoints, UserResponse"
```

---

## Task 11: Update frontend types and auth pages

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/routes/login/+page.server.ts`
- Modify: `web/src/routes/login/+page.svelte`
- Modify: `web/src/routes/register/+page.server.ts`
- Modify: `web/src/routes/register/+page.svelte`
- Modify: `web/src/routes/+layout.svelte`

- [ ] **Step 1: Update types.ts**

Update the `User` interface:

```typescript
export interface User {
  id: string;
  email: string;
  display_name: string;
  created_at: string;
}
```

Update `ProjectMember`:

```typescript
export interface ProjectMember {
  project_id: string;
  user_id: string;
  display_name: string;
  role: 'editor' | 'viewer';
  joined_at: string;
}
```

- [ ] **Step 2: Update login/+page.server.ts**

```typescript
import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { env } from '$env/dynamic/private';

export const load: PageServerLoad = ({ locals }) => {
  if (locals.user) redirect(303, '/projects');
};

export const actions: Actions = {
  default: async ({ fetch, request, cookies }) => {
    const data = await request.formData();
    const email = data.get('email') as string;
    const password = data.get('password') as string;

    const res = await fetch(`${env.INTERNAL_API_URL}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, password })
    });

    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Login failed' }));
      return fail(res.status, { error: body.message ?? 'Login failed' });
    }

    const setCookie = res.headers.get('set-cookie');
    const match = setCookie?.match(/session_id=([^;]+)/);
    if (match) {
      cookies.set('session_id', match[1], {
        path: '/',
        httpOnly: true,
        sameSite: 'strict',
        maxAge: 60 * 60 * 24 * 30
      });
    }

    redirect(303, '/projects');
  }
};
```

- [ ] **Step 3: Update login/+page.svelte**

```svelte
<script lang="ts">
  import { enhance } from '$app/forms';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { Alert } from '$lib/components/ui/alert';

  let { form } = $props();
</script>

<div class="flex min-h-[60vh] items-center justify-center">
  <div class="w-full max-w-sm space-y-6">
    <div class="space-y-1 text-center">
      <p class="text-lg font-semibold">Sign in to your account</p>
    </div>

    {#if form?.error}
      <Alert variant="destructive">{form.error}</Alert>
    {/if}

    <form method="POST" use:enhance class="space-y-4">
      <div class="space-y-2">
        <Label for="email">Email</Label>
        <Input id="email" name="email" type="email" required autocomplete="email" />
      </div>
      <div class="space-y-2">
        <Label for="password">Password</Label>
        <Input id="password" name="password" type="password" required autocomplete="current-password" />
      </div>
      <Button type="submit" class="w-full">Sign in</Button>
    </form>

    <p class="text-center text-sm text-muted-foreground">
      No account? <a href="/register" class="underline hover:text-foreground">Register</a>
    </p>
  </div>
</div>
```

- [ ] **Step 4: Update register/+page.server.ts**

```typescript
import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { env } from '$env/dynamic/private';

export const load: PageServerLoad = ({ locals }) => {
  if (locals.user) redirect(303, '/projects');
};

export const actions: Actions = {
  default: async ({ fetch, request, cookies }) => {
    const data = await request.formData();
    const email = data.get('email') as string;
    const display_name = data.get('display_name') as string;
    const password = data.get('password') as string;
    const confirmPassword = data.get('confirm_password') as string;

    if (password !== confirmPassword) {
      return fail(400, { error: 'Passwords do not match' });
    }

    const res = await fetch(`${env.INTERNAL_API_URL}/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, display_name, password })
    });

    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Registration failed' }));
      return fail(res.status, { error: body.message ?? 'Registration failed' });
    }

    const setCookie = res.headers.get('set-cookie');
    const match = setCookie?.match(/session_id=([^;]+)/);
    if (match) {
      cookies.set('session_id', match[1], {
        path: '/',
        httpOnly: true,
        sameSite: 'strict',
        maxAge: 60 * 60 * 24 * 30
      });
    }

    redirect(303, '/projects');
  }
};
```

- [ ] **Step 5: Update register/+page.svelte**

```svelte
<script lang="ts">
  import { enhance } from '$app/forms';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { Alert } from '$lib/components/ui/alert';

  let { form } = $props();
</script>

<div class="flex min-h-[60vh] items-center justify-center">
  <div class="w-full max-w-sm space-y-6">
    <div class="space-y-1 text-center">
      <p class="text-lg font-semibold">Create an account</p>
    </div>

    {#if form?.error}
      <Alert variant="destructive">{form.error}</Alert>
    {/if}

    <form method="POST" use:enhance class="space-y-4">
      <div class="space-y-2">
        <Label for="email">Email</Label>
        <Input id="email" name="email" type="email" required autocomplete="email" />
      </div>
      <div class="space-y-2">
        <Label for="display_name">Display name</Label>
        <Input id="display_name" name="display_name" required maxlength={50} autocomplete="nickname" />
      </div>
      <div class="space-y-2">
        <Label for="password">Password</Label>
        <Input id="password" name="password" type="password" required minlength={8} autocomplete="new-password" />
      </div>
      <div class="space-y-2">
        <Label for="confirm_password">Confirm password</Label>
        <Input id="confirm_password" name="confirm_password" type="password" required minlength={8} autocomplete="new-password" />
      </div>
      <Button type="submit" class="w-full">Create account</Button>
    </form>

    <p class="text-center text-sm text-muted-foreground">
      Already have an account? <a href="/login" class="underline hover:text-foreground">Sign in</a>
    </p>
  </div>
</div>
```

- [ ] **Step 6: Update +layout.svelte**

Change `data.user.username` to `data.user.display_name` and wrap it in a link to `/account`:

```svelte
{#if data.user}
  <a href="/account" class="text-sm text-muted-foreground hover:text-foreground">{data.user.display_name}</a>
  <Button variant="ghost" size="sm" onclick={logout}>Logout</Button>
{:else}
```

- [ ] **Step 7: Run frontend unit tests**

```bash
cd web && npm run test:unit
```

Expected: all tests pass (mock API uses updated User shape in next task).

- [ ] **Step 8: Commit**

```bash
git add web/src/lib/types.ts \
        web/src/routes/login/+page.server.ts \
        web/src/routes/login/+page.svelte \
        web/src/routes/register/+page.server.ts \
        web/src/routes/register/+page.svelte \
        web/src/routes/+layout.svelte
git commit -m "feat: update frontend — email login, display_name, account link in nav"
```

---

## Task 12: Add account page

**Files:**
- Create: `web/src/routes/account/+page.server.ts`
- Create: `web/src/routes/account/+page.svelte`

- [ ] **Step 1: Create account/+page.server.ts**

```typescript
import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { env } from '$env/dynamic/private';

export const load: PageServerLoad = ({ locals }) => {
  if (!locals.user) redirect(303, '/login');
  return { user: locals.user };
};

export const actions: Actions = {
  updateProfile: async ({ fetch, request, locals }) => {
    if (!locals.user) redirect(303, '/login');
    const data = await request.formData();
    const display_name = (data.get('display_name') as string).trim();
    const email = (data.get('email') as string).trim();

    const res = await fetch(`${env.INTERNAL_API_URL}/account/profile`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ display_name, email })
    });

    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Update failed' }));
      return fail(res.status, { profileError: body.message ?? 'Update failed' });
    }

    return { profileSuccess: true };
  },

  updatePassword: async ({ fetch, request, locals }) => {
    if (!locals.user) redirect(303, '/login');
    const data = await request.formData();
    const current_password = data.get('current_password') as string;
    const new_password = data.get('new_password') as string;
    const confirm_password = data.get('confirm_password') as string;

    if (new_password !== confirm_password) {
      return fail(400, { passwordError: 'Passwords do not match' });
    }

    const res = await fetch(`${env.INTERNAL_API_URL}/account/password`, {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ current_password, new_password })
    });

    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Password change failed' }));
      return fail(res.status, { passwordError: body.message ?? 'Password change failed' });
    }

    return { passwordSuccess: true };
  },

  deleteAccount: async ({ fetch, locals, cookies }) => {
    if (!locals.user) redirect(303, '/login');

    await fetch(`${env.INTERNAL_API_URL}/account`, { method: 'DELETE' });

    cookies.delete('session_id', { path: '/' });
    redirect(303, '/login');
  }
};
```

- [ ] **Step 2: Create account/+page.svelte**

```svelte
<script lang="ts">
  import { enhance } from '$app/forms';
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Label } from '$lib/components/ui/label';
  import { Alert } from '$lib/components/ui/alert';
  import * as AlertDialog from '$lib/components/ui/alert-dialog';

  let { data, form } = $props();
</script>

<div class="mx-auto max-w-2xl space-y-6">
  <h1 class="text-2xl font-semibold">Account settings</h1>

  <!-- Profile card -->
  <Card.Root>
    <Card.Header>
      <Card.Title>Profile</Card.Title>
      <Card.Description>Update your display name and email address.</Card.Description>
    </Card.Header>
    <form method="POST" action="?/updateProfile" use:enhance>
      <Card.Content class="space-y-4">
        {#if form?.profileError}
          <Alert variant="destructive">{form.profileError}</Alert>
        {/if}
        {#if form?.profileSuccess}
          <Alert>Profile updated.</Alert>
        {/if}
        <div class="space-y-2">
          <Label for="display_name">Display name</Label>
          <Input
            id="display_name"
            name="display_name"
            required
            maxlength={50}
            value={data.user.display_name}
          />
        </div>
        <div class="space-y-2">
          <Label for="email">Email</Label>
          <Input
            id="email"
            name="email"
            type="email"
            required
            value={data.user.email}
          />
        </div>
      </Card.Content>
      <Card.Footer class="justify-end">
        <Button type="submit">Save changes</Button>
      </Card.Footer>
    </form>
  </Card.Root>

  <!-- Password card -->
  <Card.Root>
    <Card.Header>
      <Card.Title>Password</Card.Title>
      <Card.Description>Use a strong, unique password.</Card.Description>
    </Card.Header>
    <form method="POST" action="?/updatePassword" use:enhance>
      <Card.Content class="space-y-4">
        {#if form?.passwordError}
          <Alert variant="destructive">{form.passwordError}</Alert>
        {/if}
        {#if form?.passwordSuccess}
          <Alert>Password changed.</Alert>
        {/if}
        <div class="space-y-2">
          <Label for="current_password">Current password</Label>
          <Input id="current_password" name="current_password" type="password" required autocomplete="current-password" />
        </div>
        <div class="space-y-2">
          <Label for="new_password">New password</Label>
          <Input id="new_password" name="new_password" type="password" required minlength={8} autocomplete="new-password" />
        </div>
        <div class="space-y-2">
          <Label for="confirm_password">Confirm new password</Label>
          <Input id="confirm_password" name="confirm_password" type="password" required minlength={8} autocomplete="new-password" />
        </div>
      </Card.Content>
      <Card.Footer class="justify-end">
        <Button type="submit">Change password</Button>
      </Card.Footer>
    </form>
  </Card.Root>

  <!-- Danger zone card -->
  <Card.Root class="border-destructive">
    <Card.Header>
      <Card.Title class="text-destructive">Delete account</Card.Title>
      <Card.Description>
        Permanently deletes your account and all projects you own. This cannot be undone.
      </Card.Description>
    </Card.Header>
    <Card.Footer class="justify-end">
      <AlertDialog.Root>
        <AlertDialog.Trigger>
          {#snippet child({ props })}
            <Button variant="destructive" {...props}>Delete account</Button>
          {/snippet}
        </AlertDialog.Trigger>
        <AlertDialog.Content>
          <AlertDialog.Header>
            <AlertDialog.Title>Are you absolutely sure?</AlertDialog.Title>
            <AlertDialog.Description>
              This will permanently delete your account and all projects you own.
              This action cannot be undone.
            </AlertDialog.Description>
          </AlertDialog.Header>
          <AlertDialog.Footer>
            <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
            <form method="POST" action="?/deleteAccount">
              <AlertDialog.Action type="submit">Delete my account</AlertDialog.Action>
            </form>
          </AlertDialog.Footer>
        </AlertDialog.Content>
      </AlertDialog.Root>
    </Card.Footer>
  </Card.Root>
</div>
```

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/account/
git commit -m "feat: add account page — profile, password, delete account"
```

---

## Task 13: Update mock API and e2e tests

**Files:**
- Modify: `web/tests/mock-api.js`
- Modify: `web/tests/auth.test.ts`

- [ ] **Step 1: Update MOCK_USER in mock-api.js**

```js
const MOCK_USER = {
  id: 'user-1',
  email: 'testuser@example.com',
  display_name: 'testuser',
  created_at: '2026-01-01T00:00:00Z'
};
```

- [ ] **Step 2: Add account endpoints to mock-api.js**

Find the request routing section and add handlers for the three new endpoints so Playwright tests don't get 404s on them:

```js
// In the request handler switch/if-else block, add:
if (method === 'PATCH' && url === '/account/profile') {
  res.writeHead(204); res.end(); return;
}
if (method === 'PATCH' && url === '/account/password') {
  res.writeHead(204); res.end(); return;
}
if (method === 'DELETE' && url === '/account') {
  res.writeHead(204); res.end(); return;
}
```

- [ ] **Step 3: Update auth.test.ts**

Any tests that assert on `user.username` should change to `user.display_name`. Any login form submissions using a `username` field should use `email`.

- [ ] **Step 4: Run all frontend tests**

```bash
cd web && npm run test:unit && npm run test:e2e
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/tests/mock-api.js web/tests/auth.test.ts
git commit -m "test: update mock API and e2e tests for email login and display_name"
```

---

## Task 14: Full test suite

- [ ] **Step 1: Run the full test suite**

```bash
bash test.sh
```

Expected: all sections pass (backend, frontend unit, frontend e2e).

- [ ] **Step 2: Commit if any last fixes were needed**

```bash
git add -p
git commit -m "fix: address any remaining test failures after account page integration"
```
