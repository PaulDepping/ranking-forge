# Mirror-Backed Rankings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all project-scoped tournament data and per-user start.gg API keys with rankings that read entirely from the `global_*` crawler mirror tables.

**Architecture:** Single migration rewrites `001_initial.sql` in place (no prod DB); project-scoped `tournaments`/`events`/`phases`/`phase_groups`/`entrants`/`sets` are dropped; a new `project_events` join table links projects to `global_events`; `ranking_events` and `ranking_set_results` FK directly into the global tables. Worker import/compute become pure Postgres-to-Postgres. All `StartggClient` usage is removed from the API and worker.

**Tech Stack:** Rust (sqlx 0.8.6, axum 0.8, tokio), SvelteKit/TypeScript, PostgreSQL 18

## Global Constraints

- All `sqlx::query!` macros require `bash backend/prepare-sqlx.sh` after any change — run it ONCE after all backend query changes are done (Tasks 1–9), not between every task
- `SQLX_OFFLINE=true` must work for CI builds; `.sqlx/` is committed
- Tests use `#[sqlx::test(migrations = "../../migrations")]` — no DB mocks
- `cargo add` only — never edit version numbers in `Cargo.toml` manually
- Run `cd backend && cargo fmt --all` before every commit
- Never call start.gg network from the API or worker — only the crawler binary may do so

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `backend/migrations/001_initial.sql` | Rewrite | Drop old tables, reorder FKs, enrich global tables, add `project_events` |
| `backend/crates/crawler/src/api.rs` | Modify | Add image/venue/hashtag/short_slug fields to `TOURNAMENT_QUERY` |
| `backend/crates/crawler/src/api_types.rs` | Modify | Add `TournamentImage`, update `TournamentNode` |
| `backend/crates/crawler/src/db.rs` | Modify | Update `upsert_tournament` + `upsert_player_full` for new columns |
| `backend/crates/common/src/models/mod.rs` | Modify | Remove `startgg_api_key` from `User` struct |
| `backend/crates/worker/src/main.rs` | Modify | Remove API key lookup before import dispatch |
| `backend/crates/worker/src/import.rs` | Rewrite | Pure DB import: global tables → `project_events` → `ranking_events` |
| `backend/crates/worker/src/compute.rs` | Modify | Phase 1 + Phase 2 queries rewritten to join through global tables |
| `backend/crates/api/src/state.rs` | Modify | Remove `startgg_base_url` field |
| `backend/crates/api/src/main.rs` | Modify | Remove `startgg_base_url` from `AppState` init |
| `backend/crates/api/src/routes/auth.rs` | Modify | Remove `startgg_api_key` from all queries/response |
| `backend/crates/api/src/routes/account.rs` | Modify | Remove `set_startgg_key`/`delete_startgg_key` handlers + tests |
| `backend/crates/api/src/routes/projects.rs` | Modify | Remove API key gate + `owner_has_startgg_key` + `with_api_key` test helper |
| `backend/crates/api/src/routes/games.rs` | Modify | `search_games` queries `global_games` instead of StartggClient |
| `backend/crates/api/src/routes/players.rs` | Modify | `link_account`/`add_players_by_handles`/`list_tournament_entrants` use global tables |
| `backend/crates/api/src/routes/import.rs` | Modify | Remove API key check; add `retrigger_import` handler |
| `backend/crates/api/src/routes/tournaments.rs` | Rewrite | All handlers join through `global_*` tables |
| `backend/crates/api/src/routes/invite_links.rs` | Modify | Remove `with_api_key` test helper + calls |
| `backend/crates/api/src/routes/rankings.rs` | Modify | Remove `with_api_key` test helper + calls |
| `backend/crates/api/src/routes/members.rs` | Modify | Remove `with_api_key` test helper + calls |
| `backend/crates/e2e/Cargo.toml` | Modify | Remove `wiremock` dev-dependency |
| `backend/crates/e2e/tests/full_flow.rs` | Rewrite | Replace wiremock + API key helpers with `seed_global_data` |
| `backend/crates/e2e/tests/import_live.rs` | Delete | Entire file — live start.gg import no longer exists |
| `backend/crates/topology/Cargo.toml` | Modify | Add `sqlx` dev-dependency |
| `backend/crates/topology/tests/smoke.rs` | Rewrite | Remove API key step; add DB seeding |
| `web/src/lib/types.ts` | Modify | Remove `has_startgg_key`, `owner_has_startgg_key` |
| `web/src/app.d.ts` | Modify | Remove `has_startgg_key` from `locals.user` |
| `web/src/routes/account/+page.svelte` | Modify | Remove API key section |
| `web/src/routes/account/+page.server.ts` | Modify | Remove `set-startgg-key`/`delete-startgg-key` form actions |
| `web/src/routes/projects/new/+page.server.ts` | Modify | Remove `hasStartggKey` gate |
| `web/src/routes/projects/[id]/(hub)/(editor)/import/+page.svelte` | Modify | Remove key warning; add Re-run button |
| `backend/openapi.yaml` | Modify | Remove key endpoints; add retrigger; update schemas |
| `docs/modules.md` | Modify | Remove `StartggClient` from api/worker module map |

---

### Task 1: Schema migration

**Files:**
- Rewrite: `backend/migrations/001_initial.sql`

**Interfaces:**
- Produces: New schema with `project_events`, enriched `global_tournaments`/`global_players`, and `ranking_events`/`ranking_set_results` referencing global tables. No `tournaments`, `events`, `phases`, `phase_groups`, `entrants`, `sets`.

**Note:** Do NOT run `prepare-sqlx.sh` yet — subsequent tasks change Rust query code that references the old tables. Run prepare-sqlx after Task 9.

- [ ] **Step 1: Rewrite `backend/migrations/001_initial.sql`**

Replace the entire file with the following. Key structural changes vs the old file: `users` loses `startgg_api_key`; the six project-scoped tournament tables are gone; `ranking_events` and `ranking_set_results` are moved to after the global table block and reference global FKs; `project_events` is added; `global_tournaments` gains six columns; `global_players` gains `banner_url`.

