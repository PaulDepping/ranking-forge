# Multi-Ranking Projects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform projects from single-ranking containers into multi-ranking containers, where each ranking independently selects a subset of the project's shared player pool and event pool.

**Architecture:** `projects` (renamed from `ranking_projects`) owns the player pool, members, and game. `rankings` (new table) is a child of a project. `ranking_players` (new join table) scopes players to a ranking with per-ranking `rank_position` and `notes`. `ranking_events` (replaces `project_events`) gates event inclusion per ranking. All stat/H2H/tournament endpoints move from project scope to ranking scope.

**Tech Stack:** Rust (Axum, sqlx), PostgreSQL, SvelteKit + TypeScript, shadcn-svelte, Tailwind CSS v4.

---

## File Map

### Backend — created
- `backend/crates/api/src/routes/rankings.rs` — ranking CRUD, ranking player membership, access helpers

### Backend — modified
- `backend/migrations/001_initial.sql` — consolidated schema (replaces both existing migration files)
- `backend/crates/common/src/models/mod.rs` — add `Ranking`, `RankingPlayer`; remove `published` from `Project`; remove `rank_position` from `Player`
- `backend/crates/api/src/routes/projects.rs` — rename table refs, remove `published`, update `require_project_read_access`
- `backend/crates/api/src/routes/players.rs` — remove `rank_position` from queries; remove `reorder_players` (moves to rankings.rs)
- `backend/crates/api/src/routes/tournaments.rs` — scope all handlers to `ranking_id`; update all queries
- `backend/crates/api/src/routes/mod.rs` — wire rankings router
- `backend/crates/worker/src/import.rs` — rename table refs; insert `ranking_events` rows for all rankings
- `backend/crates/e2e/tests/full_flow.rs` — adapt for new schema

### Backend — deleted
- `backend/migrations/002_job_progress.sql` — folded into 001_initial.sql

### Frontend — created
- `web/src/routes/projects/[id]/rankings/[rid]/+layout.server.ts`
- `web/src/routes/projects/[id]/rankings/[rid]/+page.server.ts`
- `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.server.ts`
- `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.svelte`
- `web/src/routes/projects/[id]/rankings/[rid]/stats/+page.server.ts`
- `web/src/routes/projects/[id]/rankings/[rid]/stats/+page.svelte`
- `web/src/routes/projects/[id]/rankings/[rid]/h2h/+page.server.ts`
- `web/src/routes/projects/[id]/rankings/[rid]/h2h/+page.svelte`
- `web/src/routes/projects/[id]/rankings/[rid]/tournaments/+page.server.ts`
- `web/src/routes/projects/[id]/rankings/[rid]/tournaments/+page.svelte`
- `web/src/routes/projects/[id]/rankings/[rid]/(editor)/players/+page.server.ts`
- `web/src/routes/projects/[id]/rankings/[rid]/(editor)/players/+page.svelte`

### Frontend — modified
- `web/src/lib/types.ts` — add `Ranking`, `RankingPlayer`; update `Project`, `Player`
- `web/src/lib/api.ts` — add ranking methods; update putRanking URL
- `web/src/routes/projects/[id]/+page.server.ts` — load rankings; redirect if single
- `web/src/routes/projects/[id]/+page.svelte` — ranking cards
- `web/src/tests/mock-api.js` — add ranking endpoints

### Frontend — deleted (pages moved under rankings/[rid])
- `web/src/routes/projects/[id]/ranking/` (directory)
- `web/src/routes/projects/[id]/stats/` (directory)
- `web/src/routes/projects/[id]/h2h/` (directory)
- `web/src/routes/projects/[id]/tournaments/` (directory)

### Docs — modified
- `docs/DESIGN.md`
- `docs/routes.md`
- `docs/modules.md`
- `backend/openapi.yaml`

---

## Task 1: Consolidate migrations

**Files:**
- Delete: `backend/migrations/002_job_progress.sql`
- Replace: `backend/migrations/001_initial.sql`

- [ ] **Step 1: Delete the old migration files and write the new consolidated one**

```bash
rm backend/migrations/002_job_progress.sql
```

Then write `backend/migrations/001_initial.sql` with the full consolidated schema below. The key changes vs the original are: table renamed `ranking_projects` → `projects`, `published` column removed from `projects`, `rank_position` column removed from `players`, `project_events` table replaced by `ranking_events`, new `rankings` and `ranking_players` tables added, `progress` column from migration 002 included directly in `jobs`.

```sql
-- Enums
CREATE TYPE job_kind AS ENUM ('import_tournaments');
CREATE TYPE job_status AS ENUM ('pending', 'running', 'done', 'failed');

-- Users and sessions
CREATE TABLE users (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT        NOT NULL UNIQUE,
    display_name    TEXT        NOT NULL,
    password_hash   TEXT        NOT NULL,
    startgg_api_key TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sessions (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX sessions_user_id_idx ON sessions(user_id);

CREATE TYPE project_member_role AS ENUM ('editor', 'viewer');

-- Projects (container for rankings, players, members)
CREATE TABLE projects (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    game_id     BIGINT,
    game_name   TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX projects_owner_id_idx ON projects(owner_id);

CREATE TABLE project_members (
    project_id  UUID                NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id     UUID                NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL,
    joined_at   TIMESTAMPTZ         NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, user_id)
);

CREATE INDEX project_members_user_id_idx ON project_members(user_id);

CREATE TABLE project_invite_links (
    id          UUID                PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID                NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL,
    created_by  UUID                NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at  TIMESTAMPTZ,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ         NOT NULL DEFAULT NOW()
);

CREATE INDEX project_invite_links_project_id_idx ON project_invite_links(project_id);

-- Rankings (one or more per project; each is an independent ranking view)
CREATE TABLE rankings (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    description TEXT,
    published   BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX rankings_project_id_idx ON rankings(project_id);

-- Players (project-scoped pool; rankings select a subset via ranking_players)
CREATE TABLE players (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id    UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX players_project_id_idx ON players(project_id);

-- Per-ranking player membership with ordering and notes
CREATE TABLE ranking_players (
    ranking_id    UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    player_id     UUID    NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    rank_position INTEGER NOT NULL DEFAULT 0,
    notes         TEXT,
    PRIMARY KEY (ranking_id, player_id)
);

CREATE INDEX ranking_players_player_id_idx ON ranking_players(player_id);

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
CREATE INDEX startgg_accounts_user_id_idx   ON startgg_accounts(startgg_user_id);

-- Tournaments (global, shared across all projects)
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

CREATE INDEX events_start_at_idx      ON events(start_at);
CREATE INDEX events_tournament_id_idx ON events(tournament_id);

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

-- Per-ranking event inclusion (imported by worker per ranking; default included)
CREATE TABLE ranking_events (
    ranking_id UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    event_id   UUID    NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    included   BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (ranking_id, event_id)
);

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

CREATE INDEX entrants_event_id_idx          ON entrants(event_id);
CREATE INDEX entrants_player_id_idx         ON entrants(player_id);
CREATE INDEX entrants_startgg_user_id_idx   ON entrants(startgg_user_id);

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

CREATE INDEX sets_event_id_idx           ON sets(event_id);
CREATE INDEX sets_phase_group_id_idx     ON sets(phase_group_id);
CREATE INDEX sets_winner_entrant_id_idx  ON sets(winner_entrant_id);
CREATE INDEX sets_loser_entrant_id_idx   ON sets(loser_entrant_id);
CREATE INDEX sets_completed_at_idx       ON sets(completed_at);

-- Job queue
CREATE TABLE jobs (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    kind       job_kind    NOT NULL,
    project_id UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    params     JSONB       NOT NULL DEFAULT '{}',
    result     JSONB,
    progress   JSONB,
    status     job_status  NOT NULL DEFAULT 'pending',
    error      TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX jobs_status_idx     ON jobs(status) WHERE status IN ('pending', 'running');
CREATE INDEX jobs_project_id_idx ON jobs(project_id);
```

