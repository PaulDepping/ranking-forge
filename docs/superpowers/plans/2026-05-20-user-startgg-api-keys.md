# User-Provided start.gg API Keys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single server-wide `STARTGG_API_KEY` with per-user API keys stored on the `users` table, used for both games search and tournament imports.

**Architecture:** The project owner's `startgg_api_key` (stored in `users`) is used for all imports on their projects. The API creates per-request `StartggClient` instances instead of sharing one. `AppState` gains `startgg_base_url: String` (configurable for tests) and loses `startgg: StartggClient`. The worker looks up the owner's key at job-claim time.

**Tech Stack:** Rust/Axum (backend), sqlx (DB), wiremock (test mocking), SvelteKit/TypeScript (frontend)

---

## File Map

**Modified:**
- `backend/migrations/001_initial.sql` — add `startgg_api_key TEXT` to `users`
- `backend/crates/common/src/models/mod.rs` — add `startgg_api_key: Option<String>` to `User`
- `backend/crates/api/src/config.rs` — remove `startgg_api_key` field
- `backend/crates/api/src/state.rs` — remove `startgg`, add `startgg_base_url: String`
- `backend/crates/api/src/lib.rs` — remove `StartggClient` re-export
- `backend/crates/api/src/main.rs` — remove `StartggClient` construction, set `startgg_base_url`
- `backend/crates/api/src/routes/auth.rs` — add `has_startgg_key` to `UserResponse`, update all `User` queries
- `backend/crates/api/src/routes/account.rs` — add `set_startgg_key` and `delete_startgg_key` handlers
- `backend/crates/api/src/routes/games.rs` — use `AuthUser`'s key, return 422 if absent
- `backend/crates/api/src/routes/import.rs` — check owner's key before enqueue
- `backend/crates/api/src/routes/projects.rs` — add `owner_has_startgg_key: bool` to `ProjectResponse`
- `backend/crates/api/tests/api.rs` — update `make_app` helper
- `backend/crates/worker/src/config.rs` — remove `startgg_api_key`
- `backend/crates/worker/src/main.rs` — look up owner's key per job
- `web/src/app.d.ts` — add `has_startgg_key` to `Locals.user`
- `web/src/lib/types.ts` — add `has_startgg_key` to `User`, `owner_has_startgg_key` to `Project`
- `web/src/routes/account/+page.server.ts` — add `setStartggKey` and `removeStartggKey` actions
- `web/src/routes/account/+page.svelte` — add start.gg API Key card
- `web/src/routes/projects/[id]/(editor)/import/+page.svelte` — add no-key callout
- `backend/openapi.yaml` — document new endpoints and changed responses

---

## Task 1: DB migration + User model + all User queries

All `sqlx::query_as!(User, …)` calls fail to compile the moment `User` gains a new field. Update the schema, model, and every affected query in one commit.

**Files:**
- Modify: `backend/migrations/001_initial.sql`
- Modify: `backend/crates/common/src/models/mod.rs`
- Modify: `backend/crates/api/src/routes/auth.rs`

- [ ] **Step 1: Add `startgg_api_key` column to the users table in the migration**

In `backend/migrations/001_initial.sql`, update the `users` table definition:

```sql
CREATE TABLE users (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT        NOT NULL UNIQUE,
    display_name    TEXT        NOT NULL,
    password_hash   TEXT        NOT NULL,
    startgg_api_key TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 2: Add `startgg_api_key` to the `User` struct**

In `backend/crates/common/src/models/mod.rs`, update `User`:

```rust
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub password_hash: String,
    pub startgg_api_key: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 3: Add `has_startgg_key` to `UserResponse` in `auth.rs`**

In `backend/crates/api/src/routes/auth.rs`, update `UserResponse` and its `From<User>` impl:

```rust
#[derive(Serialize)]
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
```

- [ ] **Step 4: Update all `User` queries in `auth.rs` to select `startgg_api_key`**

Update the `register` handler's RETURNING clause:
```rust
let user = sqlx::query_as!(
    User,
    "INSERT INTO users (email, display_name, password_hash) VALUES ($1, $2, $3)
     RETURNING id, email, display_name, password_hash, startgg_api_key, created_at",
    body.email.to_lowercase(),
    body.display_name,
    password_hash,
)
```

Update the `login` handler's SELECT:
```rust
let user = sqlx::query_as!(
    User,
    "SELECT id, email, display_name, password_hash, startgg_api_key, created_at FROM users WHERE email = $1",
    body.email.to_lowercase(),
)
```

Update the `AuthUser` extractor's SELECT:
```rust
let user = sqlx::query_as!(
    User,
    "SELECT u.id, u.email, u.display_name, u.password_hash, u.startgg_api_key, u.created_at
     FROM sessions s
     JOIN users u ON u.id = s.user_id
     WHERE s.id = $1 AND s.expires_at > NOW()",
    session_id,
)
```

Update the `OptionalAuthUser` extractor's SELECT (same change, same query):
```rust
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
```

- [ ] **Step 5: Update TypeScript types**

In `web/src/app.d.ts`:
```typescript
interface Locals {
  user: {
    id: string;
    email: string;
    display_name: string;
    has_startgg_key: boolean;
    created_at: string;
  } | null;
}
```