```sql
-- Enums
CREATE TYPE job_kind AS ENUM ('import_tournaments', 'compute_ranking');
CREATE TYPE job_status AS ENUM ('pending', 'running', 'done', 'failed');

-- Users and sessions
CREATE TABLE users (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT        NOT NULL UNIQUE,
    display_name    TEXT        NOT NULL,
    password_hash   TEXT        NOT NULL,
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

-- Projects
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

-- Rankings
CREATE TABLE rankings (
    id                       UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id               UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name                     TEXT        NOT NULL,
    description              TEXT,
    published                BOOLEAN     NOT NULL DEFAULT FALSE,
    algorithm                TEXT,
    algorithm_config         JSONB       NOT NULL DEFAULT '{}',
    include_external_results BOOLEAN     NOT NULL DEFAULT FALSE,
    result_sort              TEXT        NOT NULL DEFAULT 'upset_factor',
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX rankings_project_id_idx ON rankings(project_id);

-- Players
CREATE TABLE players (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id    UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX players_project_id_idx ON players(project_id);

CREATE TABLE ranking_players (
    ranking_id    UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    player_id     UUID    NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    rank_position INTEGER NOT NULL DEFAULT 0,
    notes         TEXT,
    PRIMARY KEY (ranking_id, player_id)
);

CREATE INDEX ranking_players_player_id_idx ON ranking_players(player_id);

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

-- Per-player algorithm scores
CREATE TABLE ranking_player_scores (
    ranking_id      UUID        NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    player_id       UUID        NOT NULL REFERENCES players(id)  ON DELETE CASCADE,
    computed_rating FLOAT       NOT NULL,
    display_data    JSONB       NOT NULL DEFAULT '{}',
    algorithm_state JSONB       NOT NULL DEFAULT '{}',
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (ranking_id, player_id)
);

CREATE INDEX ranking_player_scores_player_id_idx ON ranking_player_scores(player_id);

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

-- ============================================================
-- Global mirror tables (start.gg crawler)
-- ============================================================

CREATE TABLE global_games (
    id         UUID   PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id BIGINT NOT NULL UNIQUE,
    name       TEXT   NOT NULL
);
CREATE INDEX global_games_startgg_id_idx ON global_games(startgg_id);

CREATE TABLE global_players (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_user_id     BIGINT      UNIQUE,
    startgg_player_id   BIGINT      UNIQUE,
    handle              TEXT        NOT NULL,
    display_name        TEXT,
    profile_image_url   TEXT,
    banner_url          TEXT,
    startgg_slug        TEXT,
    bio                 TEXT,
    pronouns            TEXT,
    location_city       TEXT,
    location_state      TEXT,
    location_country    TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_players_startgg_user_id_idx   ON global_players(startgg_user_id);
CREATE INDEX global_players_startgg_player_id_idx ON global_players(startgg_player_id);

CREATE TABLE global_tournaments (
    id                UUID             PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id        BIGINT           NOT NULL UNIQUE,
    name              TEXT             NOT NULL,
    slug              TEXT             NOT NULL,
    short_slug        TEXT,
    start_at          TIMESTAMPTZ,
    end_at            TIMESTAMPTZ,
    country_code      TEXT,
    city              TEXT,
    addr_state        TEXT,
    venue_name        TEXT,
    venue_address     TEXT,
    online            BOOLEAN,
    num_attendees     INTEGER,
    lat               DOUBLE PRECISION,
    lng               DOUBLE PRECISION,
    timezone          TEXT,
    hashtag           TEXT,
    profile_image_url TEXT,
    banner_url        TEXT,
    created_at        TIMESTAMPTZ      NOT NULL DEFAULT NOW()
);
CREATE INDEX global_tournaments_start_at_idx   ON global_tournaments(start_at);
CREATE INDEX global_tournaments_startgg_id_idx ON global_tournaments(startgg_id);

CREATE TABLE global_events (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id       BIGINT      NOT NULL UNIQUE,
    tournament_id    UUID        NOT NULL REFERENCES global_tournaments(id) ON DELETE CASCADE,
    game_id          UUID        REFERENCES global_games(id),
    name             TEXT        NOT NULL,
    slug             TEXT,
    start_at         TIMESTAMPTZ,
    num_entrants     INTEGER,
    is_online        BOOLEAN,
    competition_tier INTEGER,
    state            TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_events_tournament_id_idx    ON global_events(tournament_id);
CREATE INDEX global_events_game_id_start_at_idx ON global_events(game_id, start_at);
CREATE INDEX global_events_startgg_id_idx       ON global_events(startgg_id);

CREATE TABLE global_phases (
    id            UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT  NOT NULL UNIQUE,
    event_id      UUID    NOT NULL REFERENCES global_events(id) ON DELETE CASCADE,
    name          TEXT,
    phase_order   INTEGER,
    bracket_type  TEXT,
    is_exhibition BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX global_phases_event_id_idx   ON global_phases(event_id);
CREATE INDEX global_phases_startgg_id_idx ON global_phases(startgg_id);

CREATE TABLE global_phase_groups (
    id                 UUID   PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id         BIGINT NOT NULL UNIQUE,
    phase_id           UUID   NOT NULL REFERENCES global_phases(id) ON DELETE CASCADE,
    display_identifier TEXT,
    bracket_type       TEXT
);
CREATE INDEX global_phase_groups_phase_id_idx   ON global_phase_groups(phase_id);
CREATE INDEX global_phase_groups_startgg_id_idx ON global_phase_groups(startgg_id);

CREATE TABLE global_event_entries (
    id        UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id  UUID    NOT NULL REFERENCES global_events(id) ON DELETE CASCADE,
    player_id UUID    NOT NULL REFERENCES global_players(id) ON DELETE CASCADE,
    seed      INTEGER,
    placement INTEGER,
    UNIQUE (event_id, player_id)
);
CREATE INDEX global_event_entries_player_id_idx ON global_event_entries(player_id);
CREATE INDEX global_event_entries_event_id_idx  ON global_event_entries(event_id);

CREATE TABLE global_sets (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id       BIGINT      NOT NULL UNIQUE,
    event_id         UUID        NOT NULL REFERENCES global_events(id) ON DELETE CASCADE,
    phase_group_id   UUID        REFERENCES global_phase_groups(id),
    winner_player_id UUID        REFERENCES global_players(id),
    loser_player_id  UUID        REFERENCES global_players(id),
    round            INTEGER,
    round_name       TEXT,
    winner_score     SMALLINT,
    loser_score      SMALLINT,
    is_dq            BOOLEAN     NOT NULL DEFAULT FALSE,
    vod_url          TEXT,
    completed_at     TIMESTAMPTZ,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_sets_event_id_idx          ON global_sets(event_id);
CREATE INDEX global_sets_phase_group_id_idx    ON global_sets(phase_group_id);
CREATE INDEX global_sets_winner_player_id_idx  ON global_sets(winner_player_id);
CREATE INDEX global_sets_loser_player_id_idx   ON global_sets(loser_player_id);
CREATE INDEX global_sets_completed_at_idx      ON global_sets(completed_at);
CREATE INDEX global_sets_startgg_id_idx        ON global_sets(startgg_id);

CREATE TABLE global_set_games (
    id               UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    set_id           UUID    NOT NULL REFERENCES global_sets(id) ON DELETE CASCADE,
    order_num        INTEGER NOT NULL,
    winner_player_id UUID    REFERENCES global_players(id),
    stage_id         BIGINT,
    stage_name       TEXT,
    UNIQUE (set_id, order_num)
);
CREATE INDEX global_set_games_set_id_idx ON global_set_games(set_id);

CREATE TABLE global_game_selections (
    game_id        UUID   NOT NULL REFERENCES global_set_games(id) ON DELETE CASCADE,
    player_id      UUID   REFERENCES global_players(id),
    selection_type TEXT   NOT NULL,
    character_id   BIGINT,
    character_name TEXT,
    PRIMARY KEY (game_id, player_id, selection_type)
);
CREATE INDEX global_game_selections_player_id_idx ON global_game_selections(player_id);

CREATE TABLE global_player_ratings (
    player_id       UUID        NOT NULL REFERENCES global_players(id) ON DELETE CASCADE,
    game_id         UUID        NOT NULL REFERENCES global_games(id),
    computed_rating FLOAT       NOT NULL,
    display_data    JSONB       NOT NULL DEFAULT '{}',
    algorithm_state JSONB       NOT NULL DEFAULT '{}',
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (player_id, game_id)
);
CREATE INDEX global_player_ratings_game_id_idx ON global_player_ratings(game_id);

CREATE TABLE crawler_checkpoints (
    key        TEXT        PRIMARY KEY,
    value      JSONB       NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================
-- Project → global bridge tables (depend on global_events/sets)
-- ============================================================

CREATE TABLE project_events (
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    global_event_id UUID NOT NULL REFERENCES global_events(id),
    PRIMARY KEY (project_id, global_event_id)
);
CREATE INDEX project_events_project_id_idx ON project_events(project_id);

CREATE TABLE ranking_events (
    ranking_id      UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    global_event_id UUID    NOT NULL REFERENCES global_events(id),
    included        BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (ranking_id, global_event_id)
);
CREATE INDEX ranking_events_event_id_idx ON ranking_events(global_event_id);

CREATE TABLE ranking_set_results (
    ranking_id       UUID        NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    global_set_id    UUID        NOT NULL REFERENCES global_sets(id) ON DELETE CASCADE,
    winner_player_id UUID        NOT NULL REFERENCES players(id),
    loser_player_id  UUID        NOT NULL REFERENCES players(id),
    global_event_id  UUID        NOT NULL REFERENCES global_events(id),
    upset_factor     FLOAT,
    completed_at     TIMESTAMPTZ,
    PRIMARY KEY (ranking_id, global_set_id)
);
CREATE INDEX ranking_set_results_winner_idx ON ranking_set_results(ranking_id, winner_player_id);
CREATE INDEX ranking_set_results_loser_idx  ON ranking_set_results(ranking_id, loser_player_id);
```

- [ ] **Step 2: Commit (migration only — code comes next)**

```bash
git add backend/migrations/001_initial.sql
git commit -m "feat(schema): drop project-scoped tables, add project_events, enrich global_*"
```

---

### Task 2: Crawler enrichment

**Files:**
- Modify: `backend/crates/crawler/src/api.rs`
- Modify: `backend/crates/crawler/src/api_types.rs`
- Modify: `backend/crates/crawler/src/db.rs`

**Interfaces:**
- Consumes: `global_tournaments.profile_image_url`, `banner_url`, `venue_name`, `venue_address`, `hashtag`, `short_slug` (from Task 1 schema); `global_players.banner_url`
- Produces: `upsert_tournament(pool, node) -> Result<Uuid>` (same signature, more columns written); `upsert_player_full(..., banner_url: Option<&str>, ...) -> Result<Uuid>` (adds one param)

- [ ] **Step 1: Add image/venue fields to `TOURNAMENT_QUERY` in `api.rs`**

In the `TOURNAMENT_QUERY` constant, replace the current node field list:
```
id name slug startAt endAt countryCode city addrState
numAttendees isOnline lat lng timezone
```
with:
```
id name slug shortSlug startAt endAt countryCode city addrState
numAttendees isOnline lat lng timezone hashtag venueName venueAddress
images { url type }
```

Full updated constant:
```rust
pub const TOURNAMENT_QUERY: &str = r#"
query Tournaments($page: Int!, $perPage: Int!, $filter: TournamentPageFilter) {
  tournaments(query: {
    page: $page
    perPage: $perPage
    filter: $filter
  }) {
    pageInfo { total totalPages }
    nodes {
      id name slug shortSlug startAt endAt countryCode city addrState
      numAttendees isOnline lat lng timezone hashtag venueName venueAddress
      images { url type }
      events {
        id name slug startAt state isOnline numEntrants type competitionTier
        videogame { id name }
      }
    }
  }
}
"#;
```

- [ ] **Step 2: Add `TournamentImage` struct and update `TournamentNode` in `api_types.rs`**

Add after the existing `UserImage` struct (around line 334):
```rust
#[derive(Debug, Deserialize)]
pub struct TournamentImage {
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub image_type: Option<String>,
}
```

Update `TournamentNode` to add new fields:
```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub short_slug: Option<String>,
    pub start_at: Option<i64>,
    pub end_at: Option<i64>,
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub addr_state: Option<String>,
    pub num_attendees: Option<i64>,
    pub is_online: Option<bool>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub timezone: Option<String>,
    pub hashtag: Option<String>,
    pub venue_name: Option<String>,
    pub venue_address: Option<String>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub images: Vec<TournamentImage>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub events: Vec<EventNode>,
}
```

- [ ] **Step 3: Update `upsert_tournament` in `db.rs`**

Replace the entire `upsert_tournament` function:
```rust
pub async fn upsert_tournament(pool: &PgPool, node: &TournamentNode) -> Result<Uuid> {
    let start_at = node.start_at.and_then(|ts| DateTime::from_timestamp(ts, 0));
    let end_at = node.end_at.and_then(|ts| DateTime::from_timestamp(ts, 0));

    let profile_image_url = node
        .images
        .iter()
        .find(|img| img.image_type.as_deref() == Some("profile"))
        .or_else(|| node.images.first())
        .and_then(|img| img.url.as_deref());

    let banner_url = node
        .images
        .iter()
        .find(|img| img.image_type.as_deref() == Some("banner"))
        .and_then(|img| img.url.as_deref());

    let row = sqlx::query!(
        r#"
        INSERT INTO global_tournaments
            (startgg_id, name, slug, short_slug, start_at, end_at, country_code, city,
             addr_state, online, num_attendees, lat, lng, timezone,
             hashtag, venue_name, venue_address, profile_image_url, banner_url)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
        ON CONFLICT (startgg_id) DO UPDATE SET
            name              = EXCLUDED.name,
            slug              = EXCLUDED.slug,
            short_slug        = COALESCE(EXCLUDED.short_slug,        global_tournaments.short_slug),
            start_at          = EXCLUDED.start_at,
            end_at            = EXCLUDED.end_at,
            country_code      = EXCLUDED.country_code,
            city              = EXCLUDED.city,
            addr_state        = EXCLUDED.addr_state,
            online            = EXCLUDED.online,
            num_attendees     = EXCLUDED.num_attendees,
            lat               = EXCLUDED.lat,
            lng               = EXCLUDED.lng,
            timezone          = EXCLUDED.timezone,
            hashtag           = COALESCE(EXCLUDED.hashtag,           global_tournaments.hashtag),
            venue_name        = COALESCE(EXCLUDED.venue_name,        global_tournaments.venue_name),
            venue_address     = COALESCE(EXCLUDED.venue_address,     global_tournaments.venue_address),
            profile_image_url = COALESCE(EXCLUDED.profile_image_url, global_tournaments.profile_image_url),
            banner_url        = COALESCE(EXCLUDED.banner_url,        global_tournaments.banner_url)
        RETURNING id
        "#,
        node.id,
        node.name,
        node.slug,
        node.short_slug,
        start_at,
        end_at,
        node.country_code,
        node.city,
        node.addr_state,
        node.is_online,
        node.num_attendees.map(|n| n as i32),
        node.lat,
        node.lng,
        node.timezone,
        node.hashtag,
        node.venue_name,
        node.venue_address,
        profile_image_url,
        banner_url,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}
```