- [ ] **Step 2: Regenerate the sqlx offline cache**

```bash
cd backend && bash prepare-sqlx.sh
```

Expected: script completes without error, `.sqlx/` directory is updated.

- [ ] **Step 3: Run backend tests to confirm the schema compiles**

```bash
cd backend && bash test.sh
```

Expected: PASS (tests may fail on logic until later tasks, but should compile).

- [ ] **Step 4: Commit**

```bash
git add backend/migrations/001_initial.sql
git rm backend/migrations/002_job_progress.sql
git commit -m "chore(db): consolidate migrations; rename ranking_projects→projects; add rankings/ranking_players/ranking_events"
```

---

## Task 2: Update common models

**Files:**
- Modify: `backend/crates/common/src/models/mod.rs`

- [ ] **Step 1: Write the new models file**

Replace the entire contents of `backend/crates/common/src/models/mod.rs`:

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
    pub progress: Option<serde_json::Value>,
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
    pub startgg_api_key: Option<String>,
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
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Ranking {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub published: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RankingPlayer {
    pub ranking_id: Uuid,
    pub player_id: Uuid,
    pub rank_position: i32,
    pub notes: Option<String>,
}

/// DB-mapped role for project_members rows.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "project_member_role", rename_all = "snake_case")]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
    Editor,
    Viewer,
}

/// Role returned in API responses — includes Owner (synthesised, never stored).
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
    pub email: String,
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

- [ ] **Step 2: Build to check compilation**

```bash
cd backend && cargo build -p common
```

Expected: compiles without error.

- [ ] **Step 3: Commit**

```bash
git add backend/crates/common/src/models/mod.rs
git commit -m "feat(common): add Ranking/RankingPlayer models; remove published from Project, rank_position from Player"
```

---

## Task 3: Update projects.rs — rename table refs, remove published

**Files:**
- Modify: `backend/crates/api/src/routes/projects.rs`

The changes in this file are:
1. All SQL `ranking_projects` → `projects`
2. Remove `published` field from `Project` queries, `ProjectResponse`, and `PatchProjectRequest`
3. Update `require_project_read_access` to check whether any ranking in the project is published (instead of `project.published`)

- [ ] **Step 1: Update ProjectResponse and PatchProjectRequest**

Remove `published: bool` from `ProjectResponse` and `published: Option<bool>` from `PatchProjectRequest`.

```rust
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
    pub owner_has_startgg_key: bool,
}

impl ProjectResponse {
    fn from_project(p: Project, user_role: Option<UserRole>, owner_has_startgg_key: bool) -> Self {
        ProjectResponse {
            id: p.id,
            name: p.name,
            game_id: p.game_id,
            game_name: p.game_name,
            created_at: p.created_at,
            user_role,
            owner_has_startgg_key,
        }
    }
}
```

- [ ] **Step 2: Update require_project_access**

Replace `ranking_projects` with `projects` in the SQL query. Also remove `published` from the SELECT and struct:

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
```

- [ ] **Step 3: Update require_project_read_access**

Now grants access if user is a member OR if any ranking in the project is published:

```rust
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

- [ ] **Step 4: Update all handlers**

In `list_projects`, `create_project`, `get_project`, `patch_project`, `delete_project`: replace every occurrence of `ranking_projects` with `projects` in SQL strings. Remove `published` from all SELECT lists and RETURNING clauses. Remove the `published` field from all `Project { .. }` construction sites. Remove `new_published` logic from `patch_project`.

The updated `patch_project` UPDATE query becomes:
```sql
UPDATE projects SET name = $1 WHERE id = $2
RETURNING id, owner_id, name, game_id, game_name, created_at
```

The updated `create_project` INSERT becomes:
```sql
INSERT INTO projects (owner_id, name, game_id, game_name)
VALUES ($1, $2, $3, $4)
RETURNING id, owner_id, name, game_id, game_name, created_at
```

- [ ] **Step 5: Update tests**

In the test module, update every SQL query that references `ranking_projects` to use `projects`. Remove any test that asserts on `published`. Update `test_unauthenticated_can_access_published_project` — instead of PATCHing the project to set published=true, create a ranking and PATCH that ranking to set published=true (this test can be deferred until rankings exist; for now stub it out or remove it). The test `test_unauthenticated_can_read_stats_of_published_project` references `/projects/:id/stats` which no longer exists at this path — remove or stub it.

- [ ] **Step 6: Build to check compilation**

```bash
cd backend && cargo build -p api
```

- [ ] **Step 7: Commit**

```bash
git add backend/crates/api/src/routes/projects.rs
git commit -m "feat(api): rename ranking_projects→projects; remove published from project; update access helpers"
```

---

## Task 4: Add rankings.rs — CRUD and access helpers

**Files:**
- Create: `backend/crates/api/src/routes/rankings.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`
- Modify: `backend/crates/api/src/routes/projects.rs` (add nest call)

- [ ] **Step 1: Write failing tests**

At the bottom of `rankings.rs` (before the module is complete), add tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, body::Body, http::{Request, StatusCode}};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use tower::ServiceExt;
    use crate::{routes, state::AppState};

    fn make_app(pool: PgPool) -> Router {
        let state = AppState {
            db: pool,
            cors_origin: "http://localhost".into(),
            startgg_base_url: "http://localhost:1".into(),
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
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        format!("session_id={}", body["session_id"].as_str().unwrap())
    }

    async fn with_api_key(pool: &PgPool, email: &str) {
        sqlx::query!("UPDATE users SET startgg_api_key = 'test-key' WHERE email = $1", email)
            .execute(pool).await.unwrap();
    }

    async fn create_project(app: &Router, cookie: &str) -> String {
        let resp = app.clone().oneshot(
            Request::builder().method("POST").uri("/projects")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_vec(&json!({"name": "Test Project"})).unwrap()))
                .unwrap()
        ).await.unwrap();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        v["id"].as_str().unwrap().to_string()
    }

    async fn json_body(resp: axum::response::Response) -> Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_create_and_list_rankings(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "owner_rank").await;
        with_api_key(&pool, "owner_rank@test.com").await;
        let proj_id = create_project(&app, &cookie).await;

        let resp = app.clone().oneshot(
            Request::builder().method("POST")
                .uri(&format!("/projects/{proj_id}/rankings"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(&json!({"name": "2025 Season"})).unwrap()))
                .unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = json_body(resp).await;
        assert_eq!(body["name"], "2025 Season");
        let ranking_id = body["id"].as_str().unwrap().to_string();

        let resp = app.clone().oneshot(
            Request::builder().method("GET")
                .uri(&format!("/projects/{proj_id}/rankings"))
                .header("cookie", &cookie)
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body.as_array().unwrap().len(), 1);
        assert_eq!(body[0]["id"], ranking_id);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_published_ranking_accessible_without_auth(pool: PgPool) {
        let app = make_app(pool.clone());
        let cookie = register(&app, "pub_owner").await;
        with_api_key(&pool, "pub_owner@test.com").await;
        let proj_id = create_project(&app, &cookie).await;

        let resp = app.clone().oneshot(
            Request::builder().method("POST")
                .uri(&format!("/projects/{proj_id}/rankings"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(&json!({"name": "Public"})).unwrap()))
                .unwrap()
        ).await.unwrap();
        let ranking_id = json_body(resp).await["id"].as_str().unwrap().to_string();

        // Private: guest gets 404
        let resp = app.clone().oneshot(
            Request::builder().method("GET")
                .uri(&format!("/projects/{proj_id}/rankings/{ranking_id}"))
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 404);

        // Publish
        app.clone().oneshot(
            Request::builder().method("PATCH")
                .uri(&format!("/projects/{proj_id}/rankings/{ranking_id}"))
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(serde_json::to_vec(&json!({"published": true})).unwrap()))
                .unwrap()
        ).await.unwrap();

        // Published: guest can access
        let resp = app.clone().oneshot(
            Request::builder().method("GET")
                .uri(&format!("/projects/{proj_id}/rankings/{ranking_id}"))
                .body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = json_body(resp).await;
        assert_eq!(body["published"], true);
        assert!(body["user_role"].is_null());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd backend && cargo test -p api -- rankings 2>&1 | tail -20
```

Expected: compile error (rankings module doesn't exist yet).

- [ ] **Step 3: Write rankings.rs**

Create `backend/crates/api/src/routes/rankings.rs`:

```rust
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post, put},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, OptionalAuthUser},
    routes::projects::require_project_access,
    state::AppState,
};
use common::models::{MemberRole, Project, Ranking, UserRole};