In `web/src/lib/types.ts`, add `has_startgg_key` to the `User` interface:
```typescript
export interface User {
  id: string;
  email: string;
  display_name: string;
  has_startgg_key: boolean;
  created_at: string;
}
```

- [ ] **Step 6: Rebuild the sqlx offline query cache**

```bash
cd /home/pd/private_projects/ranking_forge/backend
bash prepare-sqlx.sh
```

Expected: exits 0, updates `.sqlx/` files.

- [ ] **Step 7: Verify compilation and tests pass**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p common
cargo test -p api
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add backend/migrations/001_initial.sql \
        backend/crates/common/src/models/mod.rs \
        backend/crates/api/src/routes/auth.rs \
        backend/.sqlx/ \
        web/src/app.d.ts \
        web/src/lib/types.ts
git commit -m "feat: add startgg_api_key to users and has_startgg_key to UserResponse"
```

---

## Task 2: Remove server-wide StartggClient; add `startgg_base_url` to AppState

Removing `startgg` from `AppState` and `games.rs` must happen atomically — they won't compile independently. Also update all test `make_app` helpers in the same commit.

**Files:**
- Modify: `backend/crates/api/src/state.rs`
- Modify: `backend/crates/api/src/config.rs`
- Modify: `backend/crates/api/src/lib.rs`
- Modify: `backend/crates/api/src/main.rs`
- Modify: `backend/crates/api/src/routes/games.rs`
- Modify: `backend/crates/api/src/routes/account.rs` (test helper)
- Modify: `backend/crates/api/src/routes/projects.rs` (test helper)
- Modify: `backend/crates/api/tests/api.rs`
- Modify: `backend/crates/worker/src/config.rs`

- [ ] **Step 1: Replace `AppState` fields**

Replace `backend/crates/api/src/state.rs` entirely:

```rust
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cors_origin: String,
    pub startgg_base_url: String,
}
```

- [ ] **Step 2: Remove `startgg_api_key` from API config**

Replace `backend/crates/api/src/config.rs` entirely:

```rust
use std::net::{IpAddr, Ipv4Addr};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "RankingForge HTTP API server")]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "PORT", default_value = "3000")]
    pub port: u16,

    #[arg(long, env = "BIND_ADDR", default_value_t = Ipv4Addr::new(0, 0, 0, 0).into())]
    pub bind_addr: IpAddr,

    /// Allowed CORS origin. Set to http://localhost:5173 for local dev.
    #[arg(long, env = "CORS_ORIGIN")]
    pub cors_origin: String,

    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub rust_log: String,
}
```

- [ ] **Step 3: Remove `StartggClient` re-export from `lib.rs`**

Replace `backend/crates/api/src/lib.rs` entirely:

```rust
pub mod config;
pub mod error;
pub mod routes;
pub mod state;
```

- [ ] **Step 4: Update `main.rs` to drop `StartggClient` construction**

In `backend/crates/api/src/main.rs`, remove the `use common::startgg::StartggClient;` import and update `AppState` construction:

```rust
let state = AppState {
    db: pool,
    cors_origin: config.cors_origin,
    startgg_base_url: "https://api.start.gg/gql/alpha".to_string(),
};
```

- [ ] **Step 5: Update `games.rs` to use the requesting user's key**

Replace `backend/crates/api/src/routes/games.rs` entirely:

```rust
use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    state::AppState,
};
use common::startgg::StartggClient;

#[derive(Deserialize)]
pub struct GamesQuery {
    pub q: String,
}

#[derive(Serialize)]
pub struct GameResponse {
    pub id: i64,
    pub name: String,
    pub display_name: Option<String>,
}

pub async fn search_games(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(params): Query<GamesQuery>,
) -> Result<impl IntoResponse> {
    if params.q.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("q must not be empty".into()));
    }
    let api_key = user.startgg_api_key.ok_or_else(|| {
        AppError::UnprocessableEntity(
            "Configure a start.gg API key in account settings before searching".into(),
        )
    })?;
    let client = StartggClient::new_with_base_url(api_key, state.startgg_base_url.clone());
    let games = client
        .search_games(&params.q)
        .await?
        .into_iter()
        .map(|g| GameResponse {
            id: g.id,
            name: g.name,
            display_name: g.display_name,
        })
        .collect::<Vec<_>>();
    Ok(Json(games))
}
```

- [ ] **Step 6: Update the `make_app` test helper in `account.rs`**

In `backend/crates/api/src/routes/account.rs`, find the test module and update `make_app` (remove `startgg` and add `startgg_base_url`):

```rust
fn make_app(pool: PgPool) -> Router {
    let state = AppState {
        db: pool,
        cors_origin: "http://localhost".into(),
        startgg_base_url: "http://localhost:1".into(),
    };
    routes::router().with_state(state)
}
```

Also remove the `use common::startgg::StartggClient;` import from the test module.

- [ ] **Step 7: Update the `make_app` test helper in `projects.rs`**

Same change in `backend/crates/api/src/routes/projects.rs` test module:

```rust
fn make_app(pool: PgPool) -> Router {
    let state = AppState {
        db: pool,
        cors_origin: "http://localhost".into(),
        startgg_base_url: "http://localhost:1".into(),
    };
    routes::router().with_state(state)
}
```

Remove the `use common::startgg::StartggClient;` import.

- [ ] **Step 8: Update `api/tests/api.rs`**

In `backend/crates/api/tests/api.rs`, remove `use api::StartggClient;` and update `make_app`:

```rust
use api::{routes, state::AppState};

