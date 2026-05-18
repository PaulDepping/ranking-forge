# Collaboration & Publishing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add role-based project membership (Owner/Editor/Viewer), invite links, username-based invites, and public project publishing to RankingForge.

**Architecture:** Replace the single `ranking_projects.user_id` ownership column with a `project_members` join table. A new `OptionalAuthUser` extractor enables read endpoints to serve both authenticated members and unauthenticated visitors of published projects. New `/projects/:id/members` and `/projects/:id/invite-links` route groups handle collaboration management. A top-level `/invite/:token/accept` route handles invite redemption.

**Tech Stack:** Rust/Axum (backend), sqlx with `#[sqlx::test]` for integration tests, SvelteKit/TypeScript (frontend), shadcn-svelte components.

---

## File Map

### Backend — modified
- `backend/migrations/001_initial.sql` — rewrite: add `project_member_role` enum, `project_members`, `project_invite_links`; remove `user_id` from `ranking_projects`; add `published`
- `backend/crates/common/src/models/mod.rs` — remove `user_id` from `Project`, add `published`; add `ProjectMemberRole`, `ProjectMember`, `ProjectInviteLink`
- `backend/crates/api/src/error.rs` — add `Forbidden` variant
- `backend/crates/api/src/routes/auth.rs` — add `OptionalAuthUser` extractor
- `backend/crates/api/src/routes/projects.rs` — replace `require_project` with `require_project_access` + `require_project_read_access`; update all project handlers; add `user_role` + `published` to `ProjectResponse`
- `backend/crates/api/src/routes/tournaments.rs` — switch read handlers to `OptionalAuthUser` + `require_project_read_access`; switch `patch_event` to editor role check
- `backend/crates/api/src/routes/players.rs` — switch all write handlers to editor role check
- `backend/crates/api/src/routes/import.rs` — `start_import` → editor; `get_import_status` → viewer
- `backend/crates/api/src/routes/mod.rs` — register `members`, `invite_links`, and `/invite/:token/accept`

### Backend — created
- `backend/crates/api/src/routes/members.rs` — list, add-by-username, change-role, remove, transfer-ownership
- `backend/crates/api/src/routes/invite_links.rs` — create, list, revoke, accept

### Frontend — modified
- `web/src/lib/types.ts` — add `ProjectMemberRole`, `ProjectMember`, `InviteLink`; update `Project`
- `web/src/hooks.server.ts` — exempt `/projects/:id` subtree from auth redirect
- `web/src/routes/projects/[id]/+layout.server.ts` — handle 404 with SvelteKit `error()`; return `project` including `user_role`
- `web/src/routes/projects/[id]/+layout.svelte` — hide Settings nav link for non-owners
- `web/src/routes/projects/[id]/settings/+page.server.ts` — add `publish` action; guard `rename`/`delete` to owner; add member/invite-link actions
- `web/src/routes/projects/[id]/settings/+page.svelte` — add publish toggle, Members section, Invite Links section
- `web/tests/mock-api.js` — add `published`/`user_role` to mock data; add member + invite-link routes

### Frontend — created
- `web/src/routes/projects/[id]/+error.svelte` — auth-aware 404 messaging
- `web/src/routes/invite/[token]/+page.server.ts` — fetch link preview + accept action
- `web/src/routes/invite/[token]/+page.svelte` — invite accept UI

---

## Task 1: Rewrite Schema

**Files:**
- Modify: `backend/migrations/001_initial.sql`

- [ ] **Step 1: Open the migration and make the following changes**

Replace the `ranking_projects` table definition. Remove the `user_id` column and add `published`:

```sql
CREATE TABLE ranking_projects (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name        TEXT        NOT NULL,
    game_id     BIGINT,
    game_name   TEXT,
    published   BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Remove the `ranking_projects_user_id_idx` index.

Add the new enum and tables after the `sessions` table and before `ranking_projects`:

```sql
-- Project membership roles
CREATE TYPE project_member_role AS ENUM ('owner', 'editor', 'viewer');
```

After `ranking_projects`, add:

```sql
-- Project membership (replaces ranking_projects.user_id)
CREATE TABLE project_members (
    project_id  UUID                NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    user_id     UUID                NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL,
    joined_at   TIMESTAMPTZ         NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, user_id)
);

CREATE INDEX project_members_user_id_idx ON project_members(user_id);

-- Invite links (role-baked, revokable, optionally expiring)
CREATE TABLE project_invite_links (
    id          UUID                PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID                NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL CHECK (role IN ('editor', 'viewer')),
    created_by  UUID                NOT NULL REFERENCES users(id),
    expires_at  TIMESTAMPTZ,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ         NOT NULL DEFAULT NOW()
);