// ── Path param structs ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RankingPath {
    pub id: Uuid,   // project_id
    pub rid: Uuid,  // ranking_id
}

#[derive(Deserialize)]
pub struct RankingPlayerPath {
    pub id: Uuid,
    pub rid: Uuid,
    pub pid: Uuid,
}

// ── Access helpers ────────────────────────────────────────────────────────────

/// Checks project membership with at least `min_role`.
/// Returns (project, ranking, role). 404 if ranking doesn't belong to project.
pub async fn require_ranking_access(
    db: &PgPool,
    project_id: Uuid,
    ranking_id: Uuid,
    user_id: Uuid,
    min_role: UserRole,
) -> Result<(Project, Ranking, UserRole)> {
    let (project, role) = require_project_access(db, project_id, user_id, min_role).await?;
    let ranking = sqlx::query_as!(
        Ranking,
        "SELECT id, project_id, name, description, published, created_at
         FROM rankings WHERE id = $1 AND project_id = $2",
        ranking_id,
        project_id,
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok((project, ranking, role))
}

/// Grants access if the user is a project member OR the specific ranking is published.
pub async fn require_ranking_read_access(
    db: &PgPool,
    project_id: Uuid,
    ranking_id: Uuid,
    user_id: Option<Uuid>,
) -> Result<(Project, Ranking, Option<UserRole>)> {
    struct Row {
        id: Uuid,
        owner_id: Uuid,
        name: String,
        game_id: Option<i64>,
        game_name: Option<String>,
        created_at: DateTime<Utc>,
        ranking_id: Uuid,
        ranking_name: String,
        ranking_description: Option<String>,
        ranking_published: bool,
        ranking_created_at: DateTime<Utc>,
        is_owner: Option<bool>,
        member_role: Option<MemberRole>,
    }

    let row = sqlx::query_as!(
        Row,
        r#"SELECT p.id, p.owner_id, p.name, p.game_id, p.game_name, p.created_at,
                  r.id AS ranking_id, r.name AS ranking_name,
                  r.description AS ranking_description,
                  r.published AS ranking_published,
                  r.created_at AS ranking_created_at,
                  (p.owner_id = $3) AS is_owner,
                  CASE WHEN pm.user_id IS NOT NULL THEN pm.role END AS "member_role: MemberRole"
           FROM projects p
           JOIN rankings r ON r.id = $2 AND r.project_id = p.id
           LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $3
           WHERE p.id = $1"#,
        project_id,
        ranking_id,
        user_id.unwrap_or(Uuid::nil()),
    )
    .fetch_optional(db)
    .await?
    .ok_or(AppError::NotFound)?;

    let project = Project {
        id: row.id,
        owner_id: row.owner_id,
        name: row.name,
        game_id: row.game_id,
        game_name: row.game_name,
        created_at: row.created_at,
    };
    let ranking = Ranking {
        id: row.ranking_id,
        project_id,
        name: row.ranking_name,
        description: row.ranking_description,
        published: row.ranking_published,
        created_at: row.ranking_created_at,
    };

    if ranking.published {
        let role = if let Some(uid) = user_id {
            if project.owner_id == uid {
                Some(UserRole::Owner)
            } else if row.member_role.is_some() {
                row.member_role.map(UserRole::from)
            } else {
                None
            }
        } else {
            None
        };
        return Ok((project, ranking, role));
    }

    if let Some(uid) = user_id {
        if project.owner_id == uid {
            return Ok((project, ranking, Some(UserRole::Owner)));
        }
        if let Some(role) = row.member_role {
            return Ok((project, ranking, Some(UserRole::from(role))));
        }
    }

    Err(AppError::NotFound)
}

// ── Request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct CreateRankingRequest {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct PatchRankingRequest {
    name: Option<String>,
    description: Option<String>,
    published: Option<bool>,
}

#[derive(Serialize)]
struct RankingResponse {
    id: Uuid,
    project_id: Uuid,
    name: String,
    description: Option<String>,
    published: bool,
    created_at: DateTime<Utc>,
    user_role: Option<UserRole>,
}

impl RankingResponse {
    fn from_ranking(r: Ranking, role: Option<UserRole>) -> Self {
        RankingResponse {
            id: r.id,
            project_id: r.project_id,
            name: r.name,
            description: r.description,
            published: r.published,
            created_at: r.created_at,
            user_role: role,
        }
    }
}

#[derive(Serialize)]
struct RankingPlayerResponse {
    player_id: Uuid,
    name: String,
    rank_position: i32,
    notes: Option<String>,
}

#[derive(Deserialize)]
struct AddRankingPlayerRequest {
    player_id: Uuid,
}

#[derive(Deserialize)]
struct PatchRankingPlayerRequest {
    notes: Option<String>,
}

#[derive(Deserialize)]
struct ReorderRequest {
    player_ids: Vec<Uuid>,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn list_rankings(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    use crate::routes::projects::require_project_read_access;
    let (_, role) = require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;

    let rankings = sqlx::query_as!(
        Ranking,
        "SELECT id, project_id, name, description, published, created_at
         FROM rankings WHERE project_id = $1 ORDER BY created_at ASC",
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<RankingResponse> = rankings
        .into_iter()
        .map(|r| RankingResponse::from_ranking(r, role.clone()))
        .collect();
    Ok(Json(resp))
}

async fn create_ranking(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateRankingRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("name must not be empty".into()));
    }

    let ranking = sqlx::query_as!(
        Ranking,
        "INSERT INTO rankings (project_id, name, description)
         VALUES ($1, $2, $3)
         RETURNING id, project_id, name, description, published, created_at",
        project_id,
        body.name.trim(),
        body.description.as_deref(),
    )
    .fetch_one(&state.db)
    .await?;

    // Backfill ranking_events for all events already imported for this project.
    sqlx::query!(
        r#"
        INSERT INTO ranking_events (ranking_id, event_id, included)
        SELECT $1, DISTINCT e.id, true
        FROM events e
        JOIN entrants ent ON ent.event_id = e.id
        JOIN players pl ON pl.id = ent.player_id AND pl.project_id = $2
        ON CONFLICT DO NOTHING
        "#,
        ranking.id,
        project_id,
    )
    .execute(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(RankingResponse::from_ranking(ranking, Some(UserRole::Owner))),
    ))
}

async fn get_ranking(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    let (_, ranking, role) =
        require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;
    Ok(Json(RankingResponse::from_ranking(ranking, role)))
}

async fn patch_ranking(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
    Json(body): Json<PatchRankingRequest>,
) -> Result<impl IntoResponse> {
    let (_, ranking, role) =
        require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;

    if body.published == Some(true) && !matches!(role, UserRole::Owner) {
        return Err(AppError::Forbidden);
    }

    let new_name = if let Some(ref n) = body.name {
        let t = n.trim();
        if t.is_empty() {
            return Err(AppError::UnprocessableEntity("name must not be empty".into()));
        }
        t.to_string()
    } else {
        ranking.name.clone()
    };

    let updated = sqlx::query_as!(
        Ranking,
        "UPDATE rankings SET name = $1, description = $2, published = $3
         WHERE id = $4
         RETURNING id, project_id, name, description, published, created_at",
        new_name,
        body.description.as_deref().or(ranking.description.as_deref()),
        body.published.unwrap_or(ranking.published),
        path.rid,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(RankingResponse::from_ranking(updated, Some(role))))
}