fn make_app(pool: PgPool, startgg_base_url: &str) -> Router {
    let state = AppState {
        db: pool,
        cors_origin: "http://localhost".to_string(),
        startgg_base_url: if startgg_base_url.is_empty() {
            "https://api.start.gg/gql/alpha".to_string()
        } else {
            startgg_base_url.to_string()
        },
    };
    routes::router().with_state(state)
}
```

- [ ] **Step 9: Remove `STARTGG_API_KEY` from worker config**

Replace `backend/crates/worker/src/config.rs` entirely:

```rust
use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "RankingForge background import worker")]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub rust_log: String,
}
```

- [ ] **Step 10: Build entire workspace to verify compilation**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo build --workspace
```

Expected: compiles with no errors.

- [ ] **Step 11: Run tests**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p api
```

Expected: all tests pass (games tests will now require a user with a key — some may need updating, fix inline).

- [ ] **Step 12: Commit**

```bash
git add backend/crates/api/src/state.rs \
        backend/crates/api/src/config.rs \
        backend/crates/api/src/lib.rs \
        backend/crates/api/src/main.rs \
        backend/crates/api/src/routes/games.rs \
        backend/crates/api/src/routes/account.rs \
        backend/crates/api/src/routes/projects.rs \
        backend/crates/api/tests/api.rs \
        backend/crates/worker/src/config.rs
git commit -m "refactor: remove server-wide StartggClient; route API calls through user keys"
```

---

## Task 3: Account startgg-key endpoints (PUT + DELETE)

**Files:**
- Modify: `backend/crates/api/src/routes/account.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)]` block in `backend/crates/api/src/routes/account.rs`:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::method;

fn make_app_with_startgg(pool: PgPool, startgg_url: &str) -> Router {
    let state = AppState {
        db: pool,
        cors_origin: "http://localhost".into(),
        startgg_base_url: startgg_url.into(),
    };
    routes::router().with_state(state)
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_set_startgg_key_valid_stores_key(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"data": {"videogames": {"nodes": []}}}),
        ))
        .mount(&mock)
        .await;

    let app = make_app_with_startgg(pool.clone(), &mock.uri());
    let cookie = register(&app, "keyuser").await;

    let resp = app.clone().oneshot(
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

    let key =
        sqlx::query_scalar!("SELECT startgg_api_key FROM users WHERE email = 'keyuser@test.com'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(key, Some("my-valid-key".to_string()));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_set_startgg_key_invalid_returns_422(pool: PgPool) {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"data": null, "errors": [{"message": "not authorized"}]}),
        ))
        .mount(&mock)
        .await;

    let app = make_app_with_startgg(pool.clone(), &mock.uri());
    let cookie = register(&app, "badkeyuser").await;

    let resp = app.clone().oneshot(
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

    let resp = app.clone().oneshot(
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

    let resp = app.clone().oneshot(
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

    let resp = app.clone().oneshot(
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
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p api -- test_set_startgg_key test_delete_startgg_key test_me_reflects 2>&1 | tail -20
```

Expected: compilation error (handlers not defined yet).

- [ ] **Step 3: Add the handlers and update the router**

In `backend/crates/api/src/routes/account.rs`, add before the `pub fn router()` function:

```rust
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
    client.search_games("smash").await.map_err(|_| {
        AppError::UnprocessableEntity("Invalid start.gg API key".into())
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
```

Add `use common::startgg::StartggClient;` to the imports at the top of `account.rs`.

Update `pub fn router()` to add the new route:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/profile", patch(update_profile))
        .route("/password", patch(update_password))
        .route("/startgg-key", put(set_startgg_key).delete(delete_startgg_key))
        .route("/", delete(delete_account))
}
```

Add `put` to the axum routing imports:
```rust
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, patch, put},
};
```

- [ ] **Step 4: Rebuild sqlx cache (new UPDATE queries)**

```bash
cd /home/pd/private_projects/ranking_forge/backend
bash prepare-sqlx.sh
```

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p api -- test_set_startgg_key test_delete_startgg_key test_me_reflects
```

Expected: all 4 tests pass.

- [ ] **Step 6: Run full test suite**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p api
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/api/src/routes/account.rs backend/.sqlx/
git commit -m "feat: add PUT/DELETE /account/startgg-key endpoints"
```

---

## Task 4: Import enqueue guard — require owner's key

**Files:**
- Modify: `backend/crates/api/src/routes/import.rs`

- [ ] **Step 1: Write the failing test**

Add to the test module in `backend/crates/api/tests/api.rs` (or inline in `import.rs` tests if present):

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_start_import_returns_422_when_owner_has_no_key(pool: PgPool) {
    let app = make_app(&pool, "");
    let cookie = register(&app, "importowner", "password123").await;
    let proj_id = create_project(&app, &cookie, "My Project").await;

    let resp = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(&format!("/projects/{proj_id}/import"))
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(resp.status(), 422);
    let body = read_json(resp).await;
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("start.gg API key"),
        "expected message about API key, got: {}",
        body
    );
}
```

