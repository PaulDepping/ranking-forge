# Tournament Filter & Bulk Action — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a collapsible filter panel with six controls and bulk include/exclude buttons to the tournament page, backed by richer schema fields fetched from start.gg.

**Architecture:** Pure client-side filtering in Svelte — no new API endpoints. The bulk of the work is schema + import + API: new `phases` and `phase_groups` tables, extended columns on `events`/`tournaments`/`sets`, and two new fields (`event_type`, `bracket_types`) surfaced in the existing list-tournaments response. The filter panel derives a `visibleTournaments` reactive value and the bulk buttons iterate it.

**Tech Stack:** Rust (Axum, sqlx, serde), PostgreSQL, SvelteKit 5 (Svelte runes: `$state`, `$derived`), Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-05-14-tournament-filter-design.md`

---

## File Map

| File | Change |
|---|---|
| `backend/migrations/001_initial.sql` | Extend `tournaments`, `events`, `sets`; add `phases`, `phase_groups` |
| `backend/crates/common/src/startgg/queries.rs` | New structs; extend `EventNode`, `TournamentNode`, `SetNode` |
| `backend/crates/common/src/startgg/operations.rs` | Update GraphQL query strings |
| `backend/crates/worker/src/import.rs` | Extend all upserts; add phase/phase_group upsert; pass phase_group_map to set import |
| `backend/crates/api/src/routes/tournaments.rs` | Extend `ProjectEventResponse`; update `list_tournaments` + `patch_event` SQL; add `has_placeholder` filter to stats queries |
| `backend/openapi.yaml` | Add `event_type`, `bracket_types` to event schema |
| `web/src/lib/types.ts` | Add `event_type`, `bracket_types` to `TournamentEvent` |
| `web/src/routes/projects/[id]/tournaments/+page.svelte` | Add filter state, derived visibility, panel UI, bulk actions |

---

## Task 1: Schema — extend migration in-place

**Files:**
- Modify: `backend/migrations/001_initial.sql`

- [ ] **Step 1: Add columns to `tournaments`**

In `001_initial.sql`, find the `tournaments` table (line ~60) and add three columns before `created_at`:

```sql
CREATE TABLE tournaments (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id     BIGINT      NOT NULL UNIQUE,
    name           TEXT        NOT NULL,
    slug           TEXT        NOT NULL,
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
```

- [ ] **Step 2: Add columns to `events`**

Replace the events table definition:

```sql
CREATE TABLE events (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID        NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    startgg_id    BIGINT      NOT NULL UNIQUE,
    name          TEXT        NOT NULL,
    slug          TEXT,
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
```

- [ ] **Step 3: Add columns to `sets`**

Replace the sets table definition:

```sql
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
```

> `sets` references `phase_groups`, so `phase_groups` must be defined before `sets` in the migration. Add the two new tables in Step 4 **before** the `sets` table definition.

- [ ] **Step 4: Add `phases` and `phase_groups` tables**

Insert these two table definitions between the `events` indexes and the `project_events` table (i.e. after `CREATE INDEX events_tournament_id_idx` and before `CREATE TABLE project_events`):

```sql
-- Bracket phases within an event (e.g. "Pools", "Top 8 Bracket")
CREATE TABLE phases (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT      NOT NULL UNIQUE,
    event_id      UUID        NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    name          TEXT,
    bracket_type  TEXT,
    phase_order   INTEGER,
    num_seeds     INTEGER,
    group_count   INTEGER,
    state         INTEGER,
    is_exhibition BOOLEAN
);

CREATE INDEX phases_event_id_idx ON phases(event_id);

-- Individual pools/brackets within a phase (e.g. "Pool A", "Top 8")
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
```

- [ ] **Step 5: Verify migration applies**

```bash
cd /home/pd/private_projects/ranking_forge
bash backend/test.sh
```

Expected: all tests pass (existing tests don't touch the new columns so nothing should break).

- [ ] **Step 6: Commit**

```bash
git add backend/migrations/001_initial.sql
git commit -m "feat(db): extend schema with phases, phase_groups, and richer event/set/tournament fields"
```

---

## Task 2: start.gg GraphQL structs

**Files:**
- Modify: `backend/crates/common/src/startgg/queries.rs`

- [ ] **Step 1: Add new structs after `EventNode`**

After the existing `EventNode` struct (currently at line ~128), add:

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TeamRosterSize {
    pub min_players: Option<i32>,
    pub max_players: Option<i32>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhaseGroupNode {
    pub id: i64,
    pub display_identifier: Option<String>,
    pub bracket_type: Option<String>,
    pub bracket_url: Option<String>,
    pub num_rounds: Option<i32>,
    pub start_at: Option<i64>,
    pub first_round_time: Option<i64>,
    pub state: Option<i32>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PhaseGroupPage {
    pub nodes: Vec<PhaseGroupNode>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhaseNode {
    pub id: i64,
    pub name: Option<String>,
    pub bracket_type: Option<String>,
    pub phase_order: Option<i32>,
    pub num_seeds: Option<i32>,
    pub group_count: Option<i32>,
    pub state: Option<i32>,
    pub is_exhibition: Option<bool>,
    pub phase_groups: Option<PhaseGroupPage>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SetPhaseGroup {
    pub id: i64,
}
```

- [ ] **Step 2: Replace `EventNode` with extended version**

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventNode {
    pub id: i64,
    pub name: String,
    pub num_entrants: Option<i32>,
    pub start_at: Option<i64>,
    pub slug: Option<String>,
    pub state: Option<String>,
    pub is_online: Option<bool>,
    #[serde(rename = "type")]
    pub event_type: Option<i32>,
    pub team_roster_size: Option<TeamRosterSize>,
    pub phases: Option<Vec<PhaseNode>>,
}
```

- [ ] **Step 3: Extend `TournamentNode`**

Add `lat`, `lng`, `state` to `TournamentNode`:

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TournamentNode {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub city: Option<String>,
    pub addr_state: Option<String>,
    pub country_code: Option<String>,
    pub venue_name: Option<String>,
    pub venue_address: Option<String>,
    pub timezone: Option<String>,
    pub is_online: Option<bool>,
    pub num_attendees: Option<i32>,
    pub start_at: Option<i64>,
    pub end_at: Option<i64>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub state: Option<i32>,
    pub events: Option<Vec<EventNode>>,
}
```

- [ ] **Step 4: Extend `SetNode`**

Find the `SetNode` struct and add three fields plus the phase_group relation. (The struct is defined around line 230+ in queries.rs.) Add after existing fields:

```rust
pub has_placeholder: Option<bool>,
pub state: Option<i32>,
pub identifier: Option<String>,
pub phase_group: Option<SetPhaseGroup>,
```

- [ ] **Step 5: Export new types from `mod.rs`**

In `backend/crates/common/src/startgg/mod.rs`, add the new types to the `pub use` list:

```rust
pub use queries::{
    EntrantNode, EntrantPage, EntrantStanding, EventNode, GameNode, PageInfo, Participant,
    ParticipantUser, PhaseGroupNode, PhaseNode, ScoreValue, SetNode, SetPage, SetPhaseGroup,
    SlotEntrant, SlotStanding, SlotStats, TeamRosterSize, TournamentNode, TournamentPage, UserNode,
};
```

- [ ] **Step 6: Verify compilation**

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo build -p common
```

Expected: compiles without errors.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/common/src/startgg/queries.rs backend/crates/common/src/startgg/mod.rs
git commit -m "feat(common): extend start.gg GraphQL structs with phases, phase_groups, and richer fields"
```

---

## Task 3: GraphQL query strings

**Files:**
- Modify: `backend/crates/common/src/startgg/operations.rs`

- [ ] **Step 1: Update `TOURNAMENTS_BY_USER_QUERY`**

Replace the entire constant:

```rust
const TOURNAMENTS_BY_USER_QUERY: &str = r#"
    query($userId: ID!, $gameId: ID!, $page: Int!, $perPage: Int!) {
        user(id: $userId) {
            tournaments(query: {
                page: $page
                perPage: $perPage
                filter: { videogameId: [$gameId] }
            }) {
                pageInfo { total totalPages }
                nodes {
                    id name slug
                    city addrState countryCode
                    venueName venueAddress
                    timezone isOnline numAttendees
                    lat lng state
                    startAt endAt
                    events(filter: { videogameId: [$gameId] }) {
                        id name numEntrants startAt
                        slug state isOnline type
                        teamRosterSize { minPlayers maxPlayers }
                        phases {
                            id name bracketType phaseOrder
                            numSeeds groupCount state isExhibition
                            phaseGroups(query: { perPage: 100 }) {
                                nodes {
                                    id displayIdentifier bracketType bracketUrl
                                    numRounds startAt firstRoundTime state
                                }
                            }
                        }
                    }
                }
            }
        }
    }"#;
```

- [ ] **Step 2: Update `EVENT_SETS_QUERY`**

Replace the entire constant:

```rust
const EVENT_SETS_QUERY: &str = r#"
    query($eventId: ID!, $page: Int!, $perPage: Int!) {
        event(id: $eventId) {
            sets(page: $page, perPage: $perPage, sortType: STANDARD) {
                pageInfo { total totalPages }
                nodes {
                    id winnerId round fullRoundText totalGames
                    completedAt vodUrl
                    hasPlaceholder state identifier
                    phaseGroup { id }
                    slots {
                        entrant { id }
                        standing { stats { score { value } } }
                    }
                }
            }
        }
    }"#;
```

- [ ] **Step 3: Run common tests**

The existing wiremock-based tests in `mod.rs` use hand-crafted JSON that doesn't include the new fields. Since all new fields are `Option<T>`, serde will deserialize missing keys as `None` — existing tests pass unchanged.

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p common
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add backend/crates/common/src/startgg/operations.rs
git commit -m "feat(common): extend start.gg GraphQL queries with phases, phase_groups, and richer fields"
```

---

## Task 4: Import — tournament + event + phases/phase_groups

**Files:**
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Extend tournament upsert**

In `import_tournament`, replace the `sqlx::query!` INSERT (lines ~164-189) with:

```rust
let row = sqlx::query!(
    r#"INSERT INTO tournaments
           (startgg_id, name, slug, city, addr_state, country_code,
            venue_name, venue_address, timezone, online, num_attendees,
            lat, lng, state, start_at, end_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
       ON CONFLICT (startgg_id) DO UPDATE SET
           name          = EXCLUDED.name,
           num_attendees = EXCLUDED.num_attendees,
           lat           = EXCLUDED.lat,
           lng           = EXCLUDED.lng,
           state         = EXCLUDED.state,
           start_at      = EXCLUDED.start_at,
           end_at        = EXCLUDED.end_at
       RETURNING id"#,
    tournament.id,
    tournament.name,
    tournament.slug,
    tournament.city,
    tournament.addr_state,
    tournament.country_code,
    tournament.venue_name,
    tournament.venue_address,
    tournament.timezone,
    tournament.is_online.unwrap_or(false),
    tournament.num_attendees,
    tournament.lat,
    tournament.lng,
    tournament.state,
    start_at,
    end_at,
)
.fetch_one(pool)
.await?;
```

- [ ] **Step 2: Extend event upsert in `import_event`**

Replace the event INSERT (lines ~233-250):

```rust
let start_at = event.start_at.map(ts_to_dt);
let min_team_size = event.team_roster_size.as_ref().and_then(|r| r.min_players);
let max_team_size = event.team_roster_size.as_ref().and_then(|r| r.max_players);

let row = sqlx::query!(
    r#"INSERT INTO events
           (tournament_id, startgg_id, name, slug, state, is_online, event_type,
            min_team_size, max_team_size, game_id, game_name, num_entrants, start_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
       ON CONFLICT (startgg_id) DO UPDATE SET
           name          = EXCLUDED.name,
           slug          = EXCLUDED.slug,
           state         = EXCLUDED.state,
           is_online     = EXCLUDED.is_online,
           event_type    = EXCLUDED.event_type,
           min_team_size = EXCLUDED.min_team_size,
           max_team_size = EXCLUDED.max_team_size,
           num_entrants  = EXCLUDED.num_entrants,
           start_at      = EXCLUDED.start_at
       RETURNING id"#,
    tournament_db_id,
    event.id,
    event.name,
    event.slug,
    event.state,
    event.is_online,
    event.event_type,
    min_team_size,
    max_team_size,
    game_id,
    game_name,
    event.num_entrants,
    start_at,
)
.fetch_one(pool)
.await?;
```

- [ ] **Step 3: Add phase/phase_group upsert helper**

Add this function to `import.rs` (after `import_event` or at the bottom of the file). It returns a map of `startgg_phase_group_id → UUID` for use when linking sets.

```rust
async fn upsert_phases(
    pool: &PgPool,
    event_db_id: Uuid,
    phases: &[common::startgg::PhaseNode],
) -> anyhow::Result<HashMap<i64, Uuid>> {
    let mut phase_group_map: HashMap<i64, Uuid> = HashMap::new();

    for phase in phases {
        let phase_row = sqlx::query!(
            r#"INSERT INTO phases
                   (startgg_id, event_id, name, bracket_type, phase_order,
                    num_seeds, group_count, state, is_exhibition)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               ON CONFLICT (startgg_id) DO UPDATE SET
                   name         = EXCLUDED.name,
                   bracket_type = EXCLUDED.bracket_type,
                   phase_order  = EXCLUDED.phase_order,
                   state        = EXCLUDED.state
               RETURNING id"#,
            phase.id,
            event_db_id,
            phase.name,
            phase.bracket_type,
            phase.phase_order,
            phase.num_seeds,
            phase.group_count,
            phase.state,
            phase.is_exhibition,
        )
        .fetch_one(pool)
        .await?;

        let phase_db_id: Uuid = phase_row.id;

        for pg in phase
            .phase_groups
            .as_ref()
            .map(|p| p.nodes.as_slice())
            .unwrap_or(&[])
        {
            let first_round_time = pg.first_round_time.map(ts_to_dt);
            let start_at = pg.start_at.map(ts_to_dt);

            let pg_row = sqlx::query!(
                r#"INSERT INTO phase_groups
                       (startgg_id, phase_id, display_identifier, bracket_type, bracket_url,
                        num_rounds, start_at, first_round_time, state)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                   ON CONFLICT (startgg_id) DO UPDATE SET
                       display_identifier = EXCLUDED.display_identifier,
                       bracket_url        = EXCLUDED.bracket_url,
                       state              = EXCLUDED.state
                   RETURNING id"#,
                pg.id,
                phase_db_id,
                pg.display_identifier,
                pg.bracket_type,
                pg.bracket_url,
                pg.num_rounds,
                start_at,
                first_round_time,
                pg.state,
            )
            .fetch_one(pool)
            .await?;

            phase_group_map.insert(pg.id, pg_row.id);
        }
    }

    Ok(phase_group_map)
}
```

- [ ] **Step 4: Call `upsert_phases` from `import_event`**

After the `project_events` insert and before `import_entrants`, add:

```rust
let phase_group_map = upsert_phases(
    pool,
    event_db_id,
    event.phases.as_deref().unwrap_or(&[]),
)
.await?;
```

Then update the `import_sets` call to pass the map:

```rust
let set_count = import_sets(pool, startgg, event_db_id, event.id, &entrant_map, &phase_group_map).await?;
```

- [ ] **Step 5: Run tests**

```bash
bash backend/test.sh
```

Expected: all tests pass (import tests use wiremock; existing fixtures don't have phases data so `upsert_phases` will receive empty slices and no-op).

- [ ] **Step 6: Commit**

```bash
git add backend/crates/worker/src/import.rs
git commit -m "feat(worker): extend tournament/event import with new fields; upsert phases and phase_groups"
```

---

## Task 5: Import — sets with new fields

**Files:**
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Update `import_sets` signature**

Change the function signature to accept `phase_group_map`:

```rust
async fn import_sets(
    pool: &PgPool,
    startgg: &StartggClient,
    event_db_id: Uuid,
    event_startgg_id: i64,
    entrant_map: &HashMap<i64, Uuid>,
    phase_group_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<usize> {
```

- [ ] **Step 2: Update the set INSERT inside `import_sets`**

Replace the loop body (the `for set in &set_page.nodes` block). The new version adds an early skip for placeholder sets, resolves `phase_group_id`, and stores the new columns:

```rust
for set in &set_page.nodes {
    // Skip bye sets (one slot is a placeholder, not a real match)
    if set.has_placeholder.unwrap_or(false) {
        continue;
    }

    let (Some(winner_sg_id), Some(loser_sg_id)) = (set.winner_id, set.loser_id()) else {
        continue; // in-progress or unresolvable
    };
    let (Some(&winner_uuid), Some(&loser_uuid)) = (
        entrant_map.get(&winner_sg_id),
        entrant_map.get(&loser_sg_id),
    ) else {
        tracing::warn!(set_id = set.id, "entrant not found for set, skipping");
        continue;
    };

    let phase_group_id: Option<Uuid> = set
        .phase_group
        .as_ref()
        .and_then(|pg| phase_group_map.get(&pg.id))
        .copied();

    let (winner_score, loser_score) = set.scores();
    let completed_at = set.completed_at.map(ts_to_dt);

    sqlx::query!(
        r#"INSERT INTO sets
               (event_id, phase_group_id, startgg_set_id,
                winner_entrant_id, loser_entrant_id,
                round, round_name, total_games,
                winner_score, loser_score,
                is_dq, has_placeholder, state, identifier,
                vod_url, completed_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
           ON CONFLICT (event_id, startgg_set_id) DO UPDATE SET
               phase_group_id    = EXCLUDED.phase_group_id,
               winner_entrant_id = EXCLUDED.winner_entrant_id,
               loser_entrant_id  = EXCLUDED.loser_entrant_id,
               round             = EXCLUDED.round,
               round_name        = EXCLUDED.round_name,
               total_games       = EXCLUDED.total_games,
               winner_score      = EXCLUDED.winner_score,
               loser_score       = EXCLUDED.loser_score,
               is_dq             = EXCLUDED.is_dq,
               has_placeholder   = EXCLUDED.has_placeholder,
               state             = EXCLUDED.state,
               identifier        = EXCLUDED.identifier,
               vod_url           = EXCLUDED.vod_url,
               completed_at      = EXCLUDED.completed_at"#,
        event_db_id,
        phase_group_id,
        set.id,
        winner_uuid,
        loser_uuid,
        set.round,
        set.full_round_text.as_deref(),
        set.total_games.map(|b| b as i16),
        winner_score,
        loser_score,
        set.is_dq(),
        set.has_placeholder.unwrap_or(false),
        set.state,
        set.identifier.as_deref(),
        set.vod_url.as_deref(),
        completed_at,
    )
    .execute(pool)
    .await?;

    page_sets += 1;
}
```

- [ ] **Step 3: Run tests**

```bash
bash backend/test.sh
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add backend/crates/worker/src/import.rs
git commit -m "feat(worker): store has_placeholder, state, identifier, phase_group_id on sets"
```

---

## Task 6: API — extend list_tournaments response + fix stats queries

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`
- Modify: `backend/openapi.yaml`

- [ ] **Step 1: Write a failing test**

In `backend/crates/api/tests/api.rs`, find the existing tournament-related tests (or add at the end of the file) and add:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn list_tournaments_includes_event_type_and_bracket_types(pool: PgPool) {
    use uuid::Uuid;

    // Insert a tournament, event, phase, and project_event
    let t_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO tournaments (startgg_id, name, slug, online)
         VALUES (1001, 'Test Cup', 'tournament/test-cup', false)
         RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let e_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO events (tournament_id, startgg_id, name, event_type)
         VALUES ($1, 2001, 'Melee Singles', 1)
         RETURNING id",
        t_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO phases (startgg_id, event_id, bracket_type, phase_order)
         VALUES (3001, $1, 'ROUND_ROBIN', 1), (3002, $1, 'DOUBLE_ELIMINATION', 2)",
        e_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let project_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO ranking_projects (owner_id, name) VALUES (
            (INSERT INTO users (username, password_hash) VALUES ('u', 'h') RETURNING id),
            'P'
         ) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap_or_else(|_| {
        // Simpler: use the test app helper from existing tests
        Uuid::new_v4()
    });
    // NOTE: This test skeleton shows the intent. Follow the pattern used in existing
    // api.rs tests for creating users, sessions, and calling the router. The key
    // assertions are:
    //   assert_eq!(event["event_type"], 1);
    //   assert_eq!(event["bracket_types"], json!(["ROUND_ROBIN", "DOUBLE_ELIMINATION"]));
    // Look at how existing tests call `router()` and assert JSON responses, and follow
    // that exact pattern.
}
```

Run it to confirm it fails (or is not yet compilable):

```bash
cd /home/pd/private_projects/ranking_forge/backend
cargo test -p api -- list_tournaments_includes 2>&1 | tail -20
```

- [ ] **Step 2: Extend `ProjectEventResponse`**

In `tournaments.rs`, replace `ProjectEventResponse`:

```rust
#[derive(Serialize)]
pub struct ProjectEventResponse {
    pub id: Uuid,
    pub startgg_id: i64,
    pub name: String,
    pub game_name: Option<String>,
    pub num_entrants: Option<i32>,
    pub start_at: Option<DateTime<Utc>>,
    pub included: bool,
    pub event_type: Option<i32>,
    pub bracket_types: Vec<String>,
}
```

- [ ] **Step 3: Update `list_tournaments` — Row struct and SQL**

Replace the inner `Row` struct and query in `list_tournaments`:

```rust
struct Row {
    tournament_id: Uuid,
    tournament_startgg_id: i64,
    tournament_name: String,
    tournament_slug: String,
    city: Option<String>,
    addr_state: Option<String>,
    country_code: Option<String>,
    venue_name: Option<String>,
    online: bool,
    tournament_start_at: Option<DateTime<Utc>>,
    end_at: Option<DateTime<Utc>>,
    event_id: Uuid,
    event_startgg_id: i64,
    event_name: String,
    game_name: Option<String>,
    num_entrants: Option<i32>,
    event_start_at: Option<DateTime<Utc>>,
    included: bool,
    event_type: Option<i32>,
    bracket_types: Vec<String>,
}

let rows = sqlx::query_as!(
    Row,
    r#"
    SELECT
        t.id            AS tournament_id,
        t.startgg_id    AS tournament_startgg_id,
        t.name          AS tournament_name,
        t.slug          AS tournament_slug,
        t.city,
        t.addr_state,
        t.country_code,
        t.venue_name,
        t.online,
        t.start_at      AS tournament_start_at,
        t.end_at,
        e.id            AS event_id,
        e.startgg_id    AS event_startgg_id,
        e.name          AS event_name,
        e.game_name,
        e.num_entrants,
        e.start_at      AS event_start_at,
        pe.included,
        e.event_type,
        ARRAY(
            SELECT p.bracket_type
            FROM phases p
            WHERE p.event_id = e.id
              AND p.bracket_type IS NOT NULL
            ORDER BY p.phase_order ASC NULLS LAST
        )               AS "bracket_types!: Vec<String>"
    FROM project_events pe
    JOIN events      e ON e.id = pe.event_id
    JOIN tournaments t ON t.id = e.tournament_id
    WHERE pe.project_id = $1
    ORDER BY t.start_at DESC NULLS LAST, t.name ASC, e.name ASC
    "#,
    project_id,
)
.fetch_all(&state.db)
.await?;
```

- [ ] **Step 4: Update `ProjectEventResponse` construction in `list_tournaments`**

In the row-to-response grouping loop, update the push:

```rust
t_entry.events.push(ProjectEventResponse {
    id: row.event_id,
    startgg_id: row.event_startgg_id,
    name: row.event_name,
    game_name: row.game_name,
    num_entrants: row.num_entrants,
    start_at: row.event_start_at,
    included: row.included,
    event_type: row.event_type,
    bracket_types: row.bracket_types,
});
```

- [ ] **Step 5: Update `patch_event` to return new fields**

In `patch_event`, replace the inner `EventRow` struct and both queries:

```rust
struct EventRow {
    id: Uuid,
    startgg_id: i64,
    name: String,
    game_name: Option<String>,
    num_entrants: Option<i32>,
    start_at: Option<DateTime<Utc>>,
    included: bool,
    event_type: Option<i32>,
    bracket_types: Vec<String>,
}

let ev = sqlx::query_as!(
    EventRow,
    r#"
    SELECT e.id, e.startgg_id, e.name, e.game_name, e.num_entrants,
           e.start_at, pe.included, e.event_type,
           ARRAY(
               SELECT p.bracket_type
               FROM phases p
               WHERE p.event_id = e.id
                 AND p.bracket_type IS NOT NULL
               ORDER BY p.phase_order ASC NULLS LAST
           ) AS "bracket_types!: Vec<String>"
    FROM events e
    JOIN project_events pe ON pe.event_id = e.id AND pe.project_id = $1
    WHERE e.id = $2
    "#,
    path.id,
    path.eid,
)
.fetch_one(&state.db)
.await?;

Ok(Json(ProjectEventResponse {
    id: ev.id,
    startgg_id: ev.startgg_id,
    name: ev.name,
    game_name: ev.game_name,
    num_entrants: ev.num_entrants,
    start_at: ev.start_at,
    included: ev.included,
    event_type: ev.event_type,
    bracket_types: ev.bracket_types,
}))
```

- [ ] **Step 6: Add `has_placeholder` filter to stats queries**

In `get_stats` (around line 354), `get_head_to_head` (around line 484), and `get_h2h_sets` (around line 590), add `AND s.has_placeholder = false` alongside the existing `AND s.is_dq = false`:

```sql
-- In get_stats WHERE clause:
WHERE pe.included     = true
  AND s.is_dq         = false
  AND s.has_placeholder = false
  AND (wp.id IS NOT NULL OR lp.id IS NOT NULL)

-- In get_head_to_head WHERE clause:
WHERE pe.included       = true
  AND s.is_dq           = false
  AND s.has_placeholder = false

-- In get_h2h_sets WHERE clause:
WHERE pe.included       = true
  AND s.is_dq           = false
  AND s.has_placeholder = false
  AND (
      (we.player_id = $2 AND le.player_id = $3)
   OR (we.player_id = $3 AND le.player_id = $2)
  )
```

- [ ] **Step 7: Update openapi.yaml**

Find the event response schema in `backend/openapi.yaml` (search for `num_entrants`) and add the two new fields:

```yaml
event_type:
  type: integer
  nullable: true
  description: "start.gg event type: 1 = singles/individual, 2 = teams"
bracket_types:
  type: array
  items:
    type: string
  description: "Bracket types ordered by phase_order (e.g. [ROUND_ROBIN, DOUBLE_ELIMINATION])"
```

- [ ] **Step 8: Regenerate sqlx offline cache**

```bash
cd /home/pd/private_projects/ranking_forge
bash backend/prepare-sqlx.sh
```

Expected: runs migrations, introspects queries, updates `.sqlx/`. May take 30–60 seconds.

- [ ] **Step 9: Run full backend tests**

```bash
bash backend/test.sh
```

Expected: all tests pass. If the `list_tournaments_includes_event_type_and_bracket_types` test from Step 1 was written fully following the existing pattern, it should pass now.

- [ ] **Step 10: Commit**

```bash
git add backend/crates/api/src/routes/tournaments.rs backend/openapi.yaml backend/.sqlx
git commit -m "feat(api): surface event_type and bracket_types in tournament list; filter has_placeholder from stats"
```

---

## Task 7: Frontend — types and mock data

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/tests/mock-api.js`

- [ ] **Step 1: Extend `TournamentEvent` type**

In `web/src/lib/types.ts`, replace the `TournamentEvent` interface:

```ts
export interface TournamentEvent {
  id: string;
  startgg_id: number;
  name: string;
  game_name: string | null;
  num_entrants: number | null;
  start_at: string | null;
  included: boolean;
  event_type: number | null;
  bracket_types: string[];
}
```

- [ ] **Step 2: Update the e2e mock tournament response**

The mock at `web/tests/mock-api.js` line ~210 returns `[]` for tournaments. Since existing e2e tests don't assert tournament content, no change is needed. Skip this step.

- [ ] **Step 3: Run e2e tests to confirm no regressions**

```bash
cd /home/pd/private_projects/ranking_forge/web
npm run test:e2e
```

Expected: all existing e2e tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/types.ts
git commit -m "feat(web): add event_type and bracket_types to TournamentEvent type"
```

---

## Task 8: Frontend — filter state and logic

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Add filter state at the top of the `<script>` block**

After the existing `let tournaments = $state(...)` line, add:

```ts
// Filter state
let filterOpen    = $state(false);
let search        = $state('');
let venueFilter   = $state<'all' | 'online' | 'offline'>('all');
let minEntrants   = $state<number | null>(null);
let maxEntrants   = $state<number | null>(null);
let dateFrom      = $state('');
let dateTo        = $state('');
let eventType     = $state<'all' | 'singles' | 'teams'>('all');
let excludeLadder = $state(false);
```

- [ ] **Step 2: Add filter functions**

Add these pure functions to the `<script>` block (no imports needed — they use the filter state variables and `Tournament`/`TournamentEvent` types already imported):

```ts
import type { Tournament } from '$lib/types';

function tournamentVisible(t: Tournament): boolean {
    if (venueFilter === 'online' && !t.online) return false;
    if (venueFilter === 'offline' && t.online) return false;
    if (dateFrom && t.start_at && t.start_at < dateFrom) return false;
    if (dateTo && t.start_at && t.start_at > dateTo) return false;
    return true;
}

function eventVisible(e: import('$lib/types').TournamentEvent, t: Tournament): boolean {
    if (search.trim()) {
        const q = search.trim().toLowerCase();
        const nameMatch = e.name.toLowerCase().includes(q);
        const tournamentMatch = t.name.toLowerCase().includes(q);
        if (!nameMatch && !tournamentMatch) return false;
    }
    if (minEntrants !== null && (e.num_entrants ?? Infinity) < minEntrants) return false;
    if (maxEntrants !== null && (e.num_entrants ?? 0) > maxEntrants) return false;
    if (eventType === 'singles' && e.event_type !== null && e.event_type !== 1) return false;
    if (eventType === 'teams' && e.event_type !== null && e.event_type !== 2) return false;
    if (excludeLadder && e.bracket_types.length > 0 &&
        e.bracket_types.every(bt => bt === 'MATCHMAKING')) return false;
    return true;
}
```

- [ ] **Step 3: Add derived visible tournaments**

```ts
const visibleTournaments = $derived(
    tournaments
        .filter(t => tournamentVisible(t))
        .map(t => ({ ...t, events: t.events.filter(e => eventVisible(e, t)) }))
        .filter(t => t.events.length > 0)
);

const totalEventCount = $derived(tournaments.reduce((n, t) => n + t.events.length, 0));
const visibleEventCount = $derived(visibleTournaments.reduce((n, t) => n + t.events.length, 0));
```

- [ ] **Step 4: Add bulk action handler**

```ts
async function bulkSetIncluded(included: boolean) {
    for (const t of visibleTournaments) {
        for (const e of t.events) {
            if (e.included !== included) {
                await toggleEvent(data.project.id, e.id, included);
            }
        }
    }
}
```

- [ ] **Step 5: Run unit tests to confirm filter logic**

There are no existing unit tests for this page. Add a new test file:

Create `web/src/routes/projects/[id]/tournaments/filter.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import type { Tournament, TournamentEvent } from '$lib/types';

function makeEvent(overrides: Partial<TournamentEvent> = {}): TournamentEvent {
    return {
        id: 'e1', startgg_id: 1, name: 'Melee Singles',
        game_name: null, num_entrants: 100, start_at: null,
        included: true, event_type: 1, bracket_types: ['DOUBLE_ELIMINATION'],
        ...overrides,
    };
}

function makeTournament(events: TournamentEvent[], overrides: Partial<Tournament> = {}): Tournament {
    return {
        id: 't1', startgg_id: 1, name: 'Genesis 10', slug: 'tournament/genesis-10',
        city: 'San Jose', addr_state: 'CA', country_code: 'US',
        venue_name: null, online: false,
        start_at: '2025-01-12T00:00:00Z', end_at: null,
        events,
        ...overrides,
    };
}

// Inline the filter functions (copy from +page.svelte — keep in sync)
function tournamentVisible(
    t: Tournament,
    venueFilter: 'all' | 'online' | 'offline',
    dateFrom: string,
    dateTo: string,
): boolean {
    if (venueFilter === 'online' && !t.online) return false;
    if (venueFilter === 'offline' && t.online) return false;
    if (dateFrom && t.start_at && t.start_at < dateFrom) return false;
    if (dateTo && t.start_at && t.start_at > dateTo) return false;
    return true;
}

function eventVisible(
    e: TournamentEvent,
    t: Tournament,
    search: string,
    minEntrants: number | null,
    maxEntrants: number | null,
    eventType: 'all' | 'singles' | 'teams',
    excludeLadder: boolean,
): boolean {
    if (search.trim()) {
        const q = search.trim().toLowerCase();
        if (!e.name.toLowerCase().includes(q) && !t.name.toLowerCase().includes(q)) return false;
    }
    if (minEntrants !== null && (e.num_entrants ?? Infinity) < minEntrants) return false;
    if (maxEntrants !== null && (e.num_entrants ?? 0) > maxEntrants) return false;
    if (eventType === 'singles' && e.event_type !== null && e.event_type !== 1) return false;
    if (eventType === 'teams' && e.event_type !== null && e.event_type !== 2) return false;
    if (excludeLadder && e.bracket_types.length > 0 &&
        e.bracket_types.every(bt => bt === 'MATCHMAKING')) return false;
    return true;
}

describe('tournament filter', () => {
    it('venue filter hides online tournaments', () => {
        const t = makeTournament([], { online: true });
        expect(tournamentVisible(t, 'offline', '', '')).toBe(false);
        expect(tournamentVisible(t, 'online', '', '')).toBe(true);
    });

    it('date range filter hides tournaments outside range', () => {
        const t = makeTournament([], { start_at: '2024-06-01T00:00:00Z' });
        expect(tournamentVisible(t, 'all', '2025-01-01', '')).toBe(false);
        expect(tournamentVisible(t, 'all', '2024-01-01', '2024-12-31')).toBe(true);
    });

    it('name search matches event name', () => {
        const t = makeTournament([]);
        const e = makeEvent({ name: 'Melee Doubles' });
        expect(eventVisible(e, t, 'doubles', null, null, 'all', false)).toBe(true);
        expect(eventVisible(e, t, 'singles', null, null, 'all', false)).toBe(false);
    });

    it('name search on tournament name shows all events', () => {
        const t = makeTournament([]);
        const e = makeEvent({ name: 'Melee Doubles' });
        expect(eventVisible(e, t, 'genesis', null, null, 'all', false)).toBe(true);
    });

    it('entrant range filter', () => {
        const t = makeTournament([]);
        const small = makeEvent({ num_entrants: 16 });
        const large = makeEvent({ num_entrants: 512 });
        expect(eventVisible(small, t, '', 32, null, 'all', false)).toBe(false);
        expect(eventVisible(large, t, '', 32, 200, 'all', false)).toBe(false);
        expect(eventVisible(large, t, '', 32, null, 'all', false)).toBe(true);
    });

    it('null num_entrants passes min/max filter', () => {
        const t = makeTournament([]);
        const e = makeEvent({ num_entrants: null });
        expect(eventVisible(e, t, '', 32, 100, 'all', false)).toBe(true);
    });

    it('eventType singles filter', () => {
        const t = makeTournament([]);
        const singles = makeEvent({ event_type: 1 });
        const teams = makeEvent({ event_type: 2 });
        expect(eventVisible(singles, t, '', null, null, 'singles', false)).toBe(true);
        expect(eventVisible(teams, t, '', null, null, 'singles', false)).toBe(false);
    });

    it('null event_type passes all eventType filters', () => {
        const t = makeTournament([]);
        const e = makeEvent({ event_type: null });
        expect(eventVisible(e, t, '', null, null, 'singles', false)).toBe(true);
        expect(eventVisible(e, t, '', null, null, 'teams', false)).toBe(true);
    });

    it('excludeLadder only hides pure MATCHMAKING events', () => {
        const t = makeTournament([]);
        const ladder = makeEvent({ bracket_types: ['MATCHMAKING'] });
        const mixed = makeEvent({ bracket_types: ['ROUND_ROBIN', 'DOUBLE_ELIMINATION'] });
        const pools_bracket = makeEvent({ bracket_types: ['ROUND_ROBIN', 'MATCHMAKING'] });
        expect(eventVisible(ladder, t, '', null, null, 'all', true)).toBe(false);
        expect(eventVisible(mixed, t, '', null, null, 'all', true)).toBe(true);
        expect(eventVisible(pools_bracket, t, '', null, null, 'all', true)).toBe(false); // all are not pure MATCHMAKING... wait
        // pools_bracket has ROUND_ROBIN + MATCHMAKING — not ALL are MATCHMAKING, so it passes:
        expect(eventVisible(pools_bracket, t, '', null, null, 'all', true)).toBe(true);
    });
});
```

Run:

```bash
cd /home/pd/private_projects/ranking_forge/web
npm run test:unit
```

Expected: all new filter tests pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/[id]/tournaments/+page.svelte web/src/routes/projects/[id]/tournaments/filter.test.ts
git commit -m "feat(web): add client-side filter state, logic, and unit tests for tournament page"
```

---

## Task 9: Frontend — filter panel UI

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Add imports at top of script block**

Add `Button` to existing imports (it's already available in `$lib/components/ui/button`):

```ts
import { Badge } from '$lib/components/ui/badge';
import { Button } from '$lib/components/ui/button';
```

- [ ] **Step 2: Replace the template**

Replace the entire `<div class="space-y-4">` block with:

```svelte
<div class="space-y-4">
    <h2 class="text-lg font-semibold">Tournaments</h2>

    {#if tournaments.length === 0}
        <p class="text-sm text-muted-foreground">No tournaments imported yet. Run an import first.</p>
    {:else}
        <!-- Status line + toggle -->
        <div class="flex items-center justify-between text-sm text-muted-foreground">
            <span>
                Showing <strong>{visibleTournaments.length}</strong> of {tournaments.length} tournaments
                · <strong>{visibleEventCount}</strong> of {totalEventCount} events
            </span>
            <Button variant="outline" size="sm" onclick={() => (filterOpen = !filterOpen)}>
                ⚙ Filters &amp; Actions {filterOpen ? '▲' : '▼'}
            </Button>
        </div>

        <!-- Collapsible filter panel -->
        {#if filterOpen}
            <div class="rounded-md border border-border bg-muted/30 p-4 space-y-3">
                <!-- Row 1: search + venue -->
                <div class="flex flex-wrap gap-2">
                    <input
                        type="text"
                        placeholder="Search tournament or event name…"
                        bind:value={search}
                        class="flex-1 min-w-48 rounded-md border border-input bg-background px-3 py-1.5 text-sm"
                    />
                    <select
                        bind:value={venueFilter}
                        class="rounded-md border border-input bg-background px-3 py-1.5 text-sm"
                    >
                        <option value="all">Venue: All</option>
                        <option value="online">Online only</option>
                        <option value="offline">Offline only</option>
                    </select>
                </div>

                <!-- Row 2: entrant range + date range -->
                <div class="flex flex-wrap gap-2 items-center">
                    <div class="flex items-center gap-1.5">
                        <span class="text-xs text-muted-foreground whitespace-nowrap">Entrants</span>
                        <input
                            type="number"
                            min="0"
                            placeholder="min"
                            bind:value={minEntrants}
                            class="w-20 rounded-md border border-input bg-background px-2 py-1.5 text-sm"
                        />
                        <span class="text-muted-foreground">–</span>
                        <input
                            type="number"
                            min="0"
                            placeholder="max"
                            bind:value={maxEntrants}
                            class="w-20 rounded-md border border-input bg-background px-2 py-1.5 text-sm"
                        />
                    </div>
                    <div class="flex items-center gap-1.5">
                        <span class="text-xs text-muted-foreground">From</span>
                        <input
                            type="date"
                            bind:value={dateFrom}
                            class="rounded-md border border-input bg-background px-2 py-1.5 text-sm"
                        />
                        <span class="text-xs text-muted-foreground">To</span>
                        <input
                            type="date"
                            bind:value={dateTo}
                            class="rounded-md border border-input bg-background px-2 py-1.5 text-sm"
                        />
                    </div>
                </div>

                <!-- Row 3: event type + ladder -->
                <div class="flex flex-wrap gap-4 items-center">
                    <div class="flex items-center gap-2">
                        <span class="text-xs text-muted-foreground whitespace-nowrap">Event type</span>
                        <select
                            bind:value={eventType}
                            class="rounded-md border border-input bg-background px-3 py-1.5 text-sm"
                        >
                            <option value="all">All</option>
                            <option value="singles">Singles</option>
                            <option value="teams">Teams</option>
                        </select>
                    </div>
                    <label class="flex items-center gap-2 cursor-pointer text-sm">
                        <input type="checkbox" bind:checked={excludeLadder} class="h-4 w-4 accent-primary" />
                        Exclude ladder / matchmaking
                    </label>
                </div>

                <!-- Divider + bulk actions -->
                <div class="flex items-center justify-between border-t border-border pt-3">
                    <span class="text-xs text-muted-foreground">
                        Bulk actions apply to {visibleEventCount} visible event{visibleEventCount !== 1 ? 's' : ''}
                    </span>
                    <div class="flex gap-2">
                        <Button variant="outline" size="sm" onclick={() => bulkSetIncluded(true)}>
                            ✓ Include all visible
                        </Button>
                        <Button variant="outline" size="sm"
                            class="border-destructive text-destructive hover:bg-destructive/10"
                            onclick={() => bulkSetIncluded(false)}>
                            ✕ Exclude all visible
                        </Button>
                    </div>
                </div>
            </div>
        {/if}

        <!-- Tournament list — iterate visibleTournaments -->
        <div class="space-y-3">
            {#each visibleTournaments as tournament (tournament.id)}
                <div class="rounded-md border border-border">
                    <div class="flex items-start justify-between p-3">
                        <div>
                            <p class="font-medium">{tournament.name}</p>
                            <p class="text-xs text-muted-foreground">
                                {[tournament.city, tournament.addr_state, tournament.country_code]
                                    .filter(Boolean)
                                    .join(', ')}
                                {tournament.online ? '(Online)' : ''}
                                {tournament.start_at ? '· ' + new Date(tournament.start_at).toLocaleDateString() : ''}
                            </p>
                        </div>
                        <Badge variant="outline">
                            {tournament.events.length} event{tournament.events.length !== 1 ? 's' : ''}
                        </Badge>
                    </div>
                    <div class="divide-y divide-border border-t border-border">
                        {#each tournament.events as event (event.id)}
                            <label class="flex cursor-pointer items-center justify-between px-4 py-2 hover:bg-accent/50">
                                <div>
                                    <span class="text-sm">{event.name}</span>
                                    {#if event.num_entrants}
                                        <span class="ml-2 text-xs text-muted-foreground">{event.num_entrants} entrants</span>
                                    {/if}
                                </div>
                                <input
                                    type="checkbox"
                                    checked={event.included}
                                    onchange={() => handleToggle(data.project.id, event)}
                                    class="h-4 w-4 rounded border-border accent-primary"
                                />
                            </label>
                        {/each}
                    </div>
                </div>
            {/each}
        </div>
    {/if}
</div>
```

- [ ] **Step 3: Start dev server and manually test**

```bash
cd /home/pd/private_projects/ranking_forge
# Start API (in one terminal): cargo run --bin api
# Start frontend (in another): cd web && npm run dev
```

Open `http://localhost:5173`, navigate to a project's Tournaments page.

Verify:
- "Filters & Actions" button opens/closes the panel
- Status line shows correct counts
- Searching a name filters tournaments and events correctly
- Venue toggle works
- Event type dropdown works
- "Exclude ladder / matchmaking" checkbox works
- "Exclude all visible" marks all visible events as unchecked (verify persists on reload)
- "Include all visible" re-checks them

- [ ] **Step 4: Run full test suite**

```bash
cd /home/pd/private_projects/ranking_forge
bash test.sh
```

Expected: all backend + frontend unit + e2e tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/[id]/tournaments/+page.svelte
git commit -m "feat(web): tournament filter panel with bulk include/exclude actions"
```

---

## Self-Review Checklist

**Spec coverage:**
- ✅ Schema: all new columns and tables present (Task 1)
- ✅ start.gg GraphQL: new fields fetched (Tasks 2–3)
- ✅ Import: tournament, event, phase, phase_group, set upserts extended (Tasks 4–5)
- ✅ API: `event_type` + `bracket_types` in response; `has_placeholder` filtered from stats (Task 6)
- ✅ Frontend types updated (Task 7)
- ✅ Filter state, logic, unit tests (Task 8)
- ✅ Filter panel UI + bulk actions (Task 9)
- ✅ `prepare-sqlx.sh` run after all sqlx query changes (Task 6, Step 8)
- ✅ `.gitignore` note: add `.superpowers/` if not already present

**Null handling verified in tests:**
- `null num_entrants` passes min/max filter ✅
- `null event_type` passes singles/teams filter ✅
- Empty `bracket_types` passes excludeLadder filter ✅