async fn delete_ranking(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Owner).await?;
    sqlx::query!("DELETE FROM rankings WHERE id = $1", path.rid)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Ranking player membership ─────────────────────────────────────────────────

async fn list_ranking_players(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    struct Row {
        player_id: Uuid,
        name: String,
        rank_position: i32,
        notes: Option<String>,
    }

    let rows = sqlx::query_as!(
        Row,
        "SELECT rp.player_id, pl.name, rp.rank_position, rp.notes
         FROM ranking_players rp
         JOIN players pl ON pl.id = rp.player_id
         WHERE rp.ranking_id = $1
         ORDER BY rp.rank_position ASC, pl.created_at ASC",
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<RankingPlayerResponse> = rows
        .into_iter()
        .map(|r| RankingPlayerResponse {
            player_id: r.player_id,
            name: r.name,
            rank_position: r.rank_position,
            notes: r.notes,
        })
        .collect();
    Ok(Json(resp))
}

async fn add_ranking_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
    Json(body): Json<AddRankingPlayerRequest>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;

    // Verify the player belongs to this project.
    sqlx::query!(
        "SELECT id FROM players WHERE id = $1 AND project_id = $2",
        body.player_id,
        path.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let max_pos: i32 = sqlx::query_scalar!(
        "SELECT COALESCE(MAX(rank_position), 0) FROM ranking_players WHERE ranking_id = $1",
        path.rid,
    )
    .fetch_one(&state.db)
    .await?
    .unwrap_or(0);

    sqlx::query!(
        "INSERT INTO ranking_players (ranking_id, player_id, rank_position)
         VALUES ($1, $2, $3)
         ON CONFLICT DO NOTHING",
        path.rid,
        body.player_id,
        max_pos + 1,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::CREATED)
}

async fn remove_ranking_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPlayerPath>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;
    let result = sqlx::query!(
        "DELETE FROM ranking_players WHERE ranking_id = $1 AND player_id = $2",
        path.rid,
        path.pid,
    )
    .execute(&state.db)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn patch_ranking_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPlayerPath>,
    Json(body): Json<PatchRankingPlayerRequest>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;
    let result = sqlx::query!(
        "UPDATE ranking_players SET notes = $1 WHERE ranking_id = $2 AND player_id = $3",
        body.notes.as_deref(),
        path.rid,
        path.pid,
    )
    .execute(&state.db)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::OK)
}