CREATE INDEX project_invite_links_project_id_idx ON project_invite_links(project_id);
```

- [ ] **Step 2: Verify the migration compiles by attempting an sqlx prepare (expect it to fail at compile — that's OK at this stage, it just confirms the SQL parses)**

```bash
cd backend && cargo check 2>&1 | head -30
```

Expected: compile errors about missing `user_id` on `Project` struct — confirms the schema change is visible to sqlx.

- [ ] **Step 3: Commit**

```bash
git add backend/migrations/001_initial.sql
git commit -m "feat: rewrite schema for project membership and publishing"
```

---

## Task 2: Update Common Models

**Files:**
- Modify: `backend/crates/common/src/models/mod.rs`

- [ ] **Step 1: Replace the `Project` struct and add new types**

Remove `user_id: Uuid` from `Project`, add `published: bool`. Add `ProjectMemberRole`, `ProjectMember`, `ProjectInviteLink`:

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
    pub username: String,
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
    pub name: String,
    pub game_id: Option<i64>,
    pub game_name: Option<String>,
    pub published: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "project_member_role", rename_all = "snake_case")]
#[serde(rename_all = "lowercase")]
pub enum ProjectMemberRole {
    Owner,
    Editor,
    Viewer,
}

impl ProjectMemberRole {
    pub fn satisfies(&self, min: &ProjectMemberRole) -> bool {
        match (self, min) {
            (_, ProjectMemberRole::Viewer) => true,
            (ProjectMemberRole::Owner | ProjectMemberRole::Editor, ProjectMemberRole::Editor) => true,
            (ProjectMemberRole::Owner, ProjectMemberRole::Owner) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectMember {
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub role: ProjectMemberRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectInviteLink {
    pub id: Uuid,
    pub project_id: Uuid,
    pub role: ProjectMemberRole,
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

- [ ] **Step 2: Verify common compiles**

```bash
cd backend && cargo check -p common 2>&1 | head -20
```

Expected: no errors in the `common` crate (api crate will still have errors due to missing `user_id`).

- [ ] **Step 3: Commit**

```bash
git add backend/crates/common/src/models/mod.rs
git commit -m "feat: update Project model; add ProjectMemberRole, ProjectMember, ProjectInviteLink"
```

---

## Task 3: Add Forbidden Error + OptionalAuthUser Extractor

**Files:**
- Modify: `backend/crates/api/src/error.rs`
- Modify: `backend/crates/api/src/routes/auth.rs`

- [ ] **Step 1: Add `Forbidden` variant to `AppError`**

In `backend/crates/api/src/error.rs`, add `Forbidden` to the enum and its `IntoResponse` arm:

```rust
#[derive(Debug)]
pub enum AppError {
    NotFound,
    Unauthorized,
    Forbidden,
    UnprocessableEntity(String),
    Db(sqlx::Error),
    PasswordHash,
    ExternalApi(reqwest::Error),
    ExternalApiError,
}
```

In the `IntoResponse` impl, add before `UnprocessableEntity`:

```rust
AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden".into()),
```

- [ ] **Step 2: Add `OptionalAuthUser` extractor to `auth.rs`**

After the existing `AuthUser` impl, add:

```rust
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
                "SELECT u.id, u.username, u.password_hash, u.created_at
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
```

- [ ] **Step 3: Check compile**

```bash
cd backend && cargo check -p api 2>&1 | grep "^error" | head -20
```

Expected: errors about `user_id` field in `projects.rs` and other handlers — these are fixed in subsequent tasks.

- [ ] **Step 4: Commit**

```bash
git add backend/crates/api/src/error.rs backend/crates/api/src/routes/auth.rs
git commit -m "feat: add Forbidden error variant and OptionalAuthUser extractor"
```

---

## Task 4: Replace Access Helpers + Update Project CRUD (TDD)

**Files:**
- Modify: `backend/crates/api/src/routes/projects.rs`

- [ ] **Step 1: Write failing integration tests**

Add a `#[cfg(test)]` module at the bottom of `projects.rs`. These tests require `DATABASE_URL` — run with `bash backend/test.sh`.

```rust
#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, http::{Request, StatusCode}};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;

    use crate::{routes, state::AppState};
    use common::StartggClient;

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

        // Add editor1 as editor via SQL (member management tested in Task 7)
        let editor_id = sqlx::query_scalar!("SELECT id FROM users WHERE username = 'editor1'")
            .fetch_one(&pool).await.unwrap();
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        sqlx::query!(
            "INSERT INTO project_members (project_id, user_id, role) VALUES ($1, $2, 'editor')",
            proj_uuid, editor_id
        ).execute(&pool).await.unwrap();

        // Owner sees project with role=owner
        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri("/projects")
                .header("cookie", &owner_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body[0]["user_role"], "owner");

        // Editor sees project with role=editor
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

        // Publish the project
        app.clone().oneshot(
            Request::builder().method("PATCH").uri(&format!("/projects/{proj_id}"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(&json!({"published": true})).unwrap())).unwrap()
        ).await.unwrap();

        // Unauthenticated access succeeds
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

        // Editor cannot delete
        let resp = app.clone().oneshot(
            Request::builder().method("DELETE").uri(&format!("/projects/{proj_id}"))
                .header("cookie", &editor_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 403);

        // Owner can delete
        let resp = app.clone().oneshot(
            Request::builder().method("DELETE").uri(&format!("/projects/{proj_id}"))
                .header("cookie", &owner_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 204);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd backend && cargo test -p api -- projects::tests 2>&1 | tail -20
```

Expected: compilation errors because `require_project_access` doesn't exist yet.

- [ ] **Step 3: Rewrite `projects.rs` with new access helpers and updated handlers**

Replace the entire file content:

```rust
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
/// Returns 404 if not a member (avoids leaking existence), 403 if role is too low.
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
/// Returns 404 for private projects with no membership.
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

    // Not published — check membership
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
        require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Viewer).await?;

    if body.name.is_some() || body.published.is_some() {
        // Both rename and publish are owner-only
        if !role.satisfies(&ProjectMemberRole::Owner) {
            return Err(AppError::Forbidden);
        }
    }

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
    // ... (paste the tests from Step 1 here)
}
```

- [ ] **Step 4: Run the tests**

```bash
cd backend && cargo test -p api -- projects::tests 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/api/src/routes/projects.rs
git commit -m "feat: replace require_project with role-aware access helpers; update project CRUD"
```