Note: `make_app(&pool, "")` — the existing `api/tests/api.rs` `make_app` takes `(PgPool, &str)`. If you added it as a method that takes `&PgPool`, adjust accordingly: `make_app(pool.clone(), "")`.

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p api -- test_start_import_returns_422_when_owner_has_no_key 2>&1 | tail -10
```

Expected: FAIL (currently returns 202 or compiles, no 422 check).

- [ ] **Step 3: Add the owner key guard to `start_import`**

In `backend/crates/api/src/routes/import.rs`, update `start_import`:

```rust
pub async fn start_import(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    body: Option<Json<ImportRequest>>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    let owner_key: Option<String> = sqlx::query_scalar!(
        "SELECT u.startgg_api_key FROM ranking_projects rp
         JOIN users u ON u.id = rp.owner_id
         WHERE rp.id = $1",
        project_id,
    )
    .fetch_optional(&state.db)
    .await?
    .flatten();

    if owner_key.is_none() {
        return Err(AppError::UnprocessableEntity(
            "Project owner has not configured a start.gg API key".into(),
        ));
    }

    let req = body.map(|b| b.0).unwrap_or_default();
    let params = ImportParams {
        after_date: req.after_date.map(date_to_timestamp),
        before_date: req.before_date.map(date_to_timestamp),
    };
    let job = common::jobs::enqueue(&state.db, project_id, params).await?;
    tracing::info!(user_id = %user.id, %project_id, job_id = %job.id, "import job enqueued");
    Ok((StatusCode::ACCEPTED, Json(JobResponse::from(job))))
}
```

- [ ] **Step 4: Rebuild sqlx cache**

```bash
cd /home/pd/private_projects/ranking_forge/backend
bash prepare-sqlx.sh
```

- [ ] **Step 5: Run the new test to verify it passes**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p api -- test_start_import_returns_422_when_owner_has_no_key
```

Expected: PASS.

- [ ] **Step 6: Run full test suite**

```bash
cargo test -p api
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/api/src/routes/import.rs \
        backend/crates/api/tests/api.rs \
        backend/.sqlx/
git commit -m "feat: require project owner's start.gg API key before enqueueing import"
```

---

## Task 5: Add `owner_has_startgg_key` to ProjectResponse

**Files:**
- Modify: `backend/crates/api/src/routes/projects.rs`
- Modify: `web/src/lib/types.ts`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `backend/crates/api/src/routes/projects.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_get_project_includes_owner_has_startgg_key(pool: PgPool) {
    let app = make_app(pool.clone());
    let cookie = register(&app, "keyowner").await;
    let proj_id = create_project(&app, &cookie, "Key Project").await;

    let resp = app.clone().oneshot(
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
    assert_eq!(body["owner_has_startgg_key"], false);

    sqlx::query!(
        "UPDATE users SET startgg_api_key = 'k' WHERE email = 'keyowner@test.com'"
    )
    .execute(&pool)
    .await
    .unwrap();

    let resp = app.clone().oneshot(
        Request::builder()
            .method("GET")
            .uri(&format!("/projects/{proj_id}"))
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["owner_has_startgg_key"], true);
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cargo test -p api -- test_get_project_includes_owner_has_startgg_key 2>&1 | tail -5
```

Expected: FAIL (field absent).

- [ ] **Step 3: Update `ProjectResponse` and handlers**

In `backend/crates/api/src/routes/projects.rs`:

Update `ProjectResponse`:
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
    pub owner_has_startgg_key: bool,
}

impl ProjectResponse {
    fn from_project(p: Project, user_role: Option<UserRole>, owner_has_startgg_key: bool) -> Self {
        ProjectResponse {
            id: p.id,
            name: p.name,
            game_id: p.game_id,
            game_name: p.game_name,
            published: p.published,
            created_at: p.created_at,
            user_role,
            owner_has_startgg_key,
        }
    }
}
```

Update `get_project` to look up the owner's key status with a follow-up query:
```rust
async fn get_project(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let (project, role) =
        require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;

    let owner_has_startgg_key: bool = sqlx::query_scalar!(
        "SELECT startgg_api_key IS NOT NULL FROM users WHERE id = $1",
        project.owner_id,
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(false);

    Ok(Json(ProjectResponse::from_project(project, role, owner_has_startgg_key)))
}
```

Update `list_projects` to add a subquery:
```rust
let rows = sqlx::query_as!(
    Row,
    r#"SELECT p.id, p.name, p.game_id, p.game_name, p.published, p.created_at,
              (p.owner_id = $1) AS is_owner,
              CASE WHEN pm.user_id IS NOT NULL THEN pm.role END AS "member_role: MemberRole",
              (SELECT startgg_api_key IS NOT NULL FROM users WHERE id = p.owner_id) AS "owner_has_startgg_key!"
       FROM ranking_projects p
       LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $1
       WHERE p.owner_id = $1 OR pm.user_id = $1
       ORDER BY p.created_at DESC"#,
    user.id,
)
.fetch_all(&state.db)
.await?;
```

Update the `Row` struct in `list_projects` to include the new field:
```rust
struct Row {
    id: Uuid,
    name: String,
    game_id: Option<i64>,
    game_name: Option<String>,
    published: bool,
    created_at: DateTime<Utc>,
    is_owner: Option<bool>,
    member_role: Option<MemberRole>,
    owner_has_startgg_key: bool,
}
```

Update the `list_projects` mapping to use it:
```rust
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
                owner_id: /* need owner_id - see note below */
                ...
            },
            Some(role),
            r.owner_has_startgg_key,
        )
    })
    .collect();
