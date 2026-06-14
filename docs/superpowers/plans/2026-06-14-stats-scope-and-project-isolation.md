# Stats Scope Fix & Tournament Project Isolation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix stats endpoints to include non-member opponents, and make tournaments/events/phases/phase_groups project-scoped to eliminate cross-project `entrant.player_id` contamination.

**Architecture:** Edit `001_initial.sql` directly to add `project_id` to `tournaments` and replace global unique constraints with composite ones; update the import worker's five ON CONFLICT clauses; restore the pre-task-8 sets-based stats query with `Option<Uuid>` opponent_id; propagate the type change to the frontend and OpenAPI spec; regenerate the sqlx offline cache.

**Tech Stack:** Rust/sqlx (backend), SvelteKit/TypeScript (frontend), PostgreSQL (schema), openapi.yaml (API contract)

---

### Task 1: Schema — project-scope the tournament hierarchy

**Files:**
- Modify: `backend/migrations/001_initial.sql`
- Modify: `backend/crates/api/tests/api.rs` (lines 135–172, `seed_tournament_event` helper)

- [ ] **Step 1: Remove UNIQUE from `tournaments.startgg_id`, add `project_id` column**

In `backend/migrations/001_initial.sql`, replace the `tournaments` table definition:

```sql
CREATE TABLE tournaments (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id     UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    startgg_id     BIGINT      NOT NULL,
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

CREATE UNIQUE INDEX tournaments_project_startgg_idx ON tournaments(project_id, startgg_id);
CREATE INDEX tournaments_project_id_idx ON tournaments(project_id);
```

- [ ] **Step 2: Remove UNIQUE from `events.startgg_id`, add composite index**

In `001_initial.sql`, replace the `events` table definition (change `startgg_id BIGINT NOT NULL UNIQUE` → `startgg_id BIGINT NOT NULL`) and add index after the existing `CREATE INDEX events_*` lines:

```sql
CREATE TABLE events (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID        NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    startgg_id    BIGINT      NOT NULL,
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
CREATE UNIQUE INDEX events_tournament_startgg_idx ON events(tournament_id, startgg_id);
```

- [ ] **Step 3: Remove UNIQUE from `phases.startgg_id`, add composite index**

```sql
CREATE TABLE phases (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT      NOT NULL,
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
CREATE UNIQUE INDEX phases_event_startgg_idx ON phases(event_id, startgg_id);
```

- [ ] **Step 4: Remove UNIQUE from `phase_groups.startgg_id`, add composite index**

```sql
CREATE TABLE phase_groups (
    id                 UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id         BIGINT      NOT NULL,
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
CREATE UNIQUE INDEX phase_groups_phase_startgg_idx ON phase_groups(phase_id, startgg_id);
```

- [ ] **Step 5: Update `seed_tournament_event` test helper to supply `project_id`**

In `backend/crates/api/tests/api.rs`, replace `seed_tournament_event` (lines 135–172):

```rust
async fn seed_tournament_event(
    pool: &sqlx::PgPool,
    ranking_id: Uuid,
    startgg_tournament_id: i64,
    startgg_event_id: i64,
) -> (Uuid, Uuid) {
    let project_id: Uuid = sqlx::query_scalar!(
        "SELECT project_id FROM rankings WHERE id = $1",
        ranking_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let tournament_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO tournaments (project_id, startgg_id, name, handle, online)
         VALUES ($1, $2, 'Test Tournament', 'test-tournament', false)
         RETURNING id",
        project_id,
        startgg_tournament_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let event_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO events (tournament_id, startgg_id, name, handle)
         VALUES ($1, $2, 'Singles', 'singles')
         RETURNING id",
        tournament_id,
        startgg_event_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO ranking_events (ranking_id, event_id, included) VALUES ($1, $2, true)",
        ranking_id,
        event_id,
    )
    .execute(pool)
    .await
    .unwrap();

    (tournament_id, event_id)
}
```

- [ ] **Step 6: Verify the schema builds (worker will fail — expected)**

```bash
cd backend && cargo build -p api 2>&1 | head -40
```