---

## Task 5: Update Read Endpoints for Optional Auth (TDD)

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`
- Modify: `backend/crates/api/src/routes/import.rs`

- [ ] **Step 1: Write failing tests for public read access**

Add to the `#[cfg(test)]` block in `projects.rs` (the test helpers are already there):

```rust
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
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd backend && cargo test -p api -- projects::tests::test_unauthenticated_can_read_stats 2>&1 | tail -10
```

Expected: compilation error — `tournaments.rs` still uses `AuthUser`.

- [ ] **Step 3: Update `tournaments.rs` read handlers**

In `tournaments.rs`, change the imports to include `OptionalAuthUser` and `require_project_read_access`, and replace `require_project` calls in the read handlers:

Change the imports block to:
```rust
use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, OptionalAuthUser},
    routes::projects::{require_project_access, require_project_read_access},
    state::AppState,
};
use common::models::ProjectMemberRole;
```

Update `list_tournaments`, `get_stats`, `get_head_to_head`, `get_h2h_sets` signatures and first lines:

```rust
pub async fn list_tournaments(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;
    // ... rest unchanged
}

pub async fn get_stats(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;
    // ... rest unchanged
}

pub async fn get_head_to_head(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;
    // ... rest unchanged
}

pub async fn get_h2h_sets(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path((project_id, player_a_id, player_b_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;
    // ... rest unchanged
}
```

Update `patch_event` (write) to use `require_project_access` with editor:
```rust
pub async fn patch_event(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, event_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<PatchEventRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Editor).await?;
    // ... rest unchanged
}
```

- [ ] **Step 4: Update `import.rs`**

In `import.rs`, change imports and handlers:

```rust
use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, OptionalAuthUser},
    routes::projects::{require_project_access, require_project_read_access},
    state::AppState,
};
use common::{jobs::ImportParams, models::{Job, ProjectMemberRole}};
```

```rust
pub async fn start_import(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    body: Option<Json<ImportRequest>>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Editor).await?;
    // ... rest unchanged
}

pub async fn get_import_status(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;
    // ... rest unchanged
}
```

- [ ] **Step 5: Run the tests**

```bash
cd backend && cargo test -p api -- projects::tests 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/tournaments.rs backend/crates/api/src/routes/import.rs
git commit -m "feat: switch read endpoints to OptionalAuthUser; enforce editor role on writes"
```

---

## Task 6: Update Player Write Endpoints

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`

- [ ] **Step 1: Write a failing test for viewer access denial**

Add to `projects::tests`:

```rust
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
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd backend && cargo test -p api -- projects::tests::test_viewer_cannot_add_player 2>&1 | tail -10
```

Expected: test fails (currently returns 404 because require_project still uses user_id check).

- [ ] **Step 3: Update `players.rs` imports and all `require_project` calls**

Change the imports:
```rust
use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    routes::projects::require_project_access,
    state::AppState,
};
use common::models::{Player, ProjectMemberRole, StartggAccount};
```

Update the imports in `players.rs`:
```rust
use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, OptionalAuthUser},
    routes::projects::{require_project_access, require_project_read_access},
    state::AppState,
};
use common::models::{Player, ProjectMemberRole, StartggAccount};
```

`list_players` is called by the Ranking page, which must be publicly accessible for published projects. Change it to `OptionalAuthUser` + `require_project_read_access`:

```rust
async fn list_players(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_read_access(&state.db, id, user.map(|u| u.id)).await?;
    // ... rest unchanged
}
```

Replace every other `require_project(&state.db, ...)` call with:
```rust
require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Editor).await?;
```

This applies to: `add_player`, `delete_player`, `link_account`, `unlink_account`, `bulk_add_players`, `add_players_by_handles`, `list_tournament_entrants`, `rename_player`, `reorder_players`.

- [ ] **Step 4: Run the tests**

```bash
cd backend && cargo test -p api -- projects::tests::test_viewer_cannot_add_player 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/api/src/routes/players.rs
git commit -m "feat: enforce editor role on all player management endpoints"
```

---

## Task 7: Member Management Routes (TDD)

**Files:**
- Create: `backend/crates/api/src/routes/members.rs`

- [ ] **Step 1: Create `members.rs` with failing tests first**

```rust
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    routes::projects::require_project_access,
    state::AppState,
};
use common::models::{ProjectMember, ProjectMemberRole};

// ── Request types ─────────────────────────────────────────────────────────────

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