async fn reorder_ranking_players(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
    Json(body): Json<ReorderRequest>,
) -> Result<impl IntoResponse> {
    require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;

    let existing_ids: Vec<Uuid> = sqlx::query_scalar!(
        "SELECT player_id FROM ranking_players WHERE ranking_id = $1",
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    let existing_set: std::collections::HashSet<Uuid> = existing_ids.into_iter().collect();
    let input_set: std::collections::HashSet<Uuid> = body.player_ids.iter().copied().collect();

    if body.player_ids.len() != existing_set.len() || input_set.len() != body.player_ids.len() {
        return Err(AppError::UnprocessableEntity(
            "player_ids must contain exactly all players in this ranking, no duplicates".into(),
        ));
    }
    for &pid in &body.player_ids {
        if !existing_set.contains(&pid) {
            return Err(AppError::UnprocessableEntity(
                "player_ids contains an id not in this ranking".into(),
            ));
        }
    }

    let mut tx = state.db.begin().await?;
    for (i, &player_id) in body.player_ids.iter().enumerate() {
        sqlx::query!(
            "UPDATE ranking_players SET rank_position = $1
             WHERE ranking_id = $2 AND player_id = $3",
            (i + 1) as i32,
            path.rid,
            player_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(StatusCode::OK)
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_rankings).post(create_ranking))
        .route("/{rid}", get(get_ranking).patch(patch_ranking).delete(delete_ranking))
        .route("/{rid}/players", get(list_ranking_players).post(add_ranking_player))
        .route("/{rid}/players/{pid}", delete(remove_ranking_player).patch(patch_ranking_player))
        .route("/{rid}/ranking", put(reorder_ranking_players))
}
```

**Note:** The `DISTINCT` keyword in the CREATE backfill query above is syntactically incorrect as written. The correct form is `SELECT DISTINCT e.id` not `SELECT $1, DISTINCT e.id`. The actual backfill query should be:

```rust
    sqlx::query!(
        r#"
        INSERT INTO ranking_events (ranking_id, event_id, included)
        SELECT DISTINCT $1, e.id, true
        FROM events e
        JOIN entrants ent ON ent.event_id = e.id
        JOIN players pl ON pl.id = ent.player_id AND pl.project_id = $2
        ON CONFLICT DO NOTHING
        "#,
        ranking.id,
        project_id,
    )
    .execute(&state.db)
    .await?;
```

- [ ] **Step 4: Wire rankings module into mod.rs and projects router**

Add to `backend/crates/api/src/routes/mod.rs`:
```rust
pub mod rankings;
```

Add to the `router()` function in `projects.rs`, inside the `Router::new()` chain:
```rust
.nest("/{id}/rankings", crate::routes::rankings::router())
```

Remove the old project-scoped route entries that moved to ranking scope (these will be added back in Task 7):
```rust
// Remove these lines:
.route("/{id}/tournaments", get(t::list_tournaments))
.route("/{id}/events/{eid}", patch(t::patch_event))
.route("/{id}/stats", get(t::get_stats))
.route("/{id}/stats/{player_id}", get(t::get_player_stats))
.route("/{id}/head-to-head", get(t::get_head_to_head))
.route("/{id}/head-to-head/{pid_a}/{pid_b}/sets", get(t::get_h2h_sets))
.route("/{id}/ranking", put(crate::routes::players::reorder_players))
```

- [ ] **Step 5: Run the new tests**

```bash
cd backend && cargo test -p api -- rankings 2>&1 | tail -30
```

Expected: `test_create_and_list_rankings` PASS, `test_published_ranking_accessible_without_auth` PASS.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/rankings.rs backend/crates/api/src/routes/mod.rs backend/crates/api/src/routes/projects.rs
git commit -m "feat(api): add rankings CRUD, ranking player membership, access helpers"
```

---

## Task 5: Update players.rs — remove rank_position

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`

`rank_position` no longer lives on the `players` table. Every query that references it must be updated, and `reorder_players` is removed (it moved to `rankings.rs`).

- [ ] **Step 1: Update Player queries**

Find every `SELECT` in this file that includes `rank_position` and remove it. The `Player` struct no longer has that field.

`list_players` query changes from:
```sql
SELECT id, project_id, name, rank_position, created_at
FROM players WHERE project_id = $1
ORDER BY rank_position ASC, created_at ASC
```
to:
```sql
SELECT id, project_id, name, created_at
FROM players WHERE project_id = $1
ORDER BY created_at ASC
```

`add_player` INSERT changes from:
```sql
INSERT INTO players (project_id, name, rank_position)
VALUES ($1, $2, (SELECT COALESCE(MAX(rank_position), 0) + 1 FROM players WHERE project_id = $1))
RETURNING id, project_id, name, rank_position, created_at
```
to:
```sql
INSERT INTO players (project_id, name)
VALUES ($1, $2)
RETURNING id, project_id, name, created_at
```

`create_player_with_account` query changes similarly (remove `rank_position`).

`rename_player` RETURNING clause: remove `rank_position`.

Any other query that SELECTs or returns `rank_position` must be updated.

- [ ] **Step 2: Remove reorder_players**

Delete the `reorder_players` function and its `ReorderRequest` type (they now live in `rankings.rs`).

- [ ] **Step 3: Build to check compilation**

```bash
cd backend && cargo build -p api
```

- [ ] **Step 4: Commit**

```bash
git add backend/crates/api/src/routes/players.rs
git commit -m "feat(api): remove rank_position from players; remove reorder_players (moved to rankings)"
```

---

## Task 6: Update tournaments.rs — scope all handlers to ranking_id

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`
- Modify: `backend/crates/api/src/routes/projects.rs` (re-add routes under rankings nest)

All handlers now take `RankingPath` (project_id + ranking_id). Every SQL query is updated to use `ranking_id` instead of `project_id` for event/player scoping.

- [ ] **Step 1: Update path structs**

Replace `ProjectEventPath` and `H2HSetPath` with ranking-scoped versions:

```rust
use crate::routes::rankings::{RankingPath, require_ranking_access, require_ranking_read_access};

#[derive(Deserialize)]
pub struct RankingEventPath {
    pub id: Uuid,
    pub rid: Uuid,
    pub eid: Uuid,
}

#[derive(Deserialize)]
pub struct RankingH2HPath {
    pub id: Uuid,
    pub rid: Uuid,
    pub pid_a: Uuid,
    pub pid_b: Uuid,
}
```

- [ ] **Step 2: Update list_tournaments**

Change signature to take `Path(path): Path<RankingPath>` and call `require_ranking_read_access`. Replace the SQL query's `project_events` JOIN with `ranking_events`:

```sql
FROM ranking_events re
JOIN events      e ON e.id = re.event_id
JOIN tournaments t ON t.id = e.tournament_id
WHERE re.ranking_id = $1
ORDER BY t.start_at DESC NULLS LAST, t.name ASC, e.name ASC
```

The `re.included` field in the response stays the same.

- [ ] **Step 3: Update patch_event**

Change signature to take `Path(path): Path<RankingEventPath>` and call `require_ranking_access(..., UserRole::Editor)`.

Verify event belongs to this ranking:
```sql
SELECT ranking_id FROM ranking_events
WHERE ranking_id = $1 AND event_id = $2
```

Upsert:
```sql
INSERT INTO ranking_events (ranking_id, event_id, included)
VALUES ($1, $2, $3)
ON CONFLICT (ranking_id, event_id) DO UPDATE SET included = EXCLUDED.included
```

Return updated event still uses the same `EventRow` query but joins `ranking_events`:
```sql
FROM events e
JOIN ranking_events re ON re.event_id = e.id AND re.ranking_id = $1
WHERE e.id = $2
```

- [ ] **Step 4: Update get_stats**

Change signature to take `Path(path): Path<RankingPath>`. The players are now fetched via `ranking_players` and events via `ranking_events`. The `$1` parameter throughout becomes the `ranking_id`:

Player fetch query:
```sql
SELECT rp.player_id AS id, pl.name
FROM ranking_players rp
JOIN players pl ON pl.id = rp.player_id
WHERE rp.ranking_id = $1
ORDER BY rp.rank_position ASC, pl.created_at ASC
```

In the main sets query, replace:
```sql
LEFT JOIN players wp ON wp.id = we.player_id AND wp.project_id = $1
LEFT JOIN players lp ON lp.id = le.player_id AND lp.project_id = $1
JOIN project_events pe ON pe.event_id = s.event_id AND pe.project_id = $1
WHERE pe.included = true
```
with:
```sql
LEFT JOIN ranking_players rwp ON rwp.player_id = we.player_id AND rwp.ranking_id = $1
LEFT JOIN players wp ON wp.id = rwp.player_id
LEFT JOIN ranking_players rlp ON rlp.player_id = le.player_id AND rlp.ranking_id = $1
LEFT JOIN players lp ON lp.id = rlp.player_id
JOIN ranking_events re ON re.event_id = s.event_id AND re.ranking_id = $1
WHERE re.included = true
```

Also update:
```sql
AND (wp.id IS NOT NULL OR lp.id IS NOT NULL)
```
becomes:
```sql
AND (rwp.player_id IS NOT NULL OR rlp.player_id IS NOT NULL)
```

Call `require_ranking_read_access` instead of `require_project_read_access`.

- [ ] **Step 5: Update get_player_stats**

Change to `Path((path, player_id)): Path<(RankingPath, Uuid)>` (or define a custom struct). Call `require_ranking_read_access`. Apply the same player/event join changes as Step 4.

Verify the player belongs to this ranking:
```sql
SELECT pl.name FROM ranking_players rp
JOIN players pl ON pl.id = rp.player_id
WHERE rp.ranking_id = $1 AND rp.player_id = $2
```

- [ ] **Step 6: Update get_head_to_head**

Replace:
```sql
JOIN players  wp ON wp.id = we.player_id AND wp.project_id = $1
JOIN players  lp ON lp.id = le.player_id AND lp.project_id = $1
JOIN project_events pe ON pe.event_id = s.event_id AND pe.project_id = $1
WHERE pe.included = true
```
with:
```sql
JOIN ranking_players rwp ON rwp.player_id = we.player_id AND rwp.ranking_id = $1
JOIN players  wp ON wp.id = rwp.player_id
JOIN ranking_players rlp ON rlp.player_id = le.player_id AND rlp.ranking_id = $1
JOIN players  lp ON lp.id = rlp.player_id
JOIN ranking_events re ON re.event_id = s.event_id AND re.ranking_id = $1
WHERE re.included = true
```

- [ ] **Step 7: Update get_h2h_sets**

Same player/event join changes as Step 6.

- [ ] **Step 8: Add tournament deletion handler**

```rust
pub async fn delete_tournament(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, tournament_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;
    use crate::routes::projects::require_project_access;

    sqlx::query!(
        r#"
        DELETE FROM ranking_events
        WHERE event_id IN (
            SELECT e.id FROM events e WHERE e.tournament_id = $1
        )
        AND ranking_id IN (
            SELECT id FROM rankings WHERE project_id = $2
        )
        "#,
        tournament_id,
        project_id,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 9: Update tournaments.rs router and re-add ranking routes to projects.rs**

Update `tournaments.rs` router (no longer needed as a standalone — it now just exports handlers):
```rust
pub fn router() -> axum::Router<AppState> {
    use axum::routing::{delete, get, patch};
    axum::Router::new()
        .route("/tournaments", get(list_tournaments))
        .route("/tournaments/{tid}", delete(delete_tournament))
        .route("/events/{eid}", patch(patch_event))
        .route("/stats", get(get_stats))
        .route("/stats/{player_id}", get(get_player_stats))
        .route("/head-to-head", get(get_head_to_head))
        .route("/head-to-head/{pid_a}/{pid_b}/sets", get(get_h2h_sets))
}
```

In `rankings.rs` router, nest the tournament handlers under the ranking:
```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_rankings).post(create_ranking))
        .route("/{rid}", get(get_ranking).patch(patch_ranking).delete(delete_ranking))
        .route("/{rid}/players", get(list_ranking_players).post(add_ranking_player))
        .route("/{rid}/players/{pid}", delete(remove_ranking_player).patch(patch_ranking_player))
        .route("/{rid}/ranking", put(reorder_ranking_players))
        .nest("/{rid}", crate::routes::tournaments::router())
}
```

Keep `DELETE /projects/:id/tournaments/:tid` at the project level (add it to `projects.rs` router):
```rust
.route("/{id}/tournaments/{tid}", delete(crate::routes::tournaments::delete_tournament))
```

- [ ] **Step 10: Build and run tests**

```bash
cd backend && cargo build -p api && cargo test -p api 2>&1 | tail -30
```

- [ ] **Step 11: Commit**

```bash
git add backend/crates/api/src/routes/tournaments.rs backend/crates/api/src/routes/projects.rs backend/crates/api/src/routes/rankings.rs
git commit -m "feat(api): scope stats/H2H/tournaments to ranking_id; add tournament delete endpoint"
```

---

## Task 7: Update worker import.rs

**Files:**
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Rename table reference and write ranking_events rows**

Change the project query from `ranking_projects` to `projects`:
```rust
let project = sqlx::query!(
    "SELECT game_id, game_name FROM projects WHERE id = $1",
    project_id,
)
.fetch_one(pool)
.await?;
```

After inserting events for a tournament (after the `project_events` upsert block), replace any `project_events` insert with a `ranking_events` insert for all rankings in the project:

```rust
// For each new event_id discovered:
sqlx::query!(
    r#"
    INSERT INTO ranking_events (ranking_id, event_id, included)
    SELECT r.id, $1, true
    FROM rankings r WHERE r.project_id = $2
    ON CONFLICT DO NOTHING
    "#,
    event_id,
    project_id,
)
.execute(pool)
.await?;
```

Search for all occurrences of `project_events` in this file and replace with the `ranking_events` logic above.

- [ ] **Step 2: Build worker**

```bash
cd backend && cargo build -p worker
```

- [ ] **Step 3: Commit**

```bash
git add backend/crates/worker/src/import.rs
git commit -m "feat(worker): rename projects table ref; write ranking_events for all project rankings on import"
```

---

## Task 8: Update e2e tests

**Files:**
- Modify: `backend/crates/e2e/tests/full_flow.rs`

The e2e test creates a project, adds players, runs an import, then checks stats/H2H. After the change, it must also create a ranking and add players to it before checking stats.

- [ ] **Step 1: Update full_flow.rs**

After the project is created and before the import, add a helper to create a ranking and add all test players to it. Then update all stat/H2H endpoint calls from `/projects/{id}/stats` to `/projects/{id}/rankings/{rid}/stats` etc.

Key changes:
1. Replace any `ranking_projects` SQL refs with `projects`
2. After creating the project, create a ranking:
```rust
async fn create_ranking(app: &Router, cookie: &str, project_id: &str, name: &str) -> String {
    let resp = post_json(app, &format!("/projects/{project_id}/rankings"), cookie,
        json!({"name": name})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    read_json(resp).await["id"].as_str().unwrap().to_string()
}
```
3. After adding players to the project, also add them to the ranking:
```rust
async fn add_player_to_ranking(app: &Router, cookie: &str, project_id: &str, ranking_id: &str, player_id: &str) {
    let resp = post_json(app, &format!("/projects/{project_id}/rankings/{ranking_id}/players"),
        cookie, json!({"player_id": player_id})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
}
```
4. Update all endpoint calls:
   - `/projects/{id}/stats` → `/projects/{id}/rankings/{rid}/stats`
   - `/projects/{id}/head-to-head` → `/projects/{id}/rankings/{rid}/head-to-head`
   - `/projects/{id}/tournaments` → `/projects/{id}/rankings/{rid}/tournaments`
   - `/projects/{id}/ranking` (PUT) → `/projects/{id}/rankings/{rid}/ranking`

- [ ] **Step 2: Run e2e tests**

```bash
cd backend && bash test.sh
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add backend/crates/e2e/tests/full_flow.rs
git commit -m "test(e2e): adapt full_flow for multi-ranking schema"
```

---

## Task 9: Regenerate sqlx offline cache

**Files:**
- Modify: `backend/.sqlx/` (generated)

- [ ] **Step 1: Regenerate**

```bash
cd backend && bash prepare-sqlx.sh
```

Expected: script completes, `.sqlx/` files are updated.

- [ ] **Step 2: Run full backend test suite**

```bash
cd backend && bash test.sh
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add backend/.sqlx/
git commit -m "chore(sqlx): regenerate offline query cache for multi-ranking schema"
```

---

## Task 10: Update frontend types.ts and api.ts

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/api.ts`

- [ ] **Step 1: Update types.ts**

Remove `published: boolean` from `Project`. Remove `rank_position: number` from `Player`. Add `Ranking` and `RankingPlayer` types:

```typescript
export interface Project {
  id: string;
  name: string;
  game_id: number | null;
  game_name: string | null;
  created_at: string;
  user_role: "owner" | "editor" | "viewer" | null;
  owner_has_startgg_key: boolean;
}

export interface Player {
  id: string;
  project_id: string;
  name: string;
  created_at: string;
  accounts: Account[];
}

export interface Ranking {
  id: string;
  project_id: string;
  name: string;
  description: string | null;
  published: boolean;
  created_at: string;
  user_role: "owner" | "editor" | "viewer" | null;
}

export interface RankingPlayer {
  player_id: string;
  name: string;
  rank_position: number;
  notes: string | null;
}
```

- [ ] **Step 2: Update api.ts**

Replace `putRanking` with a ranking-scoped version and add ranking CRUD methods:

```typescript
import { env } from "$env/dynamic/public";

export function makeApi(fetchFn: typeof fetch) {
  async function req(
    method: string,
    path: string,
    body?: unknown,
  ): Promise<Response> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["Content-Type"] = "application/json";
    return fetchFn(env.PUBLIC_API_URL + path, {
      method,
      credentials: "include",
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  }

  return {
    get: (path: string) => req("GET", path),
    post: (path: string, body?: unknown) => req("POST", path, body),
    patch: (path: string, body: unknown) => req("PATCH", path, body),
    put: (path: string, body: unknown) => req("PUT", path, body),
    delete: (path: string) => req("DELETE", path),
    putRanking: (projectId: string, rankingId: string, playerIds: string[]) =>
      req("PUT", `/projects/${projectId}/rankings/${rankingId}/ranking`, {
        player_ids: playerIds,
      }),
    createRanking: (projectId: string, name: string, description?: string) =>
      req("POST", `/projects/${projectId}/rankings`, { name, description }),
    patchRanking: (
      projectId: string,
      rankingId: string,
      body: { name?: string; description?: string; published?: boolean },
    ) => req("PATCH", `/projects/${projectId}/rankings/${rankingId}`, body),
    deleteRanking: (projectId: string, rankingId: string) =>
      req("DELETE", `/projects/${projectId}/rankings/${rankingId}`),
    addRankingPlayer: (
      projectId: string,
      rankingId: string,
      playerId: string,
    ) =>
      req("POST", `/projects/${projectId}/rankings/${rankingId}/players`, {
        player_id: playerId,
      }),
    removeRankingPlayer: (
      projectId: string,
      rankingId: string,
      playerId: string,
    ) =>
      req(
        "DELETE",
        `/projects/${projectId}/rankings/${rankingId}/players/${playerId}`,
      ),
    patchRankingPlayer: (
      projectId: string,
      rankingId: string,
      playerId: string,
      notes: string | null,
    ) =>
      req(
        "PATCH",
        `/projects/${projectId}/rankings/${rankingId}/players/${playerId}`,
        { notes },
      ),
  };
}
```

- [ ] **Step 3: Run unit tests**

```bash
cd web && npm run test:unit
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/types.ts web/src/lib/api.ts
git commit -m "feat(web): update types for multi-ranking schema; update api.ts with ranking methods"
```

---

## Task 11: Update frontend project overview — rankings list

**Files:**
- Modify: `web/src/routes/projects/[id]/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/+page.svelte`

The project overview now loads the project's rankings and redirects to the first ranking if there is exactly one; otherwise shows a list.

- [ ] **Step 1: Update +page.server.ts**

```typescript
import { redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import type { Ranking } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals, parent }) => {
  const { project } = await parent();
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/rankings`);
  const rankings: Ranking[] = res.ok ? await res.json() : [];

  if (rankings.length === 1) {
    const role = project.user_role;
    const rid = rankings[0].id;
    if (role === "editor" || role === "owner") {
      redirect(303, `/projects/${params.id}/rankings/${rid}/players`);
    }
    redirect(303, `/projects/${params.id}/rankings/${rid}/ranking`);
  }

  return { rankings };
};
```

- [ ] **Step 2: Update +page.svelte**

Replace the current page content with a rankings list. Use `Card` components from shadcn-svelte:

```svelte
<script lang="ts">
  import type { PageData } from "./$types";
  import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";

  let { data }: { data: PageData } = $props();
  const { project, rankings } = data;
  const isEditor = project.user_role === "owner" || project.user_role === "editor";