Expected: compile errors in `worker` crate only (ON CONFLICT clauses reference the old `startgg_id` unique constraint). The `api` crate may also have sqlx offline cache mismatches — that is fine; the cache is regenerated in Task 6.

- [ ] **Step 7: Commit**

```bash
git add backend/migrations/001_initial.sql backend/crates/api/tests/api.rs
git commit -m "feat(schema): project-scope tournaments hierarchy; remove global startgg_id uniques"
```

---

### Task 2: Worker — update ON CONFLICT clauses

**Files:**
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Add `project_id` to the tournament INSERT**

In `import_tournament` (~line 296), replace the INSERT:

```rust
let row = sqlx::query!(
    r#"INSERT INTO tournaments
           (project_id, startgg_id, name, handle, city, addr_state, country_code,
            venue_name, venue_address, timezone, online, num_attendees,
            lat, lng, state, start_at, end_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
       ON CONFLICT (project_id, startgg_id) DO UPDATE SET
           name          = EXCLUDED.name,
           num_attendees = EXCLUDED.num_attendees,
           lat           = EXCLUDED.lat,
           lng           = EXCLUDED.lng,
           state         = EXCLUDED.state,
           start_at      = EXCLUDED.start_at,
           end_at        = EXCLUDED.end_at
       RETURNING id"#,
    project_id,             // $1  — NEW
    tournament.id,          // $2
    tournament.name,        // $3
    extract_tournament_handle(&tournament.slug), // $4
    tournament.city,        // $5
    tournament.addr_state,  // $6
    tournament.country_code, // $7
    tournament.venue_name,  // $8
    tournament.venue_address, // $9
    tournament.timezone,    // $10
    tournament.is_online.unwrap_or(false), // $11
    tournament.num_attendees, // $12
    tournament.lat,         // $13
    tournament.lng,         // $14
    tournament.state,       // $15
    start_at,               // $16
    end_at,                 // $17
)
.fetch_one(pool)
.await?;
```

- [ ] **Step 2: Update events ON CONFLICT**

In `import_event` (~line 377), change the conflict target:

```rust
ON CONFLICT (tournament_id, startgg_id) DO UPDATE SET
    name          = EXCLUDED.name,
    handle        = EXCLUDED.handle,
    state         = EXCLUDED.state,
    is_online     = EXCLUDED.is_online,
    event_type    = EXCLUDED.event_type,
    min_team_size = EXCLUDED.min_team_size,
    max_team_size = EXCLUDED.max_team_size,
    num_entrants  = EXCLUDED.num_entrants,
    start_at      = EXCLUDED.start_at
```

- [ ] **Step 3: Update phases and phase_groups ON CONFLICT**

In `upsert_phases` (~line 543), change:

```rust
// phases
ON CONFLICT (event_id, startgg_id) DO UPDATE SET
    name         = EXCLUDED.name,
    bracket_type = EXCLUDED.bracket_type,
    phase_order  = EXCLUDED.phase_order,
    state        = EXCLUDED.state

// phase_groups (inner loop, ~line 578)
ON CONFLICT (phase_id, startgg_id) DO UPDATE SET
    display_identifier = EXCLUDED.display_identifier,
    bracket_url        = EXCLUDED.bracket_url,
    num_rounds         = EXCLUDED.num_rounds,
    state              = EXCLUDED.state
```

- [ ] **Step 4: Build the workspace**

```bash
cd backend && cargo build --workspace 2>&1 | grep -E "^error"
```

Expected: no errors. (sqlx offline cache mismatches are warnings, not errors, when `SQLX_OFFLINE=true` is not set and a DB is not available — the build still succeeds at this stage.)

- [ ] **Step 5: Commit**

```bash
git add backend/crates/worker/src/import.rs
git commit -m "feat(worker): update tournament/event/phase ON CONFLICT for project-scoped schema"
```

---

### Task 3: Stats endpoints — restore sets-based query

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Update the failing test to assert correct behavior**

In `backend/crates/api/tests/api.rs`, find `stats_includes_non_project_opponent` (~line 1615) and replace its assertions:

```rust
// Replace the final assertions (everything after compute_ranking_set_results call):
let resp = get_req(
    &app,
    &format!("/projects/{pid_str}/rankings/{rid}/stats"),
    &cookie,
)
.await;
assert_eq!(resp.status(), StatusCode::OK);

let stats = read_json(resp).await;
let entries = stats.as_array().unwrap();
assert_eq!(entries.len(), 1, "only ranking players in outer list");

let alice = entries.iter().find(|s| s["name"] == "Alice").unwrap();
assert_eq!(alice["wins"], json!([]));
assert_eq!(
    alice["losses"].as_array().unwrap().len(),
    1,
    "Alice's loss to Outsider should appear"
);
assert_eq!(alice["losses"][0]["opponent_name"], "Outsider");
assert!(
    alice["losses"][0]["opponent_id"].is_null(),
    "non-ranking opponent has no player id"
);
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd backend && cargo test -p api -- stats_includes_non_project_opponent 2>&1 | tail -20
```

Expected: FAIL — Alice currently has no losses because the query reads from `ranking_set_results` which excludes non-member sets.

- [ ] **Step 3: Change `SetRecord.opponent_id` to `Option<Uuid>`**

In `backend/crates/api/src/routes/tournaments.rs`, change the `SetRecord` struct (line 72):

```rust
pub struct SetRecord {
    pub opponent_id: Option<Uuid>,   // None when opponent is not a ranking member
    pub opponent_name: String,
    // ... all other fields unchanged ...
}
```

- [ ] **Step 4: Rewrite `get_stats`**

Replace the `RsrRow` struct and query in `get_stats` with a `SetRow` + sets-based query. The full replacement (from `struct RsrRow` through the `fetch_all` call, ~lines 314–390):

```rust
struct SetRow {
    winner_player_id: Option<Uuid>,
    winner_name: String,
    winner_seed: Option<i32>,
    loser_player_id: Option<Uuid>,
    loser_name: String,
    loser_seed: Option<i32>,
    winner_score: Option<i16>,
    loser_score: Option<i16>,
    round_name: Option<String>,
    is_dq: bool,
    vod_url: Option<String>,
    startgg_set_id: i64,
    winner_placement: Option<i32>,
    loser_placement: Option<i32>,
    event_name: String,
    event_handle: Option<String>,
    num_entrants: Option<i32>,
    tournament_name: String,
    tournament_handle: String,
    online: bool,
    city: Option<String>,
    addr_state: Option<String>,
    country_code: Option<String>,
    phase_name: Option<String>,
    pool_identifier: Option<String>,
    completed_at: Option<DateTime<Utc>>,
}

let rows = sqlx::query_as!(
    SetRow,
    r#"
    SELECT
        we.player_id                        AS "winner_player_id?: Uuid",
        COALESCE(wp.name, we.display_name)  AS "winner_name!",
        we.seed                             AS winner_seed,
        le.player_id                        AS "loser_player_id?: Uuid",
        COALESCE(lp.name, le.display_name)  AS "loser_name!",
        le.seed                             AS loser_seed,
        s.winner_score,
        s.loser_score,
        s.round_name,
        s.is_dq,
        s.vod_url,
        s.startgg_set_id,
        we.final_placement                  AS winner_placement,
        le.final_placement                  AS loser_placement,
        e.name                              AS event_name,
        e.handle                            AS "event_handle?: String",
        e.num_entrants,
        t.name                              AS tournament_name,
        t.handle                            AS tournament_handle,
        t.online,
        t.city,
        t.addr_state,
        t.country_code,
        ph.name                             AS "phase_name?: String",
        pg.display_identifier               AS "pool_identifier?: String",
        s.completed_at
    FROM sets s
    JOIN entrants we ON we.id = s.winner_entrant_id
    JOIN entrants le ON le.id = s.loser_entrant_id
    JOIN ranking_events re ON re.event_id = s.event_id AND re.ranking_id = $1
    JOIN events e ON e.id = s.event_id
    JOIN tournaments t ON t.id = e.tournament_id
    LEFT JOIN ranking_players rwp ON rwp.player_id = we.player_id AND rwp.ranking_id = $1
    LEFT JOIN players wp ON wp.id = rwp.player_id
    LEFT JOIN ranking_players rlp ON rlp.player_id = le.player_id AND rlp.ranking_id = $1
    LEFT JOIN players lp ON lp.id = rlp.player_id
    LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
    LEFT JOIN phases ph ON ph.id = pg.phase_id
    WHERE re.included = true
      AND s.is_dq = false
      AND s.has_placeholder = false
      AND (rwp.player_id IS NOT NULL OR rlp.player_id IS NOT NULL)
    "#,
    path.rid,
)
.fetch_all(&state.db)
.await?;
```