// ── Handlers ──────────────────────────────────────────────────────────────────

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

    // Don't allow changing the current owner's role via this endpoint
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

    // Target must already be a member
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

    // Promote target to owner
    sqlx::query!(
        "UPDATE project_members SET role = 'owner' WHERE project_id = $1 AND user_id = $2",
        project_id,
        body.user_id,
    )
    .execute(&mut *tx)
    .await?;

    // Demote current owner to editor
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

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_members).post(add_member))
        .route("/{uid}", patch(change_member_role).delete(remove_member))
        .route("/transfer-ownership", post(transfer_ownership))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::{Request, StatusCode}};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;
    use crate::{routes, state::AppState};
    use common::StartggClient;

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

        // Add member
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri(&format!("/projects/{proj_id}/members"))
                .header("content-type", "application/json")
                .header("cookie", &owner_cookie)
                .body(Body::from(serde_json::to_vec(
                    &json!({"username": "mem_user", "role": "editor"})
                ).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 204);

        // List members
        let resp = app.clone().oneshot(
            Request::builder().method("GET").uri(&format!("/projects/{proj_id}/members"))
                .header("cookie", &owner_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let members: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(members.as_array().unwrap().len(), 2); // owner + new editor
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

        // Transfer
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

        // Verify roles in DB
        let old_role = sqlx::query_scalar!(
            r#"SELECT role as "role: String" FROM project_members
               WHERE project_id = $1 AND user_id = (SELECT id FROM users WHERE username = 'old_owner')"#,
            proj_uuid
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(old_role, "editor");

        let new_role = sqlx::query_scalar!(
            r#"SELECT role as "role: String" FROM project_members
               WHERE project_id = $1 AND user_id = $2"#,
            proj_uuid, new_owner_id
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(new_role, "owner");
    }
}
```

- [ ] **Step 2: Run the tests (will fail — `members` module not registered yet)**

```bash
cd backend && cargo check -p api 2>&1 | grep "^error" | head -10
```

Expected: module not found error for `crate::routes::members`.

- [ ] **Step 3: Register the module in `routes/mod.rs`** (do this now to unblock compilation)

Add to `backend/crates/api/src/routes/mod.rs`:
```rust
pub mod members;
pub mod invite_links;
```

- [ ] **Step 4: Create a stub `invite_links.rs`** (needed to compile, full impl in Task 8)

```rust
use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(|| async { StatusCode::NOT_IMPLEMENTED }))
}
```

- [ ] **Step 5: Run the member tests**

```bash
cd backend && cargo test -p api -- members::tests 2>&1 | tail -20
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/members.rs backend/crates/api/src/routes/mod.rs backend/crates/api/src/routes/invite_links.rs
git commit -m "feat: add member management routes (list, add, change-role, remove, transfer-ownership)"
```

---

## Task 8: Invite Link Routes (TDD)

**Files:**
- Modify: `backend/crates/api/src/routes/invite_links.rs` (replace stub)
- Modify: `backend/crates/api/src/routes/mod.rs` (add top-level invite accept route)

- [ ] **Step 1: Replace the stub with the full implementation**

```rust
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
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
use common::models::{ProjectInviteLink, ProjectMemberRole};

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateInviteLinkRequest {
    role: ProjectMemberRole,
    expires_at: Option<DateTime<Utc>>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn list_invite_links(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;

    let links = sqlx::query_as!(
        ProjectInviteLink,
        r#"SELECT id, project_id, role as "role: ProjectMemberRole",
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
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;

    if body.role == ProjectMemberRole::Owner {
        return Err(AppError::UnprocessableEntity(
            "invite links cannot grant owner role".into(),
        ));
    }

    let link = sqlx::query_as!(
        ProjectInviteLink,
        r#"INSERT INTO project_invite_links (project_id, role, created_by, expires_at)
           VALUES ($1, $2, $3, $4)
           RETURNING id, project_id, role as "role: ProjectMemberRole",
                     created_by, expires_at, revoked_at, created_at"#,
        project_id,
        body.role as ProjectMemberRole,
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
    require_project_access(&state.db, project_id, user.id, ProjectMemberRole::Owner).await?;

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
        r#"SELECT id, project_id, role as "role: ProjectMemberRole",
                  created_by, expires_at, revoked_at, created_at
           FROM project_invite_links
           WHERE id = $1"#,
        token,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    if link.revoked_at.is_some() {
        return Err(AppError::UnprocessableEntity("invite link has been revoked".into()));
    }
    if let Some(exp) = link.expires_at {
        if exp < Utc::now() {
            return Err(AppError::UnprocessableEntity("invite link has expired".into()));
        }
    }

    // Don't change the owner's role
    let is_owner = sqlx::query_scalar!(
        r#"SELECT 1 AS one FROM project_members
           WHERE project_id = $1 AND user_id = $2 AND role = 'owner'"#,
        link.project_id,
        user.id,
    )
    .fetch_optional(&state.db)
    .await?;

    if is_owner.is_some() {
        return Ok(Json(AcceptResponse { project_id: link.project_id }));
    }

    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role)
           VALUES ($1, $2, $3)
           ON CONFLICT (project_id, user_id) DO UPDATE SET role = EXCLUDED.role"#,
        link.project_id,
        user.id,
        link.role as ProjectMemberRole,
    )
    .execute(&state.db)
    .await?;

    Ok(Json(AcceptResponse { project_id: link.project_id }))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_invite_links).post(create_invite_link))
        .route("/{link_id}", delete(revoke_invite_link))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::{Request, StatusCode}};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;
    use crate::{routes, state::AppState};
    use common::StartggClient;

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

    async fn create_project(app: &Router, cookie: &str) -> String {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/projects")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_vec(&json!({"name": "Test"})).unwrap())).unwrap()
        ).await.unwrap();
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
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri(&format!("/projects/{proj_id}/invite-links"))
                .header("content-type", "application/json")
                .header("cookie", &owner_cookie)
                .body(Body::from(serde_json::to_vec(&json!({"role": "editor"})).unwrap())).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 201);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let link: Value = serde_json::from_slice(&bytes).unwrap();
        let token = link["id"].as_str().unwrap().to_string();

        // Accept the invite
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri(&format!("/invite/{token}/accept"))
                .header("cookie", &user_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);

        // User is now a member
        let proj_uuid: uuid::Uuid = proj_id.parse().unwrap();
        let role = sqlx::query_scalar!(
            r#"SELECT role as "role: String" FROM project_members
               WHERE project_id = $1 AND user_id = (SELECT id FROM users WHERE username = 'inv_user')"#,
            proj_uuid
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(role, "editor");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_revoked_link_cannot_be_accepted(pool: PgPool) {
        let app = make_app(pool.clone());
        let owner_cookie = register(&app, "rev_owner").await;
        let user_cookie = register(&app, "rev_user").await;
        let proj_id = create_project(&app, &owner_cookie).await;

        // Create and revoke link
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri(&format!("/projects/{proj_id}/invite-links"))
                .header("content-type", "application/json")
                .header("cookie", &owner_cookie)
                .body(Body::from(serde_json::to_vec(&json!({"role": "viewer"})).unwrap())).unwrap()
        ).await.unwrap();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let link: Value = serde_json::from_slice(&bytes).unwrap();
        let token = link["id"].as_str().unwrap().to_string();
        let link_id = token.clone();

        app.clone().oneshot(
            Request::builder().method("DELETE")
                .uri(&format!("/projects/{proj_id}/invite-links/{link_id}"))
                .header("cookie", &owner_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();

        // Accept fails
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri(&format!("/invite/{token}/accept"))
                .header("cookie", &user_cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 422);
    }
}
```

- [ ] **Step 2: Register the invite accept route in `routes/mod.rs`**

```rust
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
        .nest("/projects", projects::router())
        .route("/games", get(games::search_games))
        .route("/invite/{token}/accept", post(invite_links::accept_invite_link))
}
```

- [ ] **Step 3: Run invite link tests**

```bash
cd backend && cargo test -p api -- invite_links::tests 2>&1 | tail -20
```

Expected: all pass.

- [ ] **Step 4: Update the sqlx offline cache**

```bash
bash backend/prepare-sqlx.sh
```

Expected: exits 0; `.sqlx/` directory updated.

- [ ] **Step 5: Run the full backend test suite**

```bash
bash backend/test.sh
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/invite_links.rs backend/crates/api/src/routes/mod.rs backend/.sqlx/
git commit -m "feat: add invite link routes and top-level accept endpoint; update sqlx cache"
```

---

## Task 9: Frontend Types + Hooks

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/hooks.server.ts`