- [ ] **Step 4: Add `banner_url` to `upsert_player_full` in `db.rs`**

Add `banner_url: Option<&str>` as a new parameter after `profile_image_url`. Update the INSERT and ON CONFLICT SET to include:
```rust
pub async fn upsert_player_full(
    pool: &PgPool,
    startgg_user_id: i64,
    startgg_player_id: Option<i64>,
    handle: &str,
    display_name: Option<&str>,
    profile_image_url: Option<&str>,
    banner_url: Option<&str>,        // new
    startgg_slug: Option<&str>,
    bio: Option<&str>,
    pronouns: Option<&str>,
    location_city: Option<&str>,
    location_state: Option<&str>,
    location_country: Option<&str>,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_players
            (startgg_user_id, startgg_player_id, handle, display_name,
             profile_image_url, banner_url, startgg_slug, bio, pronouns,
             location_city, location_state, location_country)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (startgg_user_id) DO UPDATE SET
            handle            = EXCLUDED.handle,
            display_name      = EXCLUDED.display_name,
            startgg_player_id = COALESCE(EXCLUDED.startgg_player_id, global_players.startgg_player_id),
            profile_image_url = COALESCE(EXCLUDED.profile_image_url, global_players.profile_image_url),
            banner_url        = COALESCE(EXCLUDED.banner_url,        global_players.banner_url),
            startgg_slug      = COALESCE(EXCLUDED.startgg_slug,      global_players.startgg_slug),
            bio               = COALESCE(EXCLUDED.bio,               global_players.bio),
            pronouns          = COALESCE(EXCLUDED.pronouns,          global_players.pronouns),
            location_city     = EXCLUDED.location_city,
            location_state    = EXCLUDED.location_state,
            location_country  = EXCLUDED.location_country,
            updated_at        = NOW()
        RETURNING id
        "#,
        startgg_user_id,
        startgg_player_id,
        handle,
        display_name,
        profile_image_url,
        banner_url,
        startgg_slug,
        bio,
        pronouns,
        location_city,
        location_state,
        location_country,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}
```

- [ ] **Step 5: Update the `upsert_player_full` call site in `scraper.rs`**

In `fetch_full_path`, the call to `upsert_player_full` gains a `banner_url` argument after `image_url`. Add it:
```rust
let banner_url = user
    .images
    .iter()
    .find(|img| img.image_type.as_deref() == Some("banner"))
    .and_then(|img| img.url.as_deref());

upsert_player_full(
    pool,
    user.id,
    player_id,
    participant.player.as_ref().and_then(|p| p.gamer_tag.as_deref()).unwrap_or("Unknown"),
    user.name.as_deref(),
    image_url,
    banner_url,   // new
    user.slug.as_deref(),
    user.bio.as_deref(),
    user.gender_pronoun.as_deref(),
    loc.and_then(|l| l.city.as_deref()),
    loc.and_then(|l| l.state.as_deref()),
    loc.and_then(|l| l.country.as_deref()),
)
.await?
```

- [ ] **Step 6: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/crawler/src/
git commit -m "feat(crawler): fetch tournament images, venue, hashtag, short_slug; add player banner_url"
```

---

### Task 3: Worker rewrite

**Files:**
- Modify: `backend/crates/worker/src/main.rs`
- Rewrite: `backend/crates/worker/src/import.rs`
- Modify: `backend/crates/worker/src/compute.rs`

**Interfaces:**
- Consumes: `project_events`, `ranking_events`, `global_sets`, `global_players`, `global_event_entries`, `startgg_accounts` (Task 1 schema)
- Produces: `import::run(pool, project_id, job_id, params) -> anyhow::Result<()>` (removes `startgg: &StartggClient` param)

- [ ] **Step 1: Update `main.rs` to remove API key lookup**

In the `"import_tournaments"` match arm, remove the entire `api_key` lookup block and the `let startgg = ...` line. Replace with a direct tokio::spawn:

```rust
"import_tournaments" => {
    let import_params = common::jobs::ImportParams::from_job(&job);
    tracing::info!(%job_id, %project_id, "starting import");
    tokio::spawn(async move {
        match import::run(&pool2, project_id, job_id, import_params).await {
            Ok(()) => {
                tracing::info!(%job_id, "import complete");
                if let Err(e) = common::jobs::mark_done(&pool2, job_id).await {
                    tracing::error!(%e, %job_id, "failed to mark job done");
                }
            }
            Err(e) => {
                tracing::error!(%e, %job_id, "import failed");
                let _ = common::jobs::mark_failed(&pool2, job_id, &e.to_string()).await;
            }
        }
    })
}
```

Also remove `use common::startgg::StartggClient;` from the imports if it's present in `main.rs`.

- [ ] **Step 2: Rewrite `import.rs`**

Replace the entire file:

```rust
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use common::jobs::{ImportParams, update_progress};