- [ ] **Step 5: Rewrite the `get_stats` processing loop**

Replace the loop and `make_record` closure (~lines 398–436):

```rust
for row in rows {
    let uf = match (row.winner_seed, row.loser_seed) {
        (Some(ws), Some(ls)) => set_upset_factor(ws, ls) as i64,
        _ => 0,
    };
    let location = compute_location(
        row.online,
        row.city.as_deref(),
        row.addr_state.as_deref(),
        row.country_code.as_deref(),
    );
    let make_record = |opponent_id: Option<Uuid>, opponent_name: String| SetRecord {
        opponent_id,
        opponent_name,
        upset_factor: uf,
        winner_score: row.winner_score,
        loser_score: row.loser_score,
        tournament_name: row.tournament_name.clone(),
        tournament_handle: row.tournament_handle.clone(),
        event_name: row.event_name.clone(),
        round_name: row.round_name.clone(),
        completed_at: row.completed_at,
        is_dq: row.is_dq,
        vod_url: row.vod_url.clone(),
        startgg_set_id: row.startgg_set_id,
        winner_seed: row.winner_seed,
        loser_seed: row.loser_seed,
        phase_name: row.phase_name.clone(),
        pool_identifier: row.pool_identifier.clone(),
        winner_placement: row.winner_placement,
        loser_placement: row.loser_placement,
        location: location.clone(),
        num_entrants: row.num_entrants,
        event_handle: row.event_handle.clone(),
    };
    if let Some(winner_id) = row.winner_player_id {
        if let Some(entry) = stats.get_mut(&winner_id) {
            entry.1.push(make_record(row.loser_player_id, row.loser_name.clone()));
        }
    }
    if let Some(loser_id) = row.loser_player_id {
        if let Some(entry) = stats.get_mut(&loser_id) {
            entry.2.push(make_record(row.winner_player_id, row.winner_name.clone()));
        }
    }
}
```

- [ ] **Step 6: Rewrite `get_player_stats`**

Replace the `RsrRow` struct, query, and processing loop in `get_player_stats` (~lines 476–596). Use the same `SetRow` struct (define it once above both functions if they share a module, or define it locally in each). The query adds `AND (we.player_id = $2 OR le.player_id = $2)`:

```rust
struct SetRow {
    winner_player_id: Option<Uuid>,
    winner_name: String,
    winner_seed: Option<i32>,
    loser_player_id: Option<Uuid>,
    loser_name: String,
    loser_seed: Option<i32>,
    winner_score: Option<i16>,
    loser_score: Option<i16>,
    round_name: Option<String>,
    is_dq: bool,
    vod_url: Option<String>,
    startgg_set_id: i64,
    winner_placement: Option<i32>,
    loser_placement: Option<i32>,
    event_name: String,
    event_handle: Option<String>,
    num_entrants: Option<i32>,
    tournament_name: String,
    tournament_handle: String,
    online: bool,
    city: Option<String>,
    addr_state: Option<String>,
    country_code: Option<String>,
    phase_name: Option<String>,
    pool_identifier: Option<String>,
    completed_at: Option<DateTime<Utc>>,
}

let rows = sqlx::query_as!(
    SetRow,
    r#"
    SELECT
        we.player_id                        AS "winner_player_id?: Uuid",
        COALESCE(wp.name, we.display_name)  AS "winner_name!",
        we.seed                             AS winner_seed,
        le.player_id                        AS "loser_player_id?: Uuid",
        COALESCE(lp.name, le.display_name)  AS "loser_name!",
        le.seed                             AS loser_seed,
        s.winner_score,
        s.loser_score,
        s.round_name,
        s.is_dq,
        s.vod_url,
        s.startgg_set_id,
        we.final_placement                  AS winner_placement,
        le.final_placement                  AS loser_placement,
        e.name                              AS event_name,
        e.handle                            AS "event_handle?: String",
        e.num_entrants,
        t.name                              AS tournament_name,
        t.handle                            AS tournament_handle,
        t.online,
        t.city,
        t.addr_state,
        t.country_code,
        ph.name                             AS "phase_name?: String",
        pg.display_identifier               AS "pool_identifier?: String",
        s.completed_at
    FROM sets s
    JOIN entrants we ON we.id = s.winner_entrant_id
    JOIN entrants le ON le.id = s.loser_entrant_id
    JOIN ranking_events re ON re.event_id = s.event_id AND re.ranking_id = $1
    JOIN events e ON e.id = s.event_id
    JOIN tournaments t ON t.id = e.tournament_id
    LEFT JOIN ranking_players rwp ON rwp.player_id = we.player_id AND rwp.ranking_id = $1
    LEFT JOIN players wp ON wp.id = rwp.player_id
    LEFT JOIN ranking_players rlp ON rlp.player_id = le.player_id AND rlp.ranking_id = $1
    LEFT JOIN players lp ON lp.id = rlp.player_id
    LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
    LEFT JOIN phases ph ON ph.id = pg.phase_id
    WHERE re.included = true
      AND s.is_dq = false
      AND s.has_placeholder = false
      AND (rwp.player_id IS NOT NULL OR rlp.player_id IS NOT NULL)
      AND (we.player_id = $2 OR le.player_id = $2)
    "#,
    path.rid,
    path.player_id,
)
.fetch_all(&state.db)
.await?;

let mut wins: Vec<SetRecord> = Vec::new();
let mut losses: Vec<SetRecord> = Vec::new();

for row in rows {
    let uf = match (row.winner_seed, row.loser_seed) {
        (Some(ws), Some(ls)) => set_upset_factor(ws, ls) as i64,
        _ => 0,
    };
    let location = compute_location(
        row.online,
        row.city.as_deref(),
        row.addr_state.as_deref(),
        row.country_code.as_deref(),
    );
    let rec = |opponent_id: Option<Uuid>, opponent_name: String| SetRecord {
        opponent_id,
        opponent_name,
        upset_factor: uf,
        winner_score: row.winner_score,
        loser_score: row.loser_score,
        tournament_name: row.tournament_name.clone(),
        tournament_handle: row.tournament_handle.clone(),
        event_name: row.event_name.clone(),
        round_name: row.round_name.clone(),
        completed_at: row.completed_at,
        is_dq: row.is_dq,
        vod_url: row.vod_url.clone(),
        startgg_set_id: row.startgg_set_id,
        winner_seed: row.winner_seed,
        loser_seed: row.loser_seed,
        phase_name: row.phase_name.clone(),
        pool_identifier: row.pool_identifier.clone(),
        winner_placement: row.winner_placement,
        loser_placement: row.loser_placement,
        location,
        num_entrants: row.num_entrants,
        event_handle: row.event_handle.clone(),
    };
    if row.winner_player_id == Some(path.player_id) {
        wins.push(rec(row.loser_player_id, row.loser_name));
    } else {
        losses.push(rec(row.winner_player_id, row.winner_name));
    }
}
```

- [ ] **Step 7: Run the target test**

```bash
cd backend && cargo test -p api -- stats_includes_non_project_opponent 2>&1 | tail -20
```

Expected: PASS.

- [ ] **Step 8: Run the full api test suite**

```bash
cd backend && cargo test -p api 2>&1 | tail -30
```

Expected: all tests pass. If any stats test fails because it asserts `opponent_id` equals a specific string value (the old non-null UUID), update that assertion to match the new nullable behavior — ranking-member opponents still return `Some(uuid)`, serialized as a JSON string.