- [ ] **Step 1: Update `types.ts`**

Add to `types.ts`:

1. Update `Project` — add `published` and `user_role`:
```typescript
export interface Project {
  id: string;
  name: string;
  game_id: number | null;
  game_name: string | null;
  created_at: string;
  published: boolean;
  user_role: 'owner' | 'editor' | 'viewer' | null;
}
```

2. Add new types after the existing ones:
```typescript
export interface ProjectMember {
  project_id: string;
  user_id: string;
  username: string;
  role: 'owner' | 'editor' | 'viewer';
  joined_at: string;
}

export interface InviteLink {
  id: string;
  project_id: string;
  role: 'editor' | 'viewer';
  created_by: string;
  expires_at: string | null;
  revoked_at: string | null;
  created_at: string;
}

export interface AcceptInviteResponse {
  project_id: string;
}
```

- [ ] **Step 2: Update `hooks.server.ts`** to exempt `/projects/:id` subtree from redirect

```typescript
import type { Handle } from '@sveltejs/kit';
import { redirect } from '@sveltejs/kit';
import { INTERNAL_API_URL } from '$env/static/private';

export const handle: Handle = async ({ event, resolve }) => {
  const { pathname } = event.url;

  const sessionId = event.cookies.get('session_id');
  const res = await event.fetch(`${INTERNAL_API_URL}/auth/me`, {
    headers: sessionId ? { Cookie: `session_id=${sessionId}` } : {}
  });
  if (res.ok) {
    event.locals.user = await res.json();
  } else {
    event.locals.user = null;
    const isPublic =
      ['/login', '/register'].includes(pathname) ||
      /^\/projects\/[^/]/.test(pathname) ||
      /^\/invite\//.test(pathname);
    if (!isPublic) {
      redirect(303, '/login');
    }
  }

  return resolve(event);
};
```

- [ ] **Step 3: Run frontend unit tests to verify no regressions**

```bash
cd web && npm run test:unit
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/types.ts web/src/hooks.server.ts
git commit -m "feat: add ProjectMember and InviteLink types; exempt project/invite routes from auth redirect"
```

---

## Task 10: Update Project Layout for Role Awareness

**Files:**
- Modify: `web/src/routes/projects/[id]/+layout.server.ts`
- Modify: `web/src/routes/projects/[id]/+layout.svelte`
- Create: `web/src/routes/projects/[id]/+error.svelte`

- [ ] **Step 1: Update `+layout.server.ts`** to handle 404 with auth-aware error

```typescript
import { error } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Project } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: LayoutServerLoad = async ({ fetch, params, cookies, locals }) => {
  const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
  const res = await api.get(`/projects/${params.id}`);
  if (!res.ok) {
    if (res.status === 404) {
      if (!locals.user) {
        error(404, { message: 'private_project' });
      }
      error(404, { message: 'not_found' });
    }
    error(res.status, { message: 'error' });
  }
  const project: Project = await res.json();
  return { project };
};
```

- [ ] **Step 2: Create `+error.svelte`** for the project layout error boundary