#[instrument(skip(pool), fields(%project_id))]
pub async fn run(
    pool: &PgPool,
    project_id: Uuid,
    job_id: Uuid,
    params: ImportParams,
) -> anyhow::Result<()> {
    let project = sqlx::query!(
        "SELECT game_id FROM projects WHERE id = $1",
        project_id,
    )
    .fetch_one(pool)
    .await?;

    // Resolve project players → global_players.id via startgg_accounts
    let account_rows = sqlx::query!(
        r#"
        SELECT gp.id AS global_player_id
        FROM startgg_accounts sa
        JOIN players p ON p.id = sa.player_id
        JOIN global_players gp ON gp.startgg_user_id = sa.startgg_user_id
        WHERE p.project_id = $1
        "#,
        project_id,
    )
    .fetch_all(pool)
    .await?;

    if account_rows.is_empty() {
        tracing::info!(%project_id, "no linked accounts with global player entries, nothing to import");
        return Ok(());
    }

    let global_player_ids: Vec<Uuid> = account_rows.into_iter().map(|r| r.global_player_id).collect();

    // Discover events via global_event_entries
    let event_rows = sqlx::query!(
        r#"
        SELECT DISTINCT ge.id AS global_event_id
        FROM global_event_entries gee
        JOIN global_events ge ON ge.id = gee.event_id
        JOIN global_tournaments gt ON gt.id = ge.tournament_id
        LEFT JOIN global_games gg ON gg.id = ge.game_id
        WHERE gee.player_id = ANY($1)
          AND ($2::BIGINT IS NULL OR gg.startgg_id = $2)
          AND ($3::BIGINT IS NULL OR EXTRACT(EPOCH FROM gt.start_at) >= $3)
          AND ($4::BIGINT IS NULL OR EXTRACT(EPOCH FROM gt.start_at) <= $4)
        "#,
        &global_player_ids,
        project.game_id,
        params.after_date.map(|t| t as i64),
        params.before_date.map(|t| t as i64),
    )
    .fetch_all(pool)
    .await?;

    tracing::info!(event_count = event_rows.len(), "discovered events from global mirror");
    update_progress(pool, job_id, "importing", 0, event_rows.len() as u32).await?;

    // Check whether this is the first import for the project
    let is_first_import = sqlx::query_scalar!(
        "SELECT NOT EXISTS (SELECT 1 FROM ranking_events re JOIN rankings r ON r.id = re.ranking_id WHERE r.project_id = $1)",
        project_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(true);

    let mut tx = pool.begin().await?;

    for (i, row) in event_rows.iter().enumerate() {
        // Upsert into project_events
        sqlx::query!(
            "INSERT INTO project_events (project_id, global_event_id)
             VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
            project_id,
            row.global_event_id,
        )
        .execute(&mut *tx)
        .await?;

        // Upsert into ranking_events for every ranking in this project
        sqlx::query!(
            r#"
            INSERT INTO ranking_events (ranking_id, global_event_id, included)
            SELECT r.id, $2, true
            FROM rankings r
            WHERE r.project_id = $1
            ON CONFLICT DO NOTHING
            "#,
            project_id,
            row.global_event_id,
        )
        .execute(&mut *tx)
        .await?;

        update_progress(pool, job_id, "importing", (i + 1) as u32, event_rows.len() as u32).await?;
    }

    tx.commit().await?;

    if is_first_import && !event_rows.is_empty() {
        seed_ranking_by_winrate(pool, project_id).await?;
        tracing::info!(%project_id, "initial ranking seeded by winrate");
    }

    // Enqueue compute_ranking for all project rankings
    let ranking_ids: Vec<Uuid> = sqlx::query_scalar!(
        "SELECT id FROM rankings WHERE project_id = $1",
        project_id,
    )
    .fetch_all(pool)
    .await?;

    for rid in ranking_ids {
        if let Err(e) = common::jobs::enqueue_compute_ranking(pool, project_id, rid).await {
            tracing::warn!(%e, %rid, "failed to enqueue compute_ranking after import");
        }
    }

    Ok(())
}

async fn seed_ranking_by_winrate(pool: &PgPool, project_id: Uuid) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        WITH player_global AS (
            SELECT DISTINCT ON (p.id)
                p.id          AS player_id,
                p.created_at,
                gp.id         AS global_player_id
            FROM players p
            JOIN startgg_accounts sa ON sa.player_id = p.id
            JOIN global_players gp   ON gp.startgg_user_id = sa.startgg_user_id
            WHERE p.project_id = $1
            ORDER BY p.id, sa.created_at
        ),
        no_link AS (
            SELECT p.id AS player_id, p.created_at, NULL::UUID AS global_player_id
            FROM players p
            WHERE p.project_id = $1
              AND NOT EXISTS (SELECT 1 FROM startgg_accounts sa WHERE sa.player_id = p.id)
        ),
        all_players AS (SELECT * FROM player_global UNION ALL SELECT * FROM no_link),
        stats AS (
            SELECT
                ap.player_id,
                ap.created_at,
                COUNT(gs.id) FILTER (WHERE gs.winner_player_id = ap.global_player_id) AS wins,
                COUNT(gs.id) AS total
            FROM all_players ap
            LEFT JOIN global_sets gs
                ON ap.global_player_id IS NOT NULL
               AND (gs.winner_player_id = ap.global_player_id OR gs.loser_player_id = ap.global_player_id)
            GROUP BY ap.player_id, ap.created_at
        ),
        ranked AS (
            SELECT
                player_id,
                ROW_NUMBER() OVER (
                    ORDER BY
                        CASE WHEN total > 0 THEN wins::float8 / total ELSE 0 END DESC,
                        created_at ASC
                ) AS new_rank
            FROM stats
        )
        UPDATE ranking_players
        SET rank_position = ranked.new_rank::int4
        FROM ranked
        WHERE ranking_players.player_id = ranked.player_id
          AND ranking_players.ranking_id IN (SELECT id FROM rankings WHERE project_id = $1)
        "#,
        project_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 3: Rewrite `compute.rs` Phase 1**

Replace the `phase1_set_results` function (keep the `SetRow` struct and the rest of the function unchanged except the query):

```rust
async fn phase1_set_results(pool: &PgPool, ranking_id: Uuid) -> anyhow::Result<()> {
    struct SetRow {
        global_set_id: Uuid,
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        global_event_id: Uuid,
        winner_seed: Option<i32>,
        loser_seed: Option<i32>,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let sets = sqlx::query_as!(
        SetRow,
        r#"
        SELECT
            gs.id         AS global_set_id,
            saw.player_id AS "winner_player_id!: Uuid",
            sal.player_id AS "loser_player_id!: Uuid",
            gs.event_id   AS global_event_id,
            wee.seed      AS winner_seed,
            lee.seed      AS loser_seed,
            gs.completed_at
        FROM global_sets gs
        JOIN global_players gwp ON gwp.id = gs.winner_player_id
        JOIN global_players glp ON glp.id = gs.loser_player_id
        JOIN startgg_accounts saw ON saw.startgg_user_id = gwp.startgg_user_id
        JOIN startgg_accounts sal ON sal.startgg_user_id = glp.startgg_user_id
        JOIN ranking_players rwp ON rwp.player_id = saw.player_id AND rwp.ranking_id = $1
        JOIN ranking_players rlp ON rlp.player_id = sal.player_id AND rlp.ranking_id = $1
        JOIN ranking_events re ON re.global_event_id = gs.event_id AND re.ranking_id = $1
        LEFT JOIN global_event_entries wee ON wee.event_id = gs.event_id AND wee.player_id = gwp.id
        LEFT JOIN global_event_entries lee ON lee.event_id = gs.event_id AND lee.player_id = glp.id
        WHERE re.included = true
          AND gs.is_dq    = false
        ORDER BY gs.completed_at ASC NULLS LAST
        "#,
        ranking_id,
    )
    .fetch_all(pool)
    .await?;

    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM ranking_set_results WHERE ranking_id = $1",
        ranking_id,
    )
    .execute(&mut *tx)
    .await?;

    for row in &sets {
        let upset_factor = set_upset_factor(row.winner_seed, row.loser_seed);
        sqlx::query!(
            "INSERT INTO ranking_set_results
                 (ranking_id, global_set_id, winner_player_id, loser_player_id, global_event_id, upset_factor, completed_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (ranking_id, global_set_id) DO UPDATE SET
                 winner_player_id = EXCLUDED.winner_player_id,
                 loser_player_id  = EXCLUDED.loser_player_id,
                 global_event_id  = EXCLUDED.global_event_id,
                 upset_factor     = EXCLUDED.upset_factor,
                 completed_at     = EXCLUDED.completed_at",
            ranking_id,
            row.global_set_id,
            row.winner_player_id,
            row.loser_player_id,
            row.global_event_id,
            upset_factor,
            row.completed_at,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    tracing::info!(%ranking_id, count = sets.len(), "phase1: wrote ranking_set_results");
    Ok(())
}
```

- [ ] **Step 4: Rewrite `compute.rs` Phase 2**

Replace the `phase2_algorithm_scores` inner query. The outer function structure stays the same; only the `sqlx::query_as!` block changes:

```rust
let sets = sqlx::query_as!(
    AlgoSetRow,
    r#"
    SELECT
        saw.player_id AS "winner_player_id!: Uuid",
        sal.player_id AS "loser_player_id!: Uuid",
        gs.completed_at
    FROM global_sets gs
    JOIN global_players gwp ON gwp.id = gs.winner_player_id
    JOIN global_players glp ON glp.id = gs.loser_player_id
    JOIN startgg_accounts saw ON saw.startgg_user_id = gwp.startgg_user_id
    JOIN startgg_accounts sal ON sal.startgg_user_id = glp.startgg_user_id
    JOIN ranking_players rwp ON rwp.player_id = saw.player_id AND rwp.ranking_id = $1
    JOIN ranking_players rlp ON rlp.player_id = sal.player_id AND rlp.ranking_id = $1
    JOIN ranking_events re ON re.global_event_id = gs.event_id AND re.ranking_id = $1
    WHERE re.included = true
      AND gs.is_dq    = false
    ORDER BY gs.completed_at ASC NULLS LAST
    "#,
    ranking_id,
)
.fetch_all(pool)
.await?;
```

- [ ] **Step 5: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/worker/src/
git commit -m "feat(worker): remove StartggClient from import; rewrite import+compute for global tables"
```

---

### Task 4: Common model + auth route cleanup

**Files:**
- Modify: `backend/crates/common/src/models/mod.rs`
- Modify: `backend/crates/api/src/routes/auth.rs`

**Interfaces:**
- Produces: `User` struct without `startgg_api_key`; `UserResponse` (in auth.rs) without `has_startgg_key`; all SELECT queries for users no longer include that column

- [ ] **Step 1: Remove `startgg_api_key` from `User` in `common/src/models/mod.rs`**

Remove the field `pub startgg_api_key: Option<String>,` from the `User` struct.

- [ ] **Step 2: Update all user queries in `auth.rs`**

Four queries reference `startgg_api_key`. Update each:

**`UserResponse` struct** — remove `has_startgg_key: bool` field entirely.

**`From<User> for UserResponse`** — remove `has_startgg_key: u.startgg_api_key.is_some(),` line.

**`login` handler SELECT** (around line 89):
```sql
SELECT u.id, u.email, u.display_name, u.password_hash, u.created_at
FROM users u JOIN sessions s ON s.user_id = u.id WHERE s.id = $1 AND s.expires_at > NOW()
```

**`get_me` handler SELECT** (around line 121):
```sql
SELECT id, email, display_name, password_hash, created_at FROM users WHERE id = $1
```

**`register` RETURNING clause** (around line 235):
```sql
RETURNING id, email, display_name, password_hash, created_at
```

**`login` by email SELECT** (around line 266):
```sql
SELECT id, email, display_name, password_hash, created_at FROM users WHERE email = $1
```

- [ ] **Step 3: Update the `AuthUser` extractor query if it selects `startgg_api_key`**

Search in `auth.rs` for the `FromRequestParts` impl. If the session-check query selects `startgg_api_key`, remove it (match the same pattern as above).

- [ ] **Step 4: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/common/src/models/mod.rs backend/crates/api/src/routes/auth.rs
git commit -m "feat(auth): remove startgg_api_key from User model and all auth queries"
```

---

### Task 5: AppState + account + games cleanup

**Files:**
- Modify: `backend/crates/api/src/state.rs`
- Modify: `backend/crates/api/src/main.rs`
- Modify: `backend/crates/api/src/routes/account.rs`
- Modify: `backend/crates/api/src/routes/games.rs`

**Interfaces:**
- Produces: `AppState { db: PgPool, cors_origin: String }` (no `startgg_base_url`); `GET /games/search` queries `global_games`; `PUT/DELETE /account/startgg-key` removed

- [ ] **Step 1: Remove `startgg_base_url` from `AppState`**

`backend/crates/api/src/state.rs` — remove the field:
```rust
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub cors_origin: String,
}
```

- [ ] **Step 2: Update `api/src/main.rs`**

Remove the `startgg_base_url` field from the `AppState { ... }` initialization block.

- [ ] **Step 3: Remove API key endpoints from `account.rs`**

Delete the `set_startgg_key` handler (lines ~143–165), `delete_startgg_key` handler (lines ~167–180), and their route registrations in `router()`. Also remove any `StartggClient` import.

- [ ] **Step 4: Clean up account tests**

In the `#[cfg(test)]` block in `account.rs`:
- Delete `test_set_startgg_key_valid_stores_key`
- Delete `test_set_startgg_key_invalid_returns_422`
- Delete `test_delete_startgg_key_clears_it`
- In `test_me_reflects_has_startgg_key`: rename to `test_me_response_has_no_startgg_key_field` and change the assertion to verify `has_startgg_key` is **not** present in the JSON response body (use `assert!(body.get("has_startgg_key").is_none())`)
- In `test_delete_account_cascades_projects`: remove the `UPDATE users SET startgg_api_key` line
- In all test `AppState` constructors: remove `startgg_base_url` field

- [ ] **Step 5: Rewrite `search_games` in `games.rs`**

Remove the `StartggClient` import. Replace the handler:
```rust
pub async fn search_games(
    State(state): State<AppState>,
    _user: AuthUser,
    Query(params): Query<SearchParams>,
) -> Result<impl IntoResponse> {
    let pattern = format!("%{}%", params.q);
    let games = sqlx::query!(
        "SELECT startgg_id AS id, name FROM global_games WHERE name ILIKE $1 ORDER BY name LIMIT 20",
        pattern,
    )
    .fetch_all(&state.db)
    .await?;

    #[derive(Serialize)]
    struct GameResult { id: i64, name: String }

    let results: Vec<GameResult> = games.into_iter()
        .map(|r| GameResult { id: r.id, name: r.name })
        .collect();
    Ok(Json(results))
}
```

- [ ] **Step 6: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/api/src/state.rs backend/crates/api/src/main.rs \
         backend/crates/api/src/routes/account.rs backend/crates/api/src/routes/games.rs
git commit -m "feat(api): remove startgg_base_url from AppState; remove API key endpoints; rewrite search_games"
```

---

### Task 6: API projects + import routes

**Files:**
- Modify: `backend/crates/api/src/routes/projects.rs`
- Modify: `backend/crates/api/src/routes/import.rs`

**Interfaces:**
- Produces: `POST /projects` no longer checks API key; project responses drop `owner_has_startgg_key`; new `POST /projects/:id/import/:job_id/retrigger` endpoint

- [ ] **Step 1: Remove API key gate from `create_project` in `projects.rs`**

Remove the `if user.startgg_api_key.is_none() { return Err(...) }` check and any `startgg_api_key` column in project response queries. Remove `owner_has_startgg_key` from `list_projects`, `get_project`, and `patch_project` SELECT subqueries and their response structs.

- [ ] **Step 2: Clean up projects tests**

- Delete `with_api_key` async helper function
- Delete `test_create_project_requires_startgg_key`
- In `test_get_project_includes_owner_has_startgg_key`: rename to `test_get_project_response_shape`, remove `with_api_key` calls, and assert `owner_has_startgg_key` is absent from the response JSON
- In all other tests: remove `with_api_key(&pool, ...)` calls (projects now create without a key)
- In all test `AppState` constructors: remove `startgg_base_url`

- [ ] **Step 3: Update `start_import` in `import.rs`**

Remove the `owner_key` query block:
```rust
// DELETE the following block entirely:
let owner_key: Option<String> = sqlx::query_scalar!(
    "SELECT u.startgg_api_key FROM projects rp
     JOIN users u ON u.id = rp.owner_id
     WHERE rp.id = $1",
    project_id,
)
.fetch_optional(&state.db)
.await?
.flatten();

if owner_key.is_none() {
    return Err(AppError::UnprocessableEntity(...));
}
```

- [ ] **Step 4: Add `retrigger_import` handler to `import.rs`**

```rust
pub async fn retrigger_import(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, job_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    let original = sqlx::query!(
        "SELECT params FROM jobs WHERE id = $1 AND project_id = $2",
        job_id,
        project_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let params: common::jobs::ImportParams = serde_json::from_value(original.params)
        .unwrap_or_default();
    let job = common::jobs::enqueue(&state.db, project_id, params).await?;
    tracing::info!(user_id = %user.id, %project_id, original_job_id = %job_id, new_job_id = %job.id, "import retriggered");
    Ok((StatusCode::ACCEPTED, Json(JobResponse::from(job))))
}
```

Add the route in `rate_limited_post_router()` (or a separate non-rate-limited router):
```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/{id}/import/{job_id}/retrigger", post(retrigger_import))
}
```

Expose this new router from the crate's `routes::router()` composition.

- [ ] **Step 5: Update import.rs test**

In `test_import_post_is_rate_limited`: remove the `UPDATE users SET startgg_api_key` line (project creation no longer needs an API key). Remove `startgg_base_url` from `AppState` constructor.

- [ ] **Step 6: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/api/src/routes/projects.rs backend/crates/api/src/routes/import.rs
git commit -m "feat(api): remove API key gate from project creation; add import retrigger endpoint"
```

---

### Task 7: API players route rewrite

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`

**Interfaces:**
- Produces: `POST /projects/:id/players/:pid/accounts` resolves handle from `global_players`; `POST /projects/:id/players/by-handles` same; `GET /projects/:id/tournaments/entrants` queries global tables

- [ ] **Step 1: Rewrite `link_account`**

Remove `StartggClient` import and the `user.startgg_api_key` lookup. Replace with a `global_players` query:

```rust
async fn link_account(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, player_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<LinkAccountRequest>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    let handle = body.handle.trim_start_matches("user/");

    let gp = sqlx::query!(
        "SELECT startgg_user_id, handle, display_name FROM global_players WHERE handle ILIKE $1 AND startgg_user_id IS NOT NULL LIMIT 1",
        handle,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound)?; // 404 = not yet indexed

    let user_id = gp.startgg_user_id.unwrap(); // safe: filtered above

    let row = sqlx::query!(
        "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (player_id, startgg_user_id) DO NOTHING
         RETURNING id, player_id, startgg_user_id, handle, display_name, created_at",
        player_id,
        user_id,
        gp.handle,
        gp.display_name,
    )
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => Ok((StatusCode::CREATED, Json(StartggAccountResponse {
            id: r.id,
            player_id: r.player_id,
            startgg_user_id: r.startgg_user_id,
            handle: r.handle,
            display_name: r.display_name,
            created_at: r.created_at,
        }))),
        None => Err(AppError::Conflict("Account already linked".into())),
    }
}
```

- [ ] **Step 2: Rewrite `add_players_by_handles`**

Remove `StartggClient` usage. For each handle, query `global_players WHERE handle ILIKE $1`. Keep all surrounding logic (duplicate player check, player creation, account INSERT) unchanged — only replace the `StartggClient.user_by_slug()` call with a DB lookup:

```rust
for handle in &body.handles {
    let handle_str = handle.trim().trim_start_matches("user/");
    let gp = match sqlx::query!(
        "SELECT startgg_user_id, handle, display_name FROM global_players WHERE handle ILIKE $1 AND startgg_user_id IS NOT NULL LIMIT 1",
        handle_str,
    )
    .fetch_optional(&state.db)
    .await? {
        Some(r) => r,
        None => {
            results.push(AddPlayerResult {
                handle: handle_str.to_string(),
                status: "not_indexed".into(),
                player_id: None,
            });
            continue;
        }
    };

    let startgg_user_id = gp.startgg_user_id.unwrap();

    // Check if a player with this account already exists in the project
    let existing = sqlx::query!(
        "SELECT p.id FROM players p
         JOIN startgg_accounts sa ON sa.player_id = p.id
         WHERE p.project_id = $1 AND sa.startgg_user_id = $2",
        project_id,
        startgg_user_id,
    )
    .fetch_optional(&state.db)
    .await?;

    if let Some(row) = existing {
        results.push(AddPlayerResult {
            handle: gp.handle.clone(),
            status: "duplicate".into(),
            player_id: Some(row.id),
        });
        continue;
    }

    // Create player + link account
    let player = sqlx::query!(
        "INSERT INTO players (project_id, name) VALUES ($1, $2) RETURNING id",
        project_id,
        gp.handle,
    )
    .fetch_one(&state.db)
    .await?;

    sqlx::query!(
        "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (player_id, startgg_user_id) DO NOTHING",
        player.id,
        startgg_user_id,
        gp.handle,
        gp.display_name,
    )
    .execute(&state.db)
    .await?;

    results.push(AddPlayerResult {
        handle: gp.handle,
        status: "created".into(),
        player_id: Some(player.id),
    });
}
```

- [ ] **Step 3: Rewrite `list_tournament_entrants`**

Remove `StartggClient` usage. Query global tables and build the response by grouping rows by event. The existing response type is `Vec<TournamentEventResp>` where each element has a `name: String` and `entrants: Vec<EntrantResp>`. Preserve the exact response shape:

```rust
pub async fn list_tournament_entrants(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path((project_id, tournament_handle)): Path<(Uuid, String)>,
) -> Result<impl IntoResponse> {
    let slug = format!("tournament/{}", tournament_handle.trim_start_matches("tournament/"));

    struct EntrantRow {
        startgg_user_id: Option<i64>,
        handle: String,
        display_name: Option<String>,
        event_name: String,
        placement: Option<i32>,
        seed: Option<i32>,
    }

    let rows = sqlx::query_as!(
        EntrantRow,
        r#"
        SELECT
            gp.startgg_user_id,
            gp.handle,
            gp.display_name,
            ge.name AS event_name,
            gee.placement,
            gee.seed
        FROM global_tournaments gt
        JOIN global_events ge ON ge.tournament_id = gt.id
        JOIN global_event_entries gee ON gee.event_id = ge.id
        JOIN global_players gp ON gp.id = gee.player_id
        WHERE gt.slug = $1
          AND gp.startgg_user_id IS NOT NULL
        ORDER BY ge.name, gee.placement NULLS LAST
        "#,
        slug,
    )
    .fetch_all(&state.db)
    .await?;

    // Group by event_name, preserving insertion order
    let mut events: indexmap::IndexMap<String, Vec<EntrantResp>> = indexmap::IndexMap::new();
    for row in rows {
        let entrant = EntrantResp {
            startgg_user_id: row.startgg_user_id,
            handle: row.handle,
            display_name: row.display_name,
            placement: row.placement,
            seed: row.seed,
        };
        events.entry(row.event_name).or_default().push(entrant);
    }

    let result: Vec<TournamentEventResp> = events
        .into_iter()
        .map(|(name, entrants)| TournamentEventResp { name, entrants })
        .collect();

    Ok(Json(result))
}
```

Note: if the crate doesn't already use `indexmap`, use `cargo add indexmap`. Alternatively, use a `Vec<(String, Vec<EntrantResp>)>` with dedup-by-key if `indexmap` is unavailable — but preserving event order is important for the response.

- [ ] **Step 4: Rewrite unit tests in `players.rs`**

In the `#[cfg(test)]` block, the three handler tests currently call `StartggClient` via wiremock. Replace each with direct `global_players` seeding:

**`test_link_account`** — before calling `POST /players/:id/accounts`, insert a `global_players` row with the target handle:
```rust
sqlx::query!(
    "INSERT INTO global_players (startgg_user_id, handle, display_name) VALUES (99999, 'Mango', 'Juan')",
)
.execute(&pool)
.await
.unwrap();
```
Then call the endpoint with `json!({ "handle": "Mango" })`. Assert 201 and that a `startgg_accounts` row exists:
```rust
let account = sqlx::query!(
    "SELECT startgg_user_id FROM startgg_accounts WHERE player_id = $1",
    player_id,
)
.fetch_one(&pool)
.await
.unwrap();
assert_eq!(account.startgg_user_id, 99999);
```

**`test_link_account_not_found`** — call the endpoint with a handle that has no `global_players` row. Assert 404.

**`test_add_players_by_handles`** — insert two `global_players` rows, call `POST /players/by-handles` with both handles. Assert 201 and two players created.

**`test_list_tournament_entrants`** — seed a minimal `global_tournaments` + `global_events` + `global_event_entries` + `global_players` scenario (reuse the pattern from `seed_global_data` in Task 10). Assert the endpoint returns grouped events with entrant data.

Remove all `wiremock` imports and `MockServer` setups from the players.rs test module. Remove `startgg_base_url` from `AppState` constructors.

- [ ] **Step 5: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/api/src/routes/players.rs
git commit -m "feat(api/players): rewrite account linking and tournament entrants to use global mirror"
```

---

### Task 8: API tournaments route rewrite

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`

**Interfaces:**
- Produces: all handlers join through `global_*` tables; `TournamentResponse`/`ProjectEventResponse` shapes unchanged; `delete_tournament` targets `project_events`