```

Wait — the `list_projects` Row struct doesn't currently include `owner_id`. The `Project` struct requires `owner_id`. Add `owner_id: Uuid` to the Row struct and select `p.owner_id` in the query:

```rust
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
    owner_has_startgg_key: bool,
}
```

Query:
```rust
r#"SELECT p.id, p.owner_id, p.name, p.game_id, p.game_name, p.published, p.created_at,
          (p.owner_id = $1) AS is_owner,
          CASE WHEN pm.user_id IS NOT NULL THEN pm.role END AS "member_role: MemberRole",
          (SELECT startgg_api_key IS NOT NULL FROM users WHERE id = p.owner_id) AS "owner_has_startgg_key!"
   FROM ranking_projects p
   LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $1
   WHERE p.owner_id = $1 OR pm.user_id = $1
   ORDER BY p.created_at DESC"#
```

Mapping:
```rust
ProjectResponse::from_project(
    Project {
        id: r.id,
        owner_id: r.owner_id,
        name: r.name,
        game_id: r.game_id,
        game_name: r.game_name,
        published: r.published,
        created_at: r.created_at,
    },
    Some(role),
    r.owner_has_startgg_key,
)
```

Update `create_project` (current user is owner):
```rust
Ok((
    StatusCode::CREATED,
    Json(ProjectResponse::from_project(
        project,
        Some(UserRole::Owner),
        user.startgg_api_key.is_some(),
    )),
))
```

Update `patch_project` (must be owner to patch):
```rust
Ok(Json(ProjectResponse::from_project(
    updated,
    Some(role),
    user.startgg_api_key.is_some(),
)))
```

- [ ] **Step 4: Rebuild sqlx cache**

```bash
cd /home/pd/private_projects/ranking_forge/backend
bash prepare-sqlx.sh
```

- [ ] **Step 5: Run new test**

```bash
cargo test -p api -- test_get_project_includes_owner_has_startgg_key
```

Expected: PASS.

- [ ] **Step 6: Run full test suite**

```bash
cargo test -p api
```

Expected: all tests pass.

- [ ] **Step 7: Update the TypeScript `Project` type**

In `web/src/lib/types.ts`, add `owner_has_startgg_key` to `Project`:

```typescript
export interface Project {
  id: string;
  name: string;
  game_id: number | null;
  game_name: string | null;
  created_at: string;
  published: boolean;
  user_role: "owner" | "editor" | "viewer" | null;
  owner_has_startgg_key: boolean;
}
```

- [ ] **Step 8: Commit**

```bash
git add backend/crates/api/src/routes/projects.rs \
        backend/.sqlx/ \
        web/src/lib/types.ts
git commit -m "feat: add owner_has_startgg_key to ProjectResponse"
```

---

## Task 6: Worker — per-job key lookup

**Files:**
- Modify: `backend/crates/worker/src/main.rs`

- [ ] **Step 1: Update `main.rs` to look up the owner's key at job-claim time**

In `backend/crates/worker/src/main.rs`:

Remove `use common::startgg::StartggClient;` from the top-level imports (it will be added back but used differently).

Remove the single `let startgg = common::startgg::StartggClient::new(config.startgg_api_key.into());` line.

Replace the inner `Ok(Some(job)) =>` arm with:

```rust
Ok(Some(job)) => {
    let pool2 = pool.clone();
    let project_id = job.project_id;
    let job_id = job.id;
    let import_params = common::jobs::ImportParams::from_job(&job);

    let api_key = match sqlx::query_scalar!(
        "SELECT u.startgg_api_key FROM ranking_projects rp
         JOIN users u ON u.id = rp.owner_id
         WHERE rp.id = $1",
        project_id,
    )
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(Some(key))) => key,
        Ok(_) => {
            tracing::error!(%job_id, %project_id, "project owner has no start.gg API key");
            let _ = common::jobs::mark_failed(
                &pool,
                job_id,
                "Project owner has no start.gg API key configured",
            )
            .await;
            continue;
        }
        Err(e) => {
            tracing::error!(%e, %job_id, "failed to look up owner API key");
            let _ = common::jobs::mark_failed(&pool, job_id, &e.to_string()).await;
            continue;
        }
    };

    let startgg = common::startgg::StartggClient::new(api_key);
    tracing::info!(%job_id, %project_id, "starting import");
    let handle = tokio::spawn(async move {
        match import::run(&pool2, &startgg, project_id, job_id, import_params).await {
            Ok(()) => {
                tracing::info!(%job_id, "import complete");
                if let Err(e) = common::jobs::mark_done(&pool2, job_id).await {
                    tracing::error!(%e, %job_id, "failed to mark job done");
                }
            }
            Err(e) => {
                tracing::error!(%e, %job_id, "import failed");
                if let Err(e2) =
                    common::jobs::mark_failed(&pool2, job_id, &e.to_string()).await
                {
                    tracing::error!(%e2, %job_id, "failed to mark job failed");
                }
            }
        }
    });
    in_flight.push((job_id, handle));
}
```

Also update `main()` to remove the `startgg_api_key` reference from the config usage:
```rust
// Remove this line:
// let startgg = common::startgg::StartggClient::new(config.startgg_api_key.into());
```

- [ ] **Step 2: Rebuild sqlx cache**

```bash
cd /home/pd/private_projects/ranking_forge/backend
bash prepare-sqlx.sh
```

- [ ] **Step 3: Build worker to verify**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo build -p worker
```