```svelte
<script lang="ts">
  import { page } from '$app/state';
  import { Button } from '$lib/components/ui/button';
</script>

{#if page.error?.message === 'private_project'}
  <div class="flex flex-col items-center justify-center min-h-[60vh] gap-4 text-center">
    <h1 class="text-2xl font-bold">This project is private</h1>
    <p class="text-muted-foreground">You need to be a member to view this project.</p>
    <Button href="/register">Create an account</Button>
  </div>
{:else}
  <div class="flex flex-col items-center justify-center min-h-[60vh] gap-4 text-center">
    <h1 class="text-2xl font-bold">Project not found</h1>
    <p class="text-muted-foreground">This project does not exist or has been deleted.</p>
    <Button href="/projects">Back to projects</Button>
  </div>
{/if}
```

- [ ] **Step 3: Update `+layout.svelte`** to filter tabs by role

The current layout builds a static `tabs` array and renders them all. Replace with role-aware filtering. The full updated script block and tabs section:

```svelte
<script lang="ts">
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { Separator } from '$lib/components/ui/separator';
  import * as Tabs from '$lib/components/ui/tabs';

  let { children, data } = $props();

  const allTabs = [
    { label: 'Players', href: 'players', minRole: 'editor' as const },
    { label: 'Import', href: 'import', minRole: 'editor' as const },
    { label: 'Tournaments', href: 'tournaments', minRole: null },
    { label: 'Stats', href: 'stats', minRole: null },
    { label: 'H2H', href: 'h2h', minRole: null },
    { label: 'Ranking', href: 'ranking', minRole: null },
    { label: 'Settings', href: 'settings', minRole: 'owner' as const },
  ];

  const role = data.project.user_role;

  const tabs = $derived(allTabs.filter(t => {
    if (t.minRole === null) return true;
    if (t.minRole === 'editor') return role === 'editor' || role === 'owner';
    if (t.minRole === 'owner') return role === 'owner';
    return false;
  }));

  function tabHref(slug: string) {
    return `/projects/${data.project.id}/${slug}`;
  }

  const currentTab = $derived(
    tabs.find(t => page.url.pathname.startsWith(tabHref(t.href)))?.href ?? tabs[0].href
  );
</script>
```

The template section below the script remains the same — `{#each tabs as tab (tab.href)}` now iterates the filtered list.

- [ ] **Step 4: Run frontend unit tests**

```bash
cd web && npm run test:unit
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/[id]/+layout.server.ts \
        web/src/routes/projects/[id]/+layout.svelte \
        web/src/routes/projects/[id]/+error.svelte
git commit -m "feat: handle project 404 with auth-aware messaging; hide Settings for non-owners"
```

---

## Task 11: Settings Page — Publish, Members, Invite Links

**Files:**
- Modify: `web/src/routes/projects/[id]/settings/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/settings/+page.svelte`

- [ ] **Step 1: Update `+page.server.ts`** to add publish, member, and invite link actions

Replace the file with:

```typescript
import { fail, redirect, error } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { ProjectMember, InviteLink } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, params, cookies, parent }) => {
  const { project } = await parent();
  if (project.user_role !== 'owner') {
    error(403, { message: 'forbidden' });
  }

  const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
  const [membersRes, linksRes] = await Promise.all([
    api.get(`/projects/${params.id}/members`),
    api.get(`/projects/${params.id}/invite-links`),
  ]);

  const members: ProjectMember[] = membersRes.ok ? await membersRes.json() : [];
  const inviteLinks: InviteLink[] = linksRes.ok ? await linksRes.json() : [];

  return { members, inviteLinks };
};

export const actions: Actions = {
  rename: async ({ fetch, params, cookies, request }) => {
    const data = await request.formData();
    const name = (data.get('name') as string ?? '').trim();
    if (!name) return fail(400, { renameError: 'Name is required' });
    if ([...name].length > 100) return fail(400, { renameError: 'Name must be at most 100 characters' });
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.patch(`/projects/${params.id}`, { name });
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Rename failed' }));
      return fail(res.status, { renameError: body.message });
    }
    return { project: await res.json() };
  },

  publish: async ({ fetch, params, cookies, request }) => {
    const data = await request.formData();
    const published = data.get('published') === 'true';
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.patch(`/projects/${params.id}`, { published });
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Failed to update' }));
      return fail(res.status, { publishError: body.message });
    }
    return { project: await res.json() };
  },

  addMember: async ({ fetch, params, cookies, request }) => {
    const data = await request.formData();
    const username = (data.get('username') as string ?? '').trim();
    const role = data.get('role') as string;
    if (!username) return fail(400, { memberError: 'Username is required' });
    if (!['editor', 'viewer'].includes(role)) return fail(400, { memberError: 'Invalid role' });
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.post(`/projects/${params.id}/members`, { username, role });
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Failed to add member' }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  removeMember: async ({ fetch, params, cookies, request }) => {
    const data = await request.formData();
    const userId = data.get('user_id') as string;
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.delete(`/projects/${params.id}/members/${userId}`);
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Failed to remove member' }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  changeMemberRole: async ({ fetch, params, cookies, request }) => {
    const data = await request.formData();
    const userId = data.get('user_id') as string;
    const role = data.get('role') as string;
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.patch(`/projects/${params.id}/members/${userId}`, { role });
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Failed to update role' }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  transferOwnership: async ({ fetch, params, cookies, request }) => {
    const data = await request.formData();
    const userId = data.get('user_id') as string;
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.post(`/projects/${params.id}/members/transfer-ownership`, { user_id: userId });
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Transfer failed' }));
      return fail(res.status, { memberError: body.message });
    }
    return {};
  },

  createInviteLink: async ({ fetch, params, cookies, request }) => {
    const data = await request.formData();
    const role = data.get('role') as string;
    const expiresAtRaw = data.get('expires_at') as string | null;
    const expires_at = expiresAtRaw ? new Date(expiresAtRaw).toISOString() : undefined;
    if (!['editor', 'viewer'].includes(role)) return fail(400, { linkError: 'Invalid role' });
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.post(`/projects/${params.id}/invite-links`, { role, expires_at });
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Failed to create link' }));
      return fail(res.status, { linkError: body.message });
    }
    return { newLink: await res.json() };
  },

  revokeInviteLink: async ({ fetch, params, cookies, request }) => {
    const data = await request.formData();
    const linkId = data.get('link_id') as string;
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.delete(`/projects/${params.id}/invite-links/${linkId}`);
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Failed to revoke link' }));
      return fail(res.status, { linkError: body.message });
    }
    return {};
  },

  delete: async ({ fetch, params, cookies }) => {
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.delete(`/projects/${params.id}`);
    if (!res.ok) {
      const body = await res.json().catch(() => ({ message: 'Delete failed' }));
      return fail(res.status, { deleteError: body.message });
    }
    redirect(303, '/projects');
  }
};
```