- [ ] **Step 1: Update `list_tournaments` query**

Replace the inner `sqlx::query_as!` to join `ranking_events → global_events → global_tournaments → global_phases`:

```rust
let rows = sqlx::query_as!(
    Row,
    r#"
    SELECT
        gt.id            AS tournament_id,
        gt.startgg_id    AS tournament_startgg_id,
        gt.name          AS tournament_name,
        gt.slug          AS tournament_handle,
        gt.city,
        gt.addr_state,
        gt.country_code,
        gt.venue_name,
        gt.online        AS "online!: bool",
        gt.start_at      AS tournament_start_at,
        gt.end_at,
        ge.id            AS event_id,
        ge.startgg_id    AS event_startgg_id,
        ge.name          AS event_name,
        gg.name          AS game_name,
        ge.num_entrants,
        ge.start_at      AS event_start_at,
        re.included,
        NULL::INTEGER    AS event_type,
        ARRAY(
            SELECT gp.bracket_type
            FROM global_phases gp
            WHERE gp.event_id = ge.id
              AND gp.bracket_type IS NOT NULL
            ORDER BY gp.phase_order ASC NULLS LAST
        ) AS "bracket_types!: Vec<String>"
    FROM ranking_events re
    JOIN global_events ge     ON ge.id = re.global_event_id
    JOIN global_tournaments gt ON gt.id = ge.tournament_id
    LEFT JOIN global_games gg  ON gg.id = ge.game_id
    WHERE re.ranking_id = $1
    ORDER BY gt.start_at DESC NULLS LAST, gt.name ASC, ge.name ASC
    "#,
    path.rid,
)
.fetch_all(&state.db)
.await?;
```

Update `Row` struct: `event_id: Uuid`, `event_startgg_id: i64`, `tournament_handle: String` (was `handle`), `online: bool`. Remove `venue_name` reference from struct if it no longer maps (it still does — `global_tournaments.venue_name` exists).

- [ ] **Step 2: Update `put_events` query**

Change `ranking_events SET included = $1 WHERE ranking_id = $2 AND event_id = ANY($3)` to `WHERE ranking_id = $2 AND global_event_id = ANY($3)`.

- [ ] **Step 3: Update `get_stats` query**

The stats query currently joins `sets → entrants → ranking_events re ON re.event_id`. Rewrite to join `ranking_set_results → global_sets → global_players → startgg_accounts`:

```sql
SELECT
    rsr.winner_player_id,
    rsr.loser_player_id,
    rsr.upset_factor,
    rsr.completed_at,
    gs.winner_score,
    gs.loser_score,
    gs.round_name,
    gs.is_dq,
    gs.vod_url,
    gs.startgg_id   AS startgg_set_id,
    ge.name         AS event_name,
    gt.name         AS tournament_name,
    gt.slug         AS tournament_handle,
    wee.seed        AS winner_seed,
    lee.seed        AS loser_seed,
    wee.placement   AS winner_placement,
    lee.placement   AS loser_placement,
    gwp.handle      AS winner_handle,
    glp.handle      AS loser_handle,
    gph.name        AS phase_name,
    gpg.display_identifier AS pool_identifier,
    gt.city,
    gt.addr_state,
    gt.country_code,
    gt.online AS "online!: bool",
    ge.num_entrants,
    ge.slug         AS event_handle
FROM ranking_set_results rsr
JOIN global_sets gs        ON gs.id  = rsr.global_set_id
JOIN global_events ge      ON ge.id  = rsr.global_event_id
JOIN global_tournaments gt ON gt.id  = ge.tournament_id
JOIN global_players gwp    ON gwp.id = gs.winner_player_id
JOIN global_players glp    ON glp.id = gs.loser_player_id
LEFT JOIN global_event_entries wee ON wee.event_id = ge.id AND wee.player_id = gwp.id
LEFT JOIN global_event_entries lee ON lee.event_id = ge.id AND lee.player_id = glp.id
LEFT JOIN global_phase_groups gpg ON gpg.id = gs.phase_group_id
LEFT JOIN global_phases gph       ON gph.id = gpg.phase_id
WHERE rsr.ranking_id = $1
  AND (rsr.winner_player_id = $2 OR rsr.loser_player_id = $2)
ORDER BY rsr.completed_at DESC NULLS LAST
```

The display name for opponents who are not project players now comes from `global_players.handle` (not `entrants.display_name`).

- [ ] **Step 4: Update `get_head_to_head`, `get_player_stats`, `get_h2h_sets`, `get_player_tournaments`, `get_ranking_player_tournaments`**

Each of the five remaining handlers uses the same substitution: old project-scoped table references become global table references with the same column values. For each handler, replace the inner SQL with the following.

**`get_head_to_head`** — currently queries sets between two specific players. Replace:
```sql
SELECT
    rsr.winner_player_id,
    rsr.loser_player_id,
    rsr.upset_factor,
    rsr.completed_at,
    gs.startgg_id   AS startgg_set_id,
    gs.winner_score,
    gs.loser_score,
    gs.round_name,
    gs.is_dq,
    gs.vod_url,
    ge.name         AS event_name,
    ge.slug         AS event_handle,
    gt.name         AS tournament_name,
    gt.slug         AS tournament_handle,
    gt.start_at     AS tournament_start_at,
    gph.name        AS phase_name,
    gpg.display_identifier AS pool_identifier,
    wee.seed        AS winner_seed,
    lee.seed        AS loser_seed
FROM ranking_set_results rsr
JOIN global_sets gs        ON gs.id  = rsr.global_set_id
JOIN global_events ge      ON ge.id  = rsr.global_event_id
JOIN global_tournaments gt ON gt.id  = ge.tournament_id
JOIN global_players gwp    ON gwp.id = gs.winner_player_id
JOIN global_players glp    ON glp.id = gs.loser_player_id
LEFT JOIN global_event_entries wee ON wee.event_id = ge.id AND wee.player_id = gwp.id
LEFT JOIN global_event_entries lee ON lee.event_id = ge.id AND lee.player_id = glp.id
LEFT JOIN global_phase_groups gpg ON gpg.id = gs.phase_group_id
LEFT JOIN global_phases gph       ON gph.id = gpg.phase_id
WHERE rsr.ranking_id = $1
  AND ((rsr.winner_player_id = $2 AND rsr.loser_player_id = $3)
    OR (rsr.winner_player_id = $3 AND rsr.loser_player_id = $2))
ORDER BY rsr.completed_at DESC NULLS LAST
```

**`get_player_stats`** — same columns as `get_stats` (Step 3) but without the `AND (rsr.winner_player_id = $2 OR rsr.loser_player_id = $2)` filter (returns aggregate stats for a single player across all their sets in the ranking). The SQL is identical to Step 3 — replace the inner query with the same column list and table joins; keep the existing player-filter `WHERE` clause.

**`get_h2h_sets`** — returns individual set results for a head-to-head pair. Use the `get_head_to_head` SQL above; change the `WHERE` clause to match the endpoint's parameters (ranking_id + two player IDs).

**`get_player_tournaments`** — currently queries `events JOIN tournaments` filtered by player's entrant records. Replace:
```sql
SELECT DISTINCT
    gt.id           AS tournament_id,
    gt.startgg_id   AS tournament_startgg_id,
    gt.name         AS tournament_name,
    gt.slug         AS tournament_handle,
    gt.city,
    gt.addr_state,
    gt.country_code,
    gt.online       AS "online!: bool",
    gt.start_at,
    gt.end_at,
    gt.num_attendees,
    ge.id           AS event_id,
    ge.name         AS event_name,
    ge.num_entrants,
    gee.placement,
    gee.seed
FROM global_event_entries gee
JOIN global_events ge          ON ge.id  = gee.event_id
JOIN global_tournaments gt     ON gt.id  = ge.tournament_id
JOIN global_players gp         ON gp.id  = gee.player_id
JOIN startgg_accounts sa       ON sa.startgg_user_id = gp.startgg_user_id
WHERE sa.player_id = $1
  AND EXISTS (
      SELECT 1 FROM project_events pe
      JOIN projects pr ON pr.id = pe.project_id
      WHERE pe.global_event_id = ge.id
        AND pr.id = $2
  )
ORDER BY gt.start_at DESC NULLS LAST
```

**`get_ranking_player_tournaments`** — same as `get_player_tournaments` but filtered to events in `ranking_events` for a specific ranking instead of `project_events` for a project:
```sql
-- Change the EXISTS subquery to:
EXISTS (
    SELECT 1 FROM ranking_events re
    WHERE re.global_event_id = ge.id
      AND re.ranking_id = $2
      AND re.included = true
)
```

- [ ] **Step 5: Update `delete_tournament`**

```rust
pub async fn delete_tournament(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, tournament_startgg_id)): Path<(Uuid, i64)>,
) -> Result<impl IntoResponse> {
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    let gt = sqlx::query!(
        "SELECT id FROM global_tournaments WHERE startgg_id = $1",
        tournament_startgg_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    sqlx::query!(
        "DELETE FROM project_events
         WHERE project_id = $1
           AND global_event_id IN (SELECT id FROM global_events WHERE tournament_id = $2)",
        project_id,
        gt.id,
    )
    .execute(&state.db)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM ranking_events re
        USING rankings r
        WHERE re.ranking_id = r.id
          AND r.project_id = $1
          AND re.global_event_id IN (SELECT id FROM global_events WHERE tournament_id = $2)
        "#,
        project_id,
        gt.id,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 6: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/api/src/routes/tournaments.rs
git commit -m "feat(api/tournaments): rewrite all handlers to join through global mirror tables"
```

---

### Task 9: API test helper cleanup + prepare-sqlx

**Files:**
- Modify: `backend/crates/api/src/routes/invite_links.rs`
- Modify: `backend/crates/api/src/routes/rankings.rs`
- Modify: `backend/crates/api/src/routes/members.rs`

- [ ] **Step 1: Remove `with_api_key` helper from `invite_links.rs`**

Delete the `async fn with_api_key(pool: &PgPool, email: &str)` function and all calls to it in `test_invite_link_lifecycle` and `test_revoked_link_cannot_be_accepted`. Also remove `startgg_base_url` from any `AppState` constructor in this file's tests.

- [ ] **Step 2: Remove `with_api_key` helper from `rankings.rs`**

Same pattern: delete helper and its calls in `test_create_and_list_rankings` and `test_published_ranking_accessible_without_auth`. Remove `startgg_base_url` from `AppState` constructors.