</script>

<div class="container mx-auto py-8 max-w-3xl">
  <div class="flex items-center justify-between mb-6">
    <h1 class="text-2xl font-bold">{project.name}</h1>
    {#if isEditor}
      <Button href="/projects/{project.id}/rankings/new">New ranking</Button>
    {/if}
  </div>

  {#if rankings.length === 0}
    <p class="text-muted-foreground">No rankings yet. Create one to get started.</p>
  {:else}
    <div class="flex flex-col gap-3">
      {#each rankings as ranking}
        <a href="/projects/{project.id}/rankings/{ranking.id}/ranking">
          <Card class="hover:bg-muted/50 transition-colors cursor-pointer">
            <CardHeader>
              <div class="flex items-center justify-between">
                <CardTitle>{ranking.name}</CardTitle>
                {#if ranking.published}
                  <Badge variant="secondary">Public</Badge>
                {:else}
                  <Badge variant="outline">Private</Badge>
                {/if}
              </div>
              {#if ranking.description}
                <CardDescription>{ranking.description}</CardDescription>
              {/if}
            </CardHeader>
          </Card>
        </a>
      {/each}
    </div>
  {/if}
</div>
```

- [ ] **Step 3: Add new ranking creation page**

Create `web/src/routes/projects/[id]/rankings/new/+page.server.ts`:

```typescript
import { redirect, fail } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ parent }) => {
  const { project } = await parent();
  if (project.user_role !== "owner" && project.user_role !== "editor") {
    redirect(303, `/projects/${project.id}`);
  }
  return {};
};

export const actions = {
  default: async ({ request, params, locals }) => {
    const { api } = locals;
    const data = await request.formData();
    const name = data.get("name") as string;
    const description = (data.get("description") as string) || undefined;

    const res = await api.post(`/projects/${params.id}/rankings`, {
      name,
      description,
    });
    if (!res.ok) {
      const body = await res.json().catch(() => ({}));
      return fail(res.status, { error: body.message ?? "Failed to create ranking" });
    }
    const ranking = await res.json();
    redirect(303, `/projects/${params.id}/rankings/${ranking.id}/players`);
  },
} satisfies Actions;
```

Create `web/src/routes/projects/[id]/rankings/new/+page.svelte`:

```svelte
<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import type { ActionData } from "./$types";

  let { form }: { form: ActionData } = $props();
</script>

<div class="container mx-auto py-8 max-w-md">
  <h1 class="text-2xl font-bold mb-6">New ranking</h1>
  <form method="POST" class="flex flex-col gap-4">
    <div class="flex flex-col gap-1">
      <Label for="name">Name</Label>
      <Input id="name" name="name" required placeholder="2025 Season" />
    </div>
    <div class="flex flex-col gap-1">
      <Label for="description">Description (optional)</Label>
      <Input id="description" name="description" placeholder="Brief description of this ranking" />
    </div>
    {#if form?.error}
      <p class="text-destructive text-sm">{form.error}</p>
    {/if}
    <Button type="submit">Create ranking</Button>
  </form>
</div>
```

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/projects/
git commit -m "feat(web): project overview shows rankings list; add new ranking form"
```

---

## Task 12: Add ranking layout and move existing views

**Files:**
- Create: `web/src/routes/projects/[id]/rankings/[rid]/+layout.server.ts`
- Create/move: stat, h2h, ranking, tournaments pages under rankings/[rid]
- Delete old project-level stat/h2h/ranking/tournaments pages

The ranking layout loads the ranking and makes it available to all child routes. Child pages call `parent()` and get both `project` and `ranking`.

- [ ] **Step 1: Create ranking layout**

Create `web/src/routes/projects/[id]/rankings/[rid]/+layout.server.ts`:

```typescript
import { error } from "@sveltejs/kit";
import type { LayoutServerLoad } from "./$types";
import type { Ranking } from "$lib/types";

export const load: LayoutServerLoad = async ({ params, locals }) => {
  const { api } = locals;
  const res = await api.get(`/projects/${params.id}/rankings/${params.rid}`);
  if (!res.ok) {
    error(res.status === 404 ? 404 : res.status, {
      message: res.status === 404 ? "not_found" : "error",
    });
  }
  const ranking: Ranking = await res.json();
  return { ranking };
};
```

- [ ] **Step 2: Create ranking overview page**

Create `web/src/routes/projects/[id]/rankings/[rid]/+page.server.ts`:

```typescript
import { redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";

export const load: PageServerLoad = async ({ params, parent }) => {
  const { project } = await parent();
  const role = project.user_role;
  if (role === "editor" || role === "owner") {
    redirect(303, `/projects/${params.id}/rankings/${params.rid}/players`);
  }
  redirect(303, `/projects/${params.id}/rankings/${params.rid}/ranking`);
};
```

- [ ] **Step 3: Move ranking page**

Copy `web/src/routes/projects/[id]/ranking/+page.server.ts` to `web/src/routes/projects/[id]/rankings/[rid]/ranking/+page.server.ts`.

Update the API call from `/projects/${params.id}/ranking` to `/projects/${params.id}/rankings/${params.rid}/ranking` (GET) and update PUT calls similarly.

Copy `web/src/routes/projects/[id]/ranking/+page.svelte` to the new location. Update any `api.putRanking` calls to pass `params.rid` as the second argument.

- [ ] **Step 4: Move stats page**

Copy `web/src/routes/projects/[id]/stats/` files to `web/src/routes/projects/[id]/rankings/[rid]/stats/`.

In `+page.server.ts`, update API calls from `/projects/${params.id}/stats` to `/projects/${params.id}/rankings/${params.rid}/stats`.

- [ ] **Step 5: Move h2h page**

Copy `web/src/routes/projects/[id]/h2h/` files to `web/src/routes/projects/[id]/rankings/[rid]/h2h/`.

Update API calls from `/projects/${params.id}/head-to-head` to `/projects/${params.id}/rankings/${params.rid}/head-to-head`.

- [ ] **Step 6: Move tournaments page**

Copy `web/src/routes/projects/[id]/tournaments/` files to `web/src/routes/projects/[id]/rankings/[rid]/tournaments/`.

Update API calls:
- GET from `/projects/${params.id}/tournaments` to `/projects/${params.id}/rankings/${params.rid}/tournaments`
- PATCH event from `/projects/${params.id}/events/${eid}` to `/projects/${params.id}/rankings/${params.rid}/events/${eid}`

Add a delete tournament button: a `Button` with `variant="destructive"` in a `Dialog` confirmation. On confirm, call `api.delete(`/projects/${params.id}/tournaments/${tournament.id}`)` then reload.

- [ ] **Step 7: Update navigation tabs in layout**

The project layout (`web/src/routes/projects/[id]/+layout.svelte`) currently links to `/stats`, `/h2h`, `/ranking`, `/tournaments`. These must now include the ranking ID. Read the current layout file first, then update the tab `href` values to include `/rankings/${ranking.id}/`.

This requires the ranking ID to be available in the project layout. Either pass it from the ranking layout (via a writable store or rune) or restructure so the tabs are in the ranking layout instead.

The cleanest approach: move the per-ranking navigation tabs out of the project layout and into a new `web/src/routes/projects/[id]/rankings/[rid]/+layout.svelte` that reads `ranking` from `parent()` and renders the tab bar.

- [ ] **Step 8: Delete old route directories**

```bash
rm -rf web/src/routes/projects/\[id\]/ranking
rm -rf web/src/routes/projects/\[id\]/stats
rm -rf web/src/routes/projects/\[id\]/h2h
rm -rf web/src/routes/projects/\[id\]/tournaments
```

- [ ] **Step 9: Commit**

```bash
git add web/src/routes/
git commit -m "feat(web): add ranking layout; move stats/h2h/ranking/tournaments under rankings/[rid]"
```

---

## Task 13: Add ranking player editor

**Files:**
- Create: `web/src/routes/projects/[id]/rankings/[rid]/(editor)/players/+page.server.ts`
- Create: `web/src/routes/projects/[id]/rankings/[rid]/(editor)/players/+page.svelte`

This page shows two columns: the full project player pool on the left and the players currently in this ranking on the right. Editors can add/remove players and edit notes.

- [ ] **Step 1: Create +page.server.ts**

```typescript
import { redirect } from "@sveltejs/kit";
import type { PageServerLoad } from "./$types";
import type { Player, RankingPlayer } from "$lib/types";

export const load: PageServerLoad = async ({ params, locals, parent }) => {
  const { project } = await parent();
  if (project.user_role !== "owner" && project.user_role !== "editor") {
    redirect(303, `/projects/${params.id}/rankings/${params.rid}/ranking`);
  }

  const { api } = locals;
  const [poolRes, rankingPlayersRes] = await Promise.all([
    api.get(`/projects/${params.id}/players`),
    api.get(`/projects/${params.id}/rankings/${params.rid}/players`),
  ]);

  const pool: Player[] = poolRes.ok ? await poolRes.json() : [];
  const rankingPlayers: RankingPlayer[] = rankingPlayersRes.ok
    ? await rankingPlayersRes.json()
    : [];

  return { pool, rankingPlayers };
};
```

- [ ] **Step 2: Create +page.svelte**

```svelte
<script lang="ts">
  import type { PageData } from "./$types";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import { makeApi } from "$lib/api";
  import { invalidateAll } from "$app/navigation";

  let { data }: { data: PageData } = $props();
  const api = makeApi(fetch);

  const rankingPlayerIds = $derived(
    new Set(data.rankingPlayers.map((rp) => rp.player_id)),
  );

  async function addPlayer(playerId: string) {
    await api.addRankingPlayer(data.project.id, data.ranking.id, playerId);
    await invalidateAll();
  }

  async function removePlayer(playerId: string) {
    await api.removeRankingPlayer(data.project.id, data.ranking.id, playerId);
    await invalidateAll();
  }
</script>

<div class="container mx-auto py-8 max-w-4xl">
  <h2 class="text-xl font-semibold mb-4">Players in "{data.ranking.name}"</h2>

  <div class="grid grid-cols-2 gap-6">
    <div>
      <h3 class="font-medium mb-2 text-muted-foreground">Project pool</h3>
      <div class="flex flex-col gap-2">
        {#each data.pool as player}
          <div class="flex items-center justify-between border rounded px-3 py-2">
            <span>{player.name}</span>
            {#if rankingPlayerIds.has(player.id)}
              <Badge variant="secondary">In ranking</Badge>
            {:else}
              <Button size="sm" onclick={() => addPlayer(player.id)}>Add</Button>
            {/if}
          </div>
        {/each}
      </div>
    </div>

    <div>
      <h3 class="font-medium mb-2 text-muted-foreground">In this ranking</h3>
      <div class="flex flex-col gap-2">
        {#each data.rankingPlayers as rp}
          <div class="flex items-center justify-between border rounded px-3 py-2">
            <div>
              <span>{rp.name}</span>
              {#if rp.notes}
                <p class="text-xs text-muted-foreground">{rp.notes}</p>
              {/if}
            </div>
            <Button
              size="sm"
              variant="destructive"
              onclick={() => removePlayer(rp.player_id)}
            >Remove</Button>
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>
```

- [ ] **Step 3: Run e2e tests**

```bash
cd web && npm run test:e2e
```

Expected: PASS. If tests reference old paths, update `tests/mock-api.js` to handle the new `/rankings/:rid/` endpoints.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/projects/
git commit -m "feat(web): add ranking player editor — add/remove players from a ranking"
```

---

## Task 14: Update all documentation

**Files:**
- Modify: `docs/DESIGN.md`
- Modify: `docs/routes.md`
- Modify: `docs/modules.md`
- Modify: `backend/openapi.yaml`

- [ ] **Step 1: Update docs/DESIGN.md**

Update the **Data Model** section entities diagram to reflect the new hierarchy:

```
users
  └── projects (renamed from ranking_projects)
        ├── project_members
        ├── project_invite_links
        ├── players (pool)
        │     └── startgg_accounts
        ├── jobs
        └── rankings (NEW)
              ├── ranking_players (ranking_id, player_id, rank_position, notes)
              └── ranking_events  (ranking_id, event_id, included)
```

Update **Key Relationships**:
- Remove mention of `rank_position` on `players`
- Remove mention of `project_events`
- Remove `published` from `ranking_projects`
- Add: "A **ranking** belongs to exactly one **project**. Rankings independently select a subset of the project's player pool via **ranking_players** and control event inclusion via **ranking_events**."

Update **API Overview** table: replace project-scoped stat/H2H/tournament routes with ranking-scoped ones; add rankings CRUD and ranking player membership rows.

- [ ] **Step 2: Update docs/routes.md**

Replace the project-level stats/h2h/ranking/tournaments rows with the new ranking-scoped routes. Add the rankings list and new ranking form. Keep project-level routes (players, import, settings) unchanged.

```
| `/projects/[id]` | Owner/member (published: guest via ranking) | Lists rankings; redirects to single ranking if only one |
| `/projects/[id]/rankings/new` | Owner/editor | Create a new ranking |
| `/projects/[id]/rankings/[rid]` | Owner/member (published: guest if ranking is published) | Ranking overview |
| `/projects/[id]/rankings/[rid]/ranking` | Owner/member (published: guest) | Players ordered by aggregate upset factor |
| `/projects/[id]/rankings/[rid]/stats` | Owner/member (published: guest) | Per-player win/loss lists |
| `/projects/[id]/rankings/[rid]/h2h` | Owner/member (published: guest) | Head-to-head matrix |
| `/projects/[id]/rankings/[rid]/tournaments` | Owner/member (published: guest) | Tournament list with per-ranking include/exclude; delete tournament |
| `/projects/[id]/rankings/[rid]/(editor)/players` | Owner/editor | Manage which project players are in this ranking |
```

Remove the old `/projects/[id]/stats`, `/projects/[id]/h2h`, `/projects/[id]/ranking`, `/projects/[id]/tournaments` rows.

- [ ] **Step 3: Update docs/modules.md**

Add `src/routes/rankings.rs` entry to the `api` crate table:

```
| `src/routes/rankings.rs` | Rankings CRUD; ranking player membership (add/remove/reorder/notes); `require_ranking_access` and `require_ranking_read_access` helpers |
```

Update `src/routes/projects.rs` description to note it no longer holds the `published` flag.

Update `src/routes/tournaments.rs` to note handlers are now ranking-scoped.

- [ ] **Step 4: Update backend/openapi.yaml**

The openapi.yaml is the full REST API contract. Update it to:
- Remove `published` from the project schema
- Add the `Ranking` and `RankingPlayer` schemas
- Replace all project-scoped stat/H2H/tournament paths with ranking-scoped paths
- Add ranking CRUD paths
- Add ranking player membership paths
- Add `DELETE /projects/{id}/tournaments/{tid}` path

- [ ] **Step 5: Commit**

```bash
git add docs/DESIGN.md docs/routes.md docs/modules.md backend/openapi.yaml
git commit -m "docs: update DESIGN, routes, modules, openapi for multi-ranking projects"
```

---

## Self-Review

All spec sections are covered:

| Spec requirement | Task |
|---|---|
| `ranking_projects` → `projects` rename | Task 1, 3 |
| `rankings` table with name, description, published | Task 1, 4 |
| `ranking_players` with rank_position and notes | Task 1, 4 |
| `ranking_events` replaces `project_events` | Task 1, 6 |
| `published` moves from project to ranking | Task 1, 3, 4 |
| `rank_position` moves from players to ranking_players | Task 1, 5 |
| Rankings CRUD API | Task 4 |
| Ranking player membership API | Task 4 |
| Stats/H2H/tournaments scoped to ranking | Task 6 |
| Tournament deletion | Task 6 (Step 8) |
| Worker writes ranking_events on import | Task 7 |
| Migration consolidation | Task 1 |
| New ranking backfills events on creation | Task 4 (Step 3) |
| Frontend rankings list | Task 11 |
| Frontend ranking layout | Task 12 |
| Frontend ranking player editor | Task 13 |
| All documentation updated | Task 14 |