- [ ] **Step 9: Commit**

```bash
git add backend/crates/api/src/routes/tournaments.rs backend/crates/api/tests/api.rs
git commit -m "fix(api): restore sets-based stats queries; opponent_id nullable for non-ranking opponents"
```

---

### Task 4: Frontend — null-guard `opponent_id`

**Files:**
- Modify: `web/src/lib/types.ts` (line 134)
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/stats/+page.svelte` (lines 117–119)
- Modify: `web/src/routes/projects/[id]/(editor)/players/[player_id]/+page.svelte` (lines 153–156)

- [ ] **Step 1: Update `SetRecord` type**

In `web/src/lib/types.ts`, change line 134:

```ts
opponent_id: string | null;
```

- [ ] **Step 2: Add null guard in stats page**

In `web/src/routes/projects/[id]/rankings/[rid]/stats/+page.svelte`, replace lines 117–119:

```svelte
opponentPlayerId={selectedSet &&
  selectedSet.opponent_id !== null &&
  trackedPlayerIds.has(selectedSet.opponent_id)
  ? selectedSet.opponent_id
  : undefined}
```

- [ ] **Step 3: Add null guard in player page**

In `web/src/routes/projects/[id]/(editor)/players/[player_id]/+page.svelte`, replace lines 153–156:

```svelte
opponentPlayerId={selectedSet &&
  selectedSet.opponent_id !== null &&
  data.trackedPlayerIds.has(selectedSet.opponent_id)
  ? selectedSet.opponent_id
  : undefined}
```

- [ ] **Step 4: Run frontend unit tests**

```bash
cd web && npm run test:unit 2>&1 | tail -20
```

Expected: all tests pass. The existing `SetDetailModal.test.ts` fixture uses `opponent_id: "p2"` (a string), which is still valid for `string | null`. The `stats.test.ts` fixture uses `opponent_id: "opp"`, also still valid.

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/types.ts \
        "web/src/routes/projects/[id]/rankings/[rid]/stats/+page.svelte" \
        "web/src/routes/projects/[id]/(editor)/players/[player_id]/+page.svelte"
git commit -m "fix(web): null-guard opponent_id — non-ranking opponents return null, not a UUID"
```

---

### Task 5: OpenAPI spec + bug report cleanup

**Files:**
- Modify: `backend/openapi.yaml` (lines 281–288)
- Delete: `bug-stats-scope.md`

- [ ] **Step 1: Update SetRecord in openapi.yaml**

Replace lines 281–288:

```yaml
    SetRecord:
      type: object
      required: [opponent_id, opponent_name, upset_factor, winner_score, loser_score, tournament_name, tournament_handle, event_name, round_name, completed_at, is_dq, vod_url, startgg_set_id, winner_seed, loser_seed]
      properties:
        opponent_id:
          type: string
          format: uuid
          nullable: true
          description: >
            UUID of the opponent's player record if they are a member of this ranking;
            null for opponents who are not ranking members.
```

- [ ] **Step 2: Delete the resolved bug report**

```bash
rm bug-stats-scope.md
```

- [ ] **Step 3: Commit**

```bash
git add backend/openapi.yaml
git rm bug-stats-scope.md
git commit -m "docs: update openapi SetRecord.opponent_id nullable; remove resolved bug report"
```

---

### Task 6: Regenerate sqlx offline cache and run full test suite

**Files:**
- Regenerate: `backend/.sqlx/` (all files replaced by `prepare-sqlx.sh`)

- [ ] **Step 1: Regenerate the sqlx offline cache**

```bash
bash backend/prepare-sqlx.sh
```

Expected: exits 0. The seven `.sqlx/` files deleted in the working tree are replaced. New files are created for the rewritten stats queries. This requires Docker (spins up a temporary Postgres container).

- [ ] **Step 2: Run the full test suite**

```bash
bash test.sh
```

Expected: all sections PASS (backend, frontend unit, frontend e2e).

- [ ] **Step 3: Commit the updated cache**

```bash
git add backend/.sqlx/
git commit -m "chore: regenerate sqlx offline query cache after stats query rewrite"
```