- [ ] **Step 3: Remove `with_api_key` helper from `members.rs`**

Same pattern: delete helper and calls in `test_add_member_and_list`, `test_remove_member`, `test_transfer_ownership`. Remove `startgg_base_url` from `AppState` constructors.

- [ ] **Step 4: Run `prepare-sqlx.sh` and fix compilation**

```bash
bash backend/prepare-sqlx.sh
```

Expected: Docker container spins up, migrations run, `cargo sqlx prepare` compiles all queries against the new schema and writes `.sqlx/`. If any queries reference columns/tables that no longer exist, the compiler will report them — fix each before proceeding.

- [ ] **Step 5: Run backend unit tests**

```bash
bash backend/test.sh
```

Expected: PASS. The crawler tests still use wiremock (that's fine — crawler binary still calls start.gg). The `common` unit tests and API unit tests should all pass. Full e2e is handled in Task 10.

- [ ] **Step 6: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/api/src/routes/invite_links.rs \
         backend/crates/api/src/routes/rankings.rs \
         backend/crates/api/src/routes/members.rs \
         backend/.sqlx/
git commit -m "chore(api): remove with_api_key test helpers; update sqlx offline cache"
```

---

### Task 10: E2E tests rewrite

**Files:**
- Rewrite: `backend/crates/e2e/tests/full_flow.rs`
- Delete: `backend/crates/e2e/tests/import_live.rs`
- Modify: `backend/crates/e2e/Cargo.toml`

- [ ] **Step 1: Remove wiremock from `e2e/Cargo.toml`**

```toml
[dev-dependencies]
api    = { path = "../api" }
worker = { path = "../worker" }
axum           = { version = "0.8.9" }
http-body-util = "0.1"
serde_json     = "1.0.149"
sqlx = { version = "0.8.6", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "macros", "migrate"] }
tokio = { version = "1.52.3", features = ["full"] }
tower = { version = "0.5", features = ["util"] }
uuid  = { version = "1.23.1", features = ["v4", "serde"] }
common = { path = "../common" }
```
(Remove `wiremock = "0.6"` and the `live-tests` feature if present.)

- [ ] **Step 2: Delete `import_live.rs`**

```bash
rm backend/crates/e2e/tests/import_live.rs
```

- [ ] **Step 3: Rewrite `full_flow.rs` — update imports and `make_app`**

Replace the file header:
```rust
// End-to-end regression test: user registration → project setup → import → stats/H2H.
// Calls the real Axum router and the real import pipeline against seeded global_* tables.