Expected: compiles with no errors.

- [ ] **Step 4: Run full backend test suite**

```bash
bash test.sh
```

Expected: all backend tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/worker/src/main.rs backend/.sqlx/
git commit -m "feat: worker looks up project owner's start.gg key per job"
```

---

## Task 7: Frontend — account settings API key card

**Files:**
- Modify: `web/src/routes/account/+page.server.ts`
- Modify: `web/src/routes/account/+page.svelte`

- [ ] **Step 1: Add `setStartggKey` and `removeStartggKey` form actions**

In `web/src/routes/account/+page.server.ts`, add two new actions inside the `actions` export:

```typescript
setStartggKey: async ({ fetch, request, locals }) => {
  if (!locals.user) return fail(401, { error: "Unauthorized" });
  const data = await request.formData();
  const api_key = data.get("api_key") as string | null;
  if (!api_key?.trim()) {
    return fail(422, { startggKeyError: "API key must not be empty." });
  }

  const res = await fetch(`${env.INTERNAL_API_URL}/account/startgg-key`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ api_key: api_key.trim() }),
  });

  if (!res.ok) {
    const json = await res.json().catch(() => ({ message: "Failed to save key" }));
    return fail(res.status, { startggKeyError: json.message ?? "Failed to save key" });
  }

  return { startggKeySuccess: true };
},