- [ ] **Step 2: Update `+page.svelte`** to add the new sections

Add below the existing "Project name" section and above the "Danger zone" section:

```svelte
<Separator />

<!-- Publish section -->
<div class="space-y-3">
  <h2 class="text-lg font-semibold">Publish</h2>
  <p class="text-sm text-muted-foreground">
    {#if data.project.published}
      This project is publicly visible. Anyone with the link can view stats, H2H, and rankings.
    {:else}
      This project is private. Only members can view it.
    {/if}
  </p>
  <form method="POST" action="?/publish" use:enhance>
    <input type="hidden" name="published" value={data.project.published ? 'false' : 'true'} />
    <Button type="submit" variant={data.project.published ? 'outline' : 'default'}>
      {data.project.published ? 'Unpublish' : 'Publish project'}
    </Button>
  </form>
</div>

<Separator />

<!-- Members section -->
<div class="space-y-4">
  <h2 class="text-lg font-semibold">Members</h2>

  <Table>
    <TableHeader>
      <TableRow>
        <TableHead>Username</TableHead>
        <TableHead>Role</TableHead>
        <TableHead></TableHead>
      </TableRow>
    </TableHeader>
    <TableBody>
      {#each data.members as member}
        <TableRow>
          <TableCell>{member.username}</TableCell>
          <TableCell class="capitalize">{member.role}</TableCell>
          <TableCell class="text-right">
            {#if member.role !== 'owner'}
              <form method="POST" action="?/removeMember" use:enhance class="inline">
                <input type="hidden" name="user_id" value={member.user_id} />
                <Button type="submit" variant="ghost" size="sm">Remove</Button>
              </form>
            {/if}
          </TableCell>
        </TableRow>
      {/each}
    </TableBody>
  </Table>

  <form method="POST" action="?/addMember" use:enhance class="flex gap-2 items-end">
    <div class="flex-1 space-y-1">
      <Label for="member-username">Add by username</Label>
      <Input id="member-username" name="username" placeholder="username" />
    </div>
    <Select name="role">
      <SelectTrigger class="w-32">
        <SelectValue placeholder="Role" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="editor">Editor</SelectItem>
        <SelectItem value="viewer">Viewer</SelectItem>
      </SelectContent>
    </Select>
    <Button type="submit">Add</Button>
  </form>
  {#if form?.memberError}
    <p class="text-sm text-destructive">{form.memberError}</p>
  {/if}
</div>

<Separator />

<!-- Invite links section -->
<div class="space-y-4">
  <h2 class="text-lg font-semibold">Invite links</h2>

  {#each data.inviteLinks as link}
    <div class="flex items-center justify-between rounded-md border p-3 gap-2">
      <div class="text-sm space-y-0.5">
        <span class="font-medium capitalize">{link.role}</span>
        {#if link.expires_at}
          <span class="text-muted-foreground"> · expires {new Date(link.expires_at).toLocaleDateString()}</span>
        {/if}
      </div>
      <div class="flex gap-2">
        <Button type="button" variant="outline" size="sm"
          onclick={() => navigator.clipboard.writeText(`${location.origin}/invite/${link.id}`)}>
          Copy link
        </Button>
        <form method="POST" action="?/revokeInviteLink" use:enhance class="inline">
          <input type="hidden" name="link_id" value={link.id} />
          <Button type="submit" variant="ghost" size="sm">Revoke</Button>
        </form>
      </div>
    </div>
  {/each}

  <form method="POST" action="?/createInviteLink" use:enhance class="flex gap-2 items-end">
    <Select name="role">
      <SelectTrigger class="w-32">
        <SelectValue placeholder="Role" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="editor">Editor</SelectItem>
        <SelectItem value="viewer">Viewer</SelectItem>
      </SelectContent>
    </Select>
    <Button type="submit">Create invite link</Button>
  </form>
  {#if form?.linkError}
    <p class="text-sm text-destructive">{form.linkError}</p>
  {/if}
</div>
```

Add the required imports to the `<script>` block:
```typescript
import * as Table from '$lib/components/ui/table';
import * as Select from '$lib/components/ui/select';
```

Install the `select` shadcn component if not already present:
```bash
cd web && npx shadcn-svelte@latest add --yes --overwrite select
```

(Select is already in the installed list per CLAUDE.md — skip if already present.)

- [ ] **Step 3: Run unit tests**

```bash
cd web && npm run test:unit
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/projects/[id]/settings/
git commit -m "feat: add publish toggle, members management, and invite links to settings page"
```

---