use api::{routes, state::AppState};
use axum::{Router, body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

fn make_app(pool: PgPool) -> Router {
    let state = AppState {
        db: pool,
        cors_origin: "http://localhost".to_string(),
    };
    routes::router().with_state(state)
}
```

(Remove `use common::startgg::StartggClient`, `use wiremock::...`, `startgg_base_url` param from `make_app`.)

- [ ] **Step 4: Add `seed_global_data` helper**

This replaces `set_startgg_api_key` + `mount_import_mocks`. Insert before the test functions:

```rust
/// Seeds the global mirror with the Mango/Armada test scenario:
/// - Two global_players (Mango uid=12345, Armada uid=67890)
/// - One global_tournament → one global_event → one global_phase → one global_phase_group
/// - Two global_event_entries (seed 2 for Mango, seed 7 for Armada)
/// - One global_set (Armada beat Mango)
///
/// Returns (mango_user_id, armada_user_id) so tests can link startgg_accounts.
async fn seed_global_data(pool: &PgPool) -> (i64, i64) {
    let mango_uid: i64 = 12345;
    let armada_uid: i64 = 67890;

    let mango_id = sqlx::query_scalar!(
        "INSERT INTO global_players (startgg_user_id, handle) VALUES ($1, 'mango') RETURNING id",
        mango_uid,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let armada_id = sqlx::query_scalar!(
        "INSERT INTO global_players (startgg_user_id, handle) VALUES ($1, 'armada') RETURNING id",
        armada_uid,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let tournament_id = sqlx::query_scalar!(
        r#"INSERT INTO global_tournaments (startgg_id, name, slug, city, addr_state, country_code,
               online, num_attendees, start_at, end_at)
           VALUES (1001, 'Test Tournament', 'tournament/test-2024', 'San Jose', 'CA', 'US',
                   false, 8, to_timestamp(1700000000), to_timestamp(1700086400))
           RETURNING id"#,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let event_id = sqlx::query_scalar!(
        r#"INSERT INTO global_events (startgg_id, tournament_id, name, slug, start_at, num_entrants, state)
           VALUES (2001, $1, 'Melee Singles', 'tournament/test-2024/event/melee-singles',
                   to_timestamp(1700040000), 2, 'COMPLETED')
           RETURNING id"#,
        tournament_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let phase_id = sqlx::query_scalar!(
        "INSERT INTO global_phases (startgg_id, event_id, name, phase_order, bracket_type)
         VALUES (5001, $1, 'Bracket', 1, 'DOUBLE_ELIMINATION') RETURNING id",
        event_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let phase_group_id = sqlx::query_scalar!(
        "INSERT INTO global_phase_groups (startgg_id, phase_id, display_identifier, bracket_type)
         VALUES (6001, $1, '1', 'DOUBLE_ELIMINATION') RETURNING id",
        phase_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    // Armada seed 7, placement 1 (winner); Mango seed 2, placement 2
    sqlx::query!(
        "INSERT INTO global_event_entries (event_id, player_id, seed, placement) VALUES ($1, $2, 7, 1)",
        event_id, armada_id,
    ).execute(pool).await.unwrap();
    sqlx::query!(
        "INSERT INTO global_event_entries (event_id, player_id, seed, placement) VALUES ($1, $2, 2, 2)",
        event_id, mango_id,
    ).execute(pool).await.unwrap();

    sqlx::query!(
        r#"INSERT INTO global_sets
               (startgg_id, event_id, phase_group_id, winner_player_id, loser_player_id,
                round, round_name, winner_score, loser_score, is_dq, completed_at)
           VALUES (4001, $1, $2, $3, $4, 1, 'Round 1', 3, 1, false, to_timestamp(1700050000))"#,
        event_id, phase_group_id, armada_id, mango_id,
    )
    .execute(pool)
    .await
    .unwrap();

    (mango_uid, armada_uid)
}
```

- [ ] **Step 5: Update `full_import_flow` test**

The test structure stays the same but the setup changes. Replace the wiremock mock server setup and `set_startgg_api_key` with `seed_global_data`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn full_import_flow(pool: PgPool) {
    let (_mango_uid, _armada_uid) = seed_global_data(&pool).await;
    let app = make_app(pool.clone());

    let cookie = register(&app, "user1", "pass1234").await;
    let project_id = create_project(&app, &cookie, "Test Project").await;

    // Add players
    let mango_pid = create_player(&app, &cookie, &project_id, "Mango").await;
    let armada_pid = create_player(&app, &cookie, &project_id, "Armada").await;

    // Link accounts — link_account now looks up global_players by handle
    link_account(&app, &cookie, &project_id, &mango_pid, "mango").await;
    link_account(&app, &cookie, &project_id, &armada_pid, "armada").await;

    // Create ranking and add both players
    let ranking_id = create_ranking(&app, &cookie, &project_id, "Main Ranking").await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &mango_pid).await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &armada_pid).await;

    // Trigger import — response body has "id" (job id)
    let resp = post_json(&app, &format!("/projects/{project_id}/import"), &cookie, json!({})).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let job_body = read_json(resp).await;
    let job_id: Uuid = job_body["id"].as_str().unwrap().parse().unwrap();
    let project_id_uuid: Uuid = project_id.parse().unwrap();
    let ranking_id_uuid: Uuid = ranking_id.parse().unwrap();

    // Run worker inline against the same pool (same as old tests via job queue)
    worker::import::run(&pool, project_id_uuid, job_id, Default::default()).await.unwrap();
    worker::compute::run(&pool, ranking_id_uuid).await.unwrap();

    // Assert tournament appears
    let resp = get_req(&app, &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["name"], "Test Tournament");

    // Assert stats show one set
    let resp = get_req(&app, &format!("/projects/{project_id}/rankings/{ranking_id}/stats"), &cookie).await;
    let stats = read_json(resp).await;
    let total_sets: usize = stats.as_array().unwrap().iter()
        .map(|p| p["wins"].as_array().map(|a| a.len()).unwrap_or(0) + p["losses"].as_array().map(|a| a.len()).unwrap_or(0))
        .sum();
    assert!(total_sets > 0, "expected at least one set in stats");
}
```

- [ ] **Step 6: Update all remaining tests in `full_flow.rs`**

For every test that previously called `mount_import_mocks(&mock).await` and `set_startgg_api_key(&pool, &cookie, "test-key").await`:
1. Remove those calls
2. Call `seed_global_data(&pool).await` instead
3. Update `make_app(pool.clone(), &mock.uri())` → `make_app(pool.clone())`

The test logic and assertions remain unchanged — only the data setup method changes.

- [ ] **Step 7: Run e2e tests to verify**

```bash
bash backend/test.sh
```

Expected: all e2e tests PASS.

- [ ] **Step 8: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/e2e/
git commit -m "test(e2e): rewrite full_flow to seed global tables; remove import_live and wiremock"
```

---

### Task 11: Topology smoke test rewrite

**Files:**
- Modify: `backend/crates/topology/Cargo.toml`
- Rewrite: `backend/crates/topology/tests/smoke.rs`

- [ ] **Step 1: Add sqlx to topology `Cargo.toml`**

```bash
cd backend/crates/topology && cargo add --dev sqlx --features runtime-tokio-rustls,postgres,uuid,chrono,macros,migrate
```

- [ ] **Step 2: Rewrite `smoke.rs`**

The test is HTTP-only (hits a live API server). After migration, it needs to seed global tables before triggering import. Add `DATABASE_URL` as required env var and a `seed_topology_data` async function that inserts directly:

```rust
#![cfg(feature = "topology-tests")]

use reqwest::Client;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::sleep;

const PLAYER1_SLUG: &str = "user/06b4042d";
const PLAYER2_SLUG: &str = "user/54b7bbf3";
const WEEKLY_100_NAME: &str = "Smash Hannover Weekly #100";
const WEEKLY_88_NAME: &str = "Smash Hannover Weekly #88";

fn api_url() -> String {
    std::env::var("API_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set to run topology tests")
}

// ... (keep existing register, post_json, get_json, wait_for_api helpers unchanged)

/// Seeds global mirror rows for the two Hannover Weekly players and their shared events.
/// This replaces the old start.gg API key requirement — the global mirror is seeded
/// directly into the DB so the import job can find data without hitting start.gg.
async fn seed_topology_data(pool: &PgPool) {
    // Insert two players using their known start.gg user IDs
    // (These are the real IDs for the Hannover Weekly test players)
    let p1_id: i64 = 1823808;  // user/06b4042d
    let p2_id: i64 = 3619891;  // user/54b7bbf3

    sqlx::query!(
        "INSERT INTO global_players (startgg_user_id, handle) VALUES ($1, '06b4042d'), ($2, '54b7bbf3')
         ON CONFLICT (startgg_user_id) DO NOTHING",
        p1_id, p2_id,
    )
    .execute(pool)
    .await
    .expect("failed to seed global_players");

    // Insert the tournament + event + entries + a set for at least one of the Hannover Weeklies
    let tournament_id = sqlx::query_scalar!(
        r#"INSERT INTO global_tournaments (startgg_id, name, slug, online, start_at)
           VALUES (612663, 'Smash Hannover Weekly #100', 'tournament/smash-hannover-weekly-100', false, '2025-11-10')
           ON CONFLICT (startgg_id) DO UPDATE SET name = EXCLUDED.name
           RETURNING id"#,
    )
    .fetch_one(pool)
    .await
    .expect("failed to seed global_tournament");

    let event_id = sqlx::query_scalar!(
        r#"INSERT INTO global_events (startgg_id, tournament_id, name, state)
           VALUES (1534512, $1, 'Melee Singles', 'COMPLETED')
           ON CONFLICT (startgg_id) DO UPDATE SET name = EXCLUDED.name
           RETURNING id"#,
        tournament_id,
    )
    .fetch_one(pool)
    .await
    .expect("failed to seed global_event");

    let p1_gp = sqlx::query_scalar!(
        "SELECT id FROM global_players WHERE startgg_user_id = $1", p1_id
    ).fetch_one(pool).await.unwrap();
    let p2_gp = sqlx::query_scalar!(
        "SELECT id FROM global_players WHERE startgg_user_id = $1", p2_id
    ).fetch_one(pool).await.unwrap();

    sqlx::query!(
        "INSERT INTO global_event_entries (event_id, player_id, seed, placement) VALUES ($1, $2, 1, 2), ($1, $3, 2, 1)
         ON CONFLICT DO NOTHING",
        event_id, p1_gp, p2_gp,
    )
    .execute(pool)
    .await
    .expect("failed to seed entries");

    sqlx::query!(
        r#"INSERT INTO global_sets (startgg_id, event_id, winner_player_id, loser_player_id, is_dq, completed_at)
           VALUES (9999901, $1, $2, $3, false, NOW())
           ON CONFLICT DO NOTHING"#,
        event_id, p2_gp, p1_gp,
    )
    .execute(pool)
    .await
    .expect("failed to seed set");
}

#[tokio::test]
async fn smoke_import_roundtrip() {
    let client = Client::new();
    wait_for_api(&client).await;

    // Seed global mirror data so the import job can find events
    let pool = PgPool::connect(&db_url()).await.expect("failed to connect to DB");
    seed_topology_data(&pool).await;

    let unique_email = format!(
        "topology-{}@test.com",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let session_id = register(&client, &unique_email, "password1234").await;

    // No API key setup needed — project creation is now ungated
    let project = post_json(
        &client,
        "/projects",
        &session_id,
        json!({ "name": "Topology Smoke Test", "game_id": 1, "game_name": "Super Smash Bros. Melee" }),
    )
    .await;
    let project_id = project["id"].as_str().unwrap().to_string();

    // Add players and link their seeded global accounts
    let mut player_ids: Vec<String> = Vec::new();
    for (name, slug) in [("Player1", PLAYER1_SLUG), ("Player2", PLAYER2_SLUG)] {
        let player = post_json(&client, &format!("/projects/{project_id}/players"), &session_id, json!({ "name": name })).await;
        let player_id = player["id"].as_str().unwrap().to_string();
        post_json(
            &client,
            &format!("/projects/{project_id}/players/{player_id}/accounts"),
            &session_id,
            json!({ "handle": slug.trim_start_matches("user/") }),
        ).await;
        player_ids.push(player_id);
    }

    // Create ranking and add players
    let ranking = post_json(&client, &format!("/projects/{project_id}/rankings"), &session_id, json!({ "name": "Topology Smoke Ranking" })).await;
    let ranking_id = ranking["id"].as_str().unwrap().to_string();
    for player_id in &player_ids {
        let _ = post_json(&client, &format!("/projects/{project_id}/rankings/{ranking_id}/players"), &session_id, json!({ "player_id": player_id })).await;
    }

    // Trigger import
    let resp = client
        .post(format!("{}/projects/{project_id}/import", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .json(&json!({}))
        .send()
        .await
        .expect("POST import failed");
    assert!(resp.status().is_success(), "POST /import returned {}", resp.status());

    // Poll for completion
    let mut last_status = String::from("unknown");
    for _ in 0..300 {
        sleep(Duration::from_secs(2)).await;
        let import = get_json(&client, &format!("/projects/{project_id}/import"), &session_id).await;
        last_status = import["status"].as_str().unwrap_or("unknown").to_string();
        match last_status.as_str() {
            "done" => break,
            "failed" => panic!("import failed: {}", import["error"].as_str().unwrap_or("(no error)")),
            _ => {}
        }
    }
    assert_eq!(last_status, "done", "import did not complete within 600s");

    // Assert tournament appears
    let tournaments = get_json(&client, &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"), &session_id).await;
    let names: Vec<&str> = tournaments.as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.iter().any(|n| *n == WEEKLY_100_NAME || *n == WEEKLY_88_NAME),
        "expected a Hannover Weekly in tournaments; got: {:?}", names);

    // Assert at least one set in stats
    let stats = get_json(&client, &format!("/projects/{project_id}/rankings/{ranking_id}/stats"), &session_id).await;
    let total_sets: usize = stats.as_array().unwrap().iter()
        .map(|p| p["wins"].as_array().map(|a| a.len()).unwrap_or(0) + p["losses"].as_array().map(|a| a.len()).unwrap_or(0))
        .sum();
    assert!(total_sets > 0, "expected at least one set in stats");
}
```

- [ ] **Step 3: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/topology/
git commit -m "test(topology): remove API key step; add DB seeding for global mirror data"
```

---

### Task 12: Frontend cleanup

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/app.d.ts`
- Modify: `web/src/routes/account/+page.svelte`
- Modify: `web/src/routes/account/+page.server.ts`
- Modify: `web/src/routes/projects/new/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/(hub)/(editor)/import/+page.svelte`
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/h2h/H2HTestWrapper.svelte`

- [ ] **Step 1: Remove `has_startgg_key` from types**

In `web/src/lib/types.ts`:
- Remove `has_startgg_key: boolean` from the `User` type
- Remove `owner_has_startgg_key: boolean` from the `Project` type

In `web/src/app.d.ts`:
- Remove `has_startgg_key: boolean` from `locals.user`

- [ ] **Step 2: Remove API key section from `account/+page.svelte`**

Remove the entire `{#if data.user.has_startgg_key}` block and the "Set API key" form section. The account page should only show profile and password sections.

- [ ] **Step 3: Remove API key actions from `account/+page.server.ts`**

Remove the `set-startgg-key` and `delete-startgg-key` form actions (the `api.put("/account/startgg-key", ...)` and `api.delete("/account/startgg-key")` calls). Keep only the profile and password actions.

- [ ] **Step 4: Remove gate from `projects/new/+page.server.ts`**

Remove the `hasStartggKey` return value from the `load` function. Remove any frontend code that reads `data.hasStartggKey` and blocks project creation.

- [ ] **Step 5: Update import page**

In `web/src/routes/projects/[id]/(hub)/(editor)/import/+page.svelte`:
- Remove the `{#if !data.project.owner_has_startgg_key}` warning block
- Add a Re-run button to each job card. The button calls `POST /projects/{id}/import/{job_id}/retrigger`. Example using shadcn Button:
```svelte
<Button
  variant="outline"
  size="sm"
  onclick={() => retriggerImport(job.id)}
>
  Re-run
</Button>
```

Add the handler:
```typescript
async function retriggerImport(jobId: string) {
  await fetch(`/projects/${data.project.id}/import/${jobId}/retrigger`, {
    method: 'POST',
    credentials: 'include'
  });
  // Refresh job list
  await invalidateAll();
}
```

- [ ] **Step 6: Fix `H2HTestWrapper.svelte`**

Remove `has_startgg_key: boolean` from the inline project mock object.

- [ ] **Step 7: Run frontend tests**

```bash
cd web && npm run test:unit && npm run test:e2e
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
cd web && npm run format
git add web/src/
git commit -m "feat(web): remove start.gg API key UI; add import Re-run button"
```

---

### Task 13: OpenAPI docs + final verification

**Files:**
- Modify: `backend/openapi.yaml`
- Modify: `docs/DESIGN.md`

- [ ] **Step 1: Update `openapi.yaml`**

- Remove `PUT /account/startgg-key` and `DELETE /account/startgg-key` endpoints
- Remove `has_startgg_key` from the `User` response schema and `owner_has_startgg_key` from `Project`
- Remove `startgg_api_key` from project creation request schema (if present)
- Add `POST /projects/{id}/import/{jobId}/retrigger` endpoint (returns `JobResponse`, 202 on success, 404 if job not found)
- Remove any request body field `api_key` from project creation

- [ ] **Step 2: Update `docs/DESIGN.md`**

Update the data model section to reflect the removal of project-scoped tournament tables and the addition of `project_events`. Note the enriched `global_tournaments` image and venue fields.

- [ ] **Step 3: Update `docs/modules.md`**

Remove `StartggClient` from the api and worker entries (it is now only used by the crawler). Add a note to the worker entry that import is now pure Postgres-to-Postgres with no network calls.

- [ ] **Step 4: Run full test suite**

```bash
bash test.sh
```

Expected: PASS.

- [ ] **Step 5: Final commit**

```bash
git add backend/openapi.yaml docs/DESIGN.md docs/modules.md
git commit -m "docs: update openapi, DESIGN.md, and modules.md for mirror-backed rankings"
```