removeStartggKey: async ({ fetch, locals }) => {
  if (!locals.user) return fail(401, { error: "Unauthorized" });

  const res = await fetch(`${env.INTERNAL_API_URL}/account/startgg-key`, {
    method: "DELETE",
  });

  if (!res.ok) {
    return fail(res.status, { startggKeyError: "Failed to remove key." });
  }

  return { startggKeyRemoved: true };
},
```

- [ ] **Step 2: Add the API key card to `+page.svelte`**

In `web/src/routes/account/+page.svelte`, add the following card between the Password card and the Delete Account card:

```svelte
<!-- start.gg API Key card -->
<Card.Root>
  <Card.Header>
    <Card.Title>start.gg API Key</Card.Title>
    <Card.Description>
      Required to search for games and run tournament imports. Get your key at
      <a
        href="https://start.gg/admin/profile/developer"
        target="_blank"
        rel="noopener noreferrer"
        class="underline"
      >start.gg/admin/profile/developer</a>.
    </Card.Description>
  </Card.Header>

  {#if data.user.has_startgg_key}
    <Card.Content>
      {#if form?.startggKeyRemoved}
        <p class="text-sm text-green-600">API key removed.</p>
      {/if}
      <p class="text-sm text-muted-foreground">A start.gg API key is configured.</p>
    </Card.Content>
    <Card.Footer>
      <form method="POST" action="?/removeStartggKey" use:enhance>
        <Button type="submit" variant="outline">Remove key</Button>
      </form>
    </Card.Footer>
  {:else}
    <form method="POST" action="?/setStartggKey" use:enhance>
      <Card.Content class="space-y-4">
        {#if form?.startggKeyError}
          <p class="text-sm text-destructive">{form.startggKeyError}</p>
        {/if}
        {#if form?.startggKeySuccess}
          <p class="text-sm text-green-600">API key saved.</p>
        {/if}
        <div class="space-y-2">
          <Label for="api_key">API key</Label>
          <Input
            id="api_key"
            name="api_key"
            type="password"
            placeholder="Paste your start.gg API key"
            autocomplete="off"
          />
        </div>
      </Card.Content>
      <Card.Footer class="flex justify-end">
        <Button type="submit">Save key</Button>
      </Card.Footer>
    </form>
  {/if}
</Card.Root>
```

Note: `data.user.has_startgg_key` is available because `locals.user` is populated from `GET /auth/me` in `hooks.server.ts`, and `GET /auth/me` now returns `has_startgg_key`.

- [ ] **Step 3: Run frontend unit tests**

```bash
cd /home/pd/private_projects/ranking_forge/web
npm run test:unit
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/account/+page.server.ts web/src/routes/account/+page.svelte
git commit -m "feat: add start.gg API key management to account settings"
```

---

## Task 8: Frontend — import page no-key callout

**Files:**
- Modify: `web/src/routes/projects/[id]/(editor)/import/+page.svelte`

- [ ] **Step 1: Update the import page to show a callout when the owner has no key**

In `web/src/routes/projects/[id]/(editor)/import/+page.svelte`, replace the opening `<div class="space-y-6 max-w-lg">` block.

After the `<h2>` and `<p>` description, add a conditional that either shows the callout or the existing import controls:

```svelte
<div class="space-y-6 max-w-lg">
  <h2 class="text-lg font-semibold">Import tournaments</h2>
  <p class="text-sm text-muted-foreground">
    Fetches all start.gg tournaments for your players and imports them. This may
    take a minute.
  </p>

  {#if !data.project.owner_has_startgg_key}
    <Card.Root>
      <Card.Content class="p-4 space-y-2">
        {#if data.project.user_role === "owner"}
          <p class="text-sm font-medium">A start.gg API key is required to run imports.</p>
          <p class="text-sm text-muted-foreground">
            Add your key in
            <a href="/account" class="underline">account settings</a>, or get one at
            <a
              href="https://start.gg/admin/profile/developer"
              target="_blank"
              rel="noopener noreferrer"
              class="underline"
            >start.gg/admin/profile/developer</a>.
          </p>
        {:else}
          <p class="text-sm text-muted-foreground">
            The project owner needs to configure a start.gg API key before imports can run.
          </p>
        {/if}
      </Card.Content>
    </Card.Root>
  {:else}

    {#if form?.error}
      <Alert variant="destructive">{form.error}</Alert>
    {/if}

    <!-- existing job status card and import form go here, indented inside {:else} -->
    ... (move all existing content between here and </div> inside the {:else} block)

  {/if}
</div>
```

The full updated file should look like:

```svelte
<script lang="ts">
  import { untrack } from "svelte";
  import { enhance } from "$app/forms";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import { Alert } from "$lib/components/ui/alert";
  import * as Card from "$lib/components/ui/card";
  import DateRangePicker from "$lib/components/DateRangePicker.svelte";
  import type { DateRange } from "bits-ui";
  import { env } from "$env/dynamic/public";
  import { makeApi } from "$lib/api";
  import type { Job, ImportProgress } from "$lib/types";
  import { Progress } from "$lib/components/ui/progress";
  import { formatDateTime } from "$lib/utils";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";

  let { data, form } = $props();

  let importDialogOpen = $state(false);
  let importFormEl = $state<HTMLFormElement | null>(null);

  let dateRange = $state<DateRange | undefined>(undefined);
  const afterDateStr = $derived(dateRange?.start?.toString() ?? "");
  const beforeDateStr = $derived(dateRange?.end?.toString() ?? "");

  let job = $state<Job | null>(untrack(() => data.job ?? null));
  $effect(() => {
    job = data.job ?? null;
  });

  const isActiveJob = $derived(
    job?.status === "pending" || job?.status === "running",
  );

  const progressLabel = $derived.by((): string => {
    if (!job?.progress) return "";
    const { phase, step, total } = job.progress;
    return phase === "scanning"
      ? `Scanning players (${step} / ${total})`
      : `Importing tournaments (${step} / ${total})`;
  });

  const progressPercent = $derived.by((): number => {
    if (!job?.progress || job.progress.total === 0) return 0;
    return (job.progress.step / job.progress.total) * 100;
  });

  const statusColors: Record<
    string,
    "default" | "secondary" | "destructive" | "outline"
  > = {
    pending: "secondary",
    running: "default",
    done: "outline",
    failed: "destructive",
  };

  $effect(() => {
    if (!isActiveJob) return;
    const interval = setInterval(async () => {
      const api = makeApi(fetch, env.PUBLIC_API_URL);
      const res = await api.get(`/projects/${data.project.id}/import`);
      if (res.ok) {
        job = (await res.json()) as Job;
      }
    }, 1000);
    return () => clearInterval(interval);
  });
</script>

<div class="space-y-6 max-w-lg">
  <h2 class="text-lg font-semibold">Import tournaments</h2>
  <p class="text-sm text-muted-foreground">
    Fetches all start.gg tournaments for your players and imports them. This may
    take a minute.
  </p>

  {#if !data.project.owner_has_startgg_key}
    <Card.Root>
      <Card.Content class="p-4 space-y-2">
        {#if data.project.user_role === "owner"}
          <p class="text-sm font-medium">A start.gg API key is required to run imports.</p>
          <p class="text-sm text-muted-foreground">
            Add your key in
            <a href="/account" class="underline">account settings</a>, or get one at
            <a
              href="https://start.gg/admin/profile/developer"
              target="_blank"
              rel="noopener noreferrer"
              class="underline"
            >start.gg/admin/profile/developer</a>.
          </p>
        {:else}
          <p class="text-sm text-muted-foreground">
            The project owner needs to configure a start.gg API key before imports can run.
          </p>
        {/if}
      </Card.Content>
    </Card.Root>
  {:else}
    {#if form?.error}
      <Alert variant="destructive">{form.error}</Alert>
    {/if}

    {#if job}
      <Card.Root class="py-0">
        <Card.Content class="p-4 space-y-2">
          <div class="flex items-center gap-2">
            <span class="text-sm font-medium">Status:</span>
            <Badge variant={statusColors[job.status]}>{job.status}</Badge>
            {#if isActiveJob}
              <span class="text-xs text-muted-foreground animate-pulse"
                >updating…</span
              >
            {/if}
          </div>
          {#if job.status === "pending"}
            <p class="text-sm text-muted-foreground">Waiting to start…</p>
          {/if}
          {#if job.status === "running" && job.progress}
            <div class="space-y-1">
              <p class="text-sm text-muted-foreground">{progressLabel}</p>
              <Progress value={progressPercent} class="h-2" />
            </div>
          {/if}
          {#if job.error}
            <p class="text-sm text-destructive">{job.error}</p>
          {/if}
          <p class="text-xs text-muted-foreground">
            Started {formatDateTime(job.created_at)}
          </p>
          {#if job.status === "failed"}
            <form
              method="POST"
              use:enhance={() => {
                return ({ result }) => {
                  if (result.type === "success" && result.data?.job) {
                    job = result.data.job as Job;
                  }
                };
              }}
            >
              <input type="hidden" name="after_date" value={job.after_date ?? ""} />
              <input type="hidden" name="before_date" value={job.before_date ?? ""} />
              <Button type="submit" variant="outline" size="sm">Retry</Button>
            </form>
          {/if}
        </Card.Content>
      </Card.Root>
    {/if}

    <form
      method="POST"
      class="space-y-4"
      bind:this={importFormEl}
      use:enhance={() => {
        return ({ result }) => {
          if (result.type === "success" && result.data?.job) {
            job = result.data.job as Job;
          }
        };
      }}
    >
      <input type="hidden" name="after_date" value={afterDateStr} />
      <input type="hidden" name="before_date" value={beforeDateStr} />
      <DateRangePicker
        value={dateRange}
        onSelect={(r) => {
          dateRange = r;
        }}
        placeholder="All time"
      />
      <p class="text-xs text-muted-foreground">
        Leave blank to import all tournaments.
      </p>
      <Button
        type="button"
        onclick={() => {
          if (isActiveJob) {
            importDialogOpen = true;
          } else {
            importFormEl?.requestSubmit();
          }
        }}
      >
        {job ? "Re-import" : "Start import"}
      </Button>
    </form>
  {/if}
</div>

<AlertDialog.Root bind:open={importDialogOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Import already running</AlertDialog.Title>
      <AlertDialog.Description>
        An import is currently in progress. Start a new one anyway?
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        onclick={() => {
          importDialogOpen = false;
          importFormEl?.requestSubmit();
        }}
      >
        Start import
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
```

- [ ] **Step 2: Run frontend tests**

```bash
cd /home/pd/private_projects/ranking_forge/web
npm run test:unit
npm run test:e2e
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/projects/\[id\]/\(editor\)/import/+page.svelte
git commit -m "feat: show no-key callout on import page when owner has no start.gg API key"
```

---

## Task 9: Update openapi.yaml

**Files:**
- Modify: `backend/openapi.yaml`

- [ ] **Step 1: Add `PUT /account/startgg-key`**

Add after the existing `/account/profile` entry:

```yaml
  /account/startgg-key:
    put:
      tags: [account]
      summary: Save or replace the user's start.gg API key
      description: |
        Validates the key against start.gg before storing it.
        Returns 422 if the key is rejected by start.gg.
      security:
        - cookieAuth: []
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [api_key]
              properties:
                api_key:
                  type: string
                  description: Bearer token from https://start.gg/admin/profile/developer
      responses:
        "204":
          description: Key saved
        "401":
          description: Not authenticated
        "422":
          description: Key failed validation against start.gg
    delete:
      tags: [account]
      summary: Remove the user's start.gg API key
      security:
        - cookieAuth: []
      responses:
        "204":
          description: Key removed
        "401":
          description: Not authenticated
```

- [ ] **Step 2: Update `GET /auth/me` response to include `has_startgg_key`**

Find the response schema for `GET /auth/me` and add the field:

```yaml
has_startgg_key:
  type: boolean
  description: True if the user has a start.gg API key configured
```

- [ ] **Step 3: Update `GET /projects/{project_id}` response**

Add `owner_has_startgg_key: boolean` to the project response schema:

```yaml
owner_has_startgg_key:
  type: boolean
  description: True if the project owner has a start.gg API key configured
```

- [ ] **Step 4: Update `POST /projects/{project_id}/import` to document the 422**

Add or update the `422` response entry:

```yaml
"422":
  description: Project owner has not configured a start.gg API key
```

- [ ] **Step 5: Update `GET /games` to note that a start.gg API key is required**

Add to the description:

```yaml
description: |
  Proxies a game name search to start.gg.
  Requires the authenticated user to have a start.gg API key configured.
  Returns 422 if no key is set.
```

- [ ] **Step 6: Commit**

```bash
git add backend/openapi.yaml
git commit -m "docs: update openapi.yaml for user-provided start.gg API keys"
```

---

## Final Verification

- [ ] **Run the complete test suite**

```bash
cd /home/pd/private_projects/ranking_forge
bash test.sh
```

Expected: all backend and frontend tests pass.