## Task 12: Invite Accept Page + Mock API Updates

**Files:**
- Create: `web/src/routes/invite/[token]/+page.server.ts`
- Create: `web/src/routes/invite/[token]/+page.svelte`
- Modify: `web/tests/mock-api.js`

- [ ] **Step 1: Create `+page.server.ts`**

```typescript
import { redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import { INTERNAL_API_URL, PUBLIC_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, params, cookies }) => {
  // We can't preview the link contents without a dedicated endpoint,
  // so just return the token and let the page handle the accept action.
  return { token: params.token };
};

export const actions: Actions = {
  accept: async ({ fetch, params, cookies }) => {
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.post(`/invite/${params.token}/accept`);
    if (!res.ok) {
      if (res.status === 401) {
        redirect(303, `/login?next=/invite/${params.token}`);
      }
      const body = await res.json().catch(() => ({ message: 'Failed to accept invite' }));
      return { error: body.message };
    }
    const data = await res.json();
    redirect(303, `/projects/${data.project_id}`);
  }
};
```

- [ ] **Step 2: Create `+page.svelte`**

```svelte
<script lang="ts">
  import { enhance } from '$app/forms';
  import { Button } from '$lib/components/ui/button';
  import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '$lib/components/ui/card';

  let { data, form } = $props();
</script>

<div class="flex min-h-[60vh] items-center justify-center">
  <Card class="w-full max-w-sm">
    <CardHeader>
      <CardTitle>Project invitation</CardTitle>
      <CardDescription>You've been invited to collaborate on a project.</CardDescription>
    </CardHeader>
    <CardContent class="space-y-4">
      {#if form?.error}
        <p class="text-sm text-destructive">{form.error}</p>
      {/if}
      <form method="POST" action="?/accept" use:enhance>
        <Button type="submit" class="w-full">Accept invitation</Button>
      </form>
      <p class="text-center text-sm text-muted-foreground">
        Don't have an account? <a href="/register" class="underline">Create one</a>
      </p>
    </CardContent>
  </Card>
</div>
```

- [ ] **Step 3: Update `web/tests/mock-api.js`**

Add `published` and `user_role` to `MOCK_PROJECTS`:

```javascript
const MOCK_PROJECTS = [
  {
    id: 'proj-1',
    name: 'SSBM Power Ranking',
    game_id: 1,
    game_name: 'Super Smash Bros. Melee',
    created_at: '2026-01-01T00:00:00Z',
    published: false,
    user_role: 'owner'
  }
];
```

Add mock member and invite link data constants after the existing constants:

```javascript
const MOCK_MEMBERS = [
  { project_id: 'proj-1', user_id: 'user-1', username: 'testuser', role: 'owner', joined_at: '2026-01-01T00:00:00Z' }
];

const MOCK_INVITE_LINKS = [];
```

Add route handlers before the final `respond(res, 404, ...)` line:

```javascript
const membersMatch = path.match(/^\/projects\/([^/]+)\/members$/);
if (membersMatch) {
  if (req.method === 'GET') { respond(res, 200, MOCK_MEMBERS); return; }
  if (req.method === 'POST') { respond(res, 204, null); return; }
}

const memberMatch = path.match(/^\/projects\/([^/]+)\/members\/([^/]+)$/);
if (memberMatch) {
  if (req.method === 'PATCH') { respond(res, 204, null); return; }
  if (req.method === 'DELETE') { respond(res, 204, null); return; }
}

const transferMatch = path.match(/^\/projects\/([^/]+)\/members\/transfer-ownership$/);
if (transferMatch && req.method === 'POST') {
  respond(res, 204, null);
  return;
}

const inviteLinksMatch = path.match(/^\/projects\/([^/]+)\/invite-links$/);
if (inviteLinksMatch) {
  if (req.method === 'GET') { respond(res, 200, MOCK_INVITE_LINKS); return; }
  if (req.method === 'POST') {
    const body = await readBody(req);
    respond(res, 201, { id: 'link-new', project_id: inviteLinksMatch[1], role: body?.role ?? 'editor', created_by: 'user-1', expires_at: body?.expires_at ?? null, revoked_at: null, created_at: new Date().toISOString() });
    return;
  }
}

const inviteLinkMatch = path.match(/^\/projects\/([^/]+)\/invite-links\/([^/]+)$/);
if (inviteLinkMatch && req.method === 'DELETE') {
  respond(res, 204, null);
  return;
}

const inviteAcceptMatch = path.match(/^\/invite\/([^/]+)\/accept$/);
if (inviteAcceptMatch && req.method === 'POST') {
  respond(res, 200, { project_id: 'proj-1' });
  return;
}
```

- [ ] **Step 4: Run the full frontend test suite**

```bash
cd web && npm run test:unit && npm run test:e2e
```

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/invite/ web/tests/mock-api.js
git commit -m "feat: add invite accept page; update mock API with membership and invite link routes"
```

---

## Task 13: Final Integration Check

- [ ] **Step 1: Run the complete test suite**

```bash
bash test.sh
```

Expected: PASS — all backend and frontend tests green.

- [ ] **Step 2: If any test fails, fix it before moving on**

Common issues to check:
- sqlx query type mismatches (run `bash backend/prepare-sqlx.sh` if new queries were added)
- Frontend TypeScript errors from the updated `Project` type (components that destructure `project` may need `published` and `user_role` handled)

- [ ] **Step 3: Final commit**

```bash
git add -p  # stage any remaining changes
git commit -m "chore: final integration fixes for collaboration feature"
```
