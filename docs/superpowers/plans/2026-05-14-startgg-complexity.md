# start.gg Query Complexity Handling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect start.gg query complexity errors as a distinct error type, restructure the `TOURNAMENTS_BY_USER_QUERY` to avoid the root cause, add a new paginated `event_phases` operation, and add complexity-retry logic to all paginated import loops.

**Architecture:** Three composable changes: (1) `StartggError::ComplexityTooHigh` is parsed from GraphQL error messages in `gql()` using a compiled regex; (2) phases are removed from `TOURNAMENTS_BY_USER_QUERY` and fetched via a new `event_phases` method that internally paginates phase groups and handles complexity retries; (3) the three paginated loops in `import.rs` gain a complexity arm that halves `per_page` and restarts from page 1 via a labeled `'pages: loop`.

**Tech Stack:** Rust, `regex` crate (new), `backon` (existing), `wiremock` (test mocks), `sqlx` (no new queries)

---

### Task 1: Add `ComplexityTooHigh` error variant and detection

**Files:**
- Modify: `backend/crates/common/Cargo.toml`
- Modify: `backend/crates/common/src/startgg/mod.rs`

- [ ] **Step 1: Write two failing tests**

Add these two tests inside the existing `#[cfg(test)] mod tests` block in `backend/crates/common/src/startgg/mod.rs`, after the existing `rate_limited_multiple_times_then_succeeds` test:

```rust
#[tokio::test]
async fn complexity_error_is_parsed_as_complexity_too_high() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": null,
            "errors": [{"message": "Your query complexity is too high. A maximum of 1000 objects may be returned by each request. (actual: 1203)"}]
        })))
        .mount(&mock)
        .await;

    let err = client(&mock.uri()).search_games("melee").await.unwrap_err();
    assert!(
        matches!(err, StartggError::ComplexityTooHigh { limit: 1000, actual: 1203 }),
        "expected ComplexityTooHigh, got {err:?}"
    );
}

#[tokio::test]
async fn non_complexity_graphql_error_surfaces_as_graphql() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": null,
            "errors": [{"message": "not authorized"}]
        })))
        .mount(&mock)
        .await;

    let err = client(&mock.uri()).search_games("melee").await.unwrap_err();
    assert!(
        matches!(err, StartggError::GraphQL(_)),
        "expected GraphQL error, got {err:?}"
    );
}
```

- [ ] **Step 2: Add `regex` dependency**

Run from `backend/`:
```bash
cargo add regex -p common
```

- [ ] **Step 3: Add `ComplexityTooHigh` variant, `parse_complexity_error`, and update `gql()`**

In `backend/crates/common/src/startgg/mod.rs`, replace the `StartggError` enum and the `gql` method error-handling block:

Replace the existing `StartggError` enum:
```rust
#[derive(Debug, Error)]
pub enum StartggError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("GraphQL error: {0}")]
    GraphQL(String),
    #[error("query complexity too high (limit: {limit}, actual: {actual})")]
    ComplexityTooHigh { limit: u32, actual: u32 },
    #[error("response decode error: {0}")]
    Decode(String),
}
```

Add `parse_complexity_error` as a free function, directly above the `impl StartggClient` block:
```rust
fn parse_complexity_error(msg: &str) -> Option<StartggError> {
    use regex::Regex;
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"A maximum of (\d+) objects may be returned.*\(actual: (\d+)\)").unwrap()
    });
    let caps = re.captures(msg)?;
    let limit = caps[1].parse::<u32>().ok()?;
    let actual = caps[2].parse::<u32>().ok()?;
    Some(StartggError::ComplexityTooHigh { limit, actual })
}
```

In `gql()`, replace the error-handling block (the `if let Some(errors) = resp.errors` section):
```rust
if let Some(errors) = resp.errors {
    let msg = errors
        .into_iter()
        .map(|e| e.message)
        .collect::<Vec<_>>()
        .join("; ");
    if let Some(err) = parse_complexity_error(&msg) {
        return Err(err);
    }
    tracing::error!(body = %body, "start.gg returned GraphQL errors: {msg}");
    return Err(StartggError::GraphQL(msg));
}
```

- [ ] **Step 4: Run the two new tests and verify they pass**

```bash
cd backend && cargo test -p common -- complexity_error_is_parsed non_complexity_graphql
```

Expected: both tests pass. Also verify the full common test suite still passes:

```bash
cd backend && cargo test -p common
```

Expected: all tests pass, no regressions.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/common/Cargo.toml backend/crates/common/src/startgg/mod.rs backend/Cargo.lock
git commit -m "feat(common): add StartggError::ComplexityTooHigh with regex detection"
```

---

### Task 2: Add `event_phases` query and operation

**Files:**
- Modify: `backend/crates/common/src/startgg/queries.rs`
- Modify: `backend/crates/common/src/startgg/operations.rs`
- Modify: `backend/crates/common/src/startgg/mod.rs` (tests only)

- [ ] **Step 1: Write a failing test for `event_phases`**

Add this test inside the `#[cfg(test)] mod tests` block in `backend/crates/common/src/startgg/mod.rs`:

```rust
#[tokio::test]
async fn event_phases_merges_paginated_phase_groups() {
    let mock = MockServer::start().await;

    // Page 1 of phase groups: totalPages = 2
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "event": {
                    "phases": [{
                        "id": 1,
                        "name": "Pools",
                        "bracketType": "DOUBLE_ELIMINATION",
                        "phaseOrder": 1,
                        "numSeeds": null,
                        "groupCount": 2,
                        "state": "COMPLETED",
                        "isExhibition": false,
                        "phaseGroups": {
                            "pageInfo": { "total": 2, "totalPages": 2 },
                            "nodes": [
                                { "id": 100, "displayIdentifier": "1", "bracketType": "DOUBLE_ELIMINATION",
                                  "bracketUrl": null, "numRounds": null, "startAt": null,
                                  "firstRoundTime": null, "state": 3 }
                            ]
                        }
                    }]
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Page 2 of phase groups: totalPages = 2
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "event": {
                    "phases": [{
                        "id": 1,
                        "name": "Pools",
                        "bracketType": "DOUBLE_ELIMINATION",
                        "phaseOrder": 1,
                        "numSeeds": null,
                        "groupCount": 2,
                        "state": "COMPLETED",
                        "isExhibition": false,
                        "phaseGroups": {
                            "pageInfo": { "total": 2, "totalPages": 2 },
                            "nodes": [
                                { "id": 101, "displayIdentifier": "2", "bracketType": "DOUBLE_ELIMINATION",
                                  "bracketUrl": null, "numRounds": null, "startAt": null,
                                  "firstRoundTime": null, "state": 3 }
                            ]
                        }
                    }]
                }
            }
        })))
        .mount(&mock)
        .await;

    let phases = client(&mock.uri()).event_phases(200).await.unwrap();
    assert_eq!(phases.len(), 1);
    let groups = phases[0].phase_groups.as_ref().unwrap();
    assert_eq!(groups.nodes.len(), 2, "expected groups from both pages merged");
    assert_eq!(groups.nodes[0].id, 100);
    assert_eq!(groups.nodes[1].id, 101);
}
```

- [ ] **Step 2: Run the test to confirm it fails to compile**

```bash
cd backend && cargo test -p common -- event_phases 2>&1 | head -20
```

Expected: compile error — `event_phases` method does not exist.

- [ ] **Step 3: Update `PhaseGroupPage` to include `pageInfo` in `queries.rs`**

In `backend/crates/common/src/startgg/queries.rs`, replace the existing `PhaseGroupPage` struct:

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhaseGroupPage {
    pub page_info: Option<PageInfo>,
    pub nodes: Vec<PhaseGroupNode>,
}
```

- [ ] **Step 4: Add `EventPhasesVars`, `EventPhasesData`, and `EventWithPhases` to `queries.rs`**

Add these types at the end of `backend/crates/common/src/startgg/queries.rs`:

```rust
// ── Event phases ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct EventPhasesVars {
    #[serde(rename = "eventId")]
    pub event_id: i64,
    pub page: i32,
    #[serde(rename = "perPage")]
    pub per_page: i32,
}

#[derive(Deserialize)]
pub(crate) struct EventPhasesData {
    pub event: Option<EventWithPhases>,
}

#[derive(Deserialize)]
pub(crate) struct EventWithPhases {
    pub phases: Vec<PhaseNode>,
}
```

- [ ] **Step 5: Add `EVENT_PHASES_QUERY` and `event_phases` to `operations.rs`**

In `backend/crates/common/src/startgg/operations.rs`, add the import to the existing `use super::queries::{...}` block:

```rust
use super::queries::{
    EntrantPage, EventEntrantsData, EventEntrantsVars, EventPhasesData, EventPhasesVars,
    EventSetsData, EventSetsVars, GameNode, GameSearchData, GameSearchVars, PhaseNode, SetPage,
    TournamentPage, TournamentsByUserData, TournamentsByUserVars, UserBySlugData, UserBySlugVars,
    UserNode,
};
```

Add the query constant after `EVENT_SETS_QUERY`:

```rust
const EVENT_PHASES_QUERY: &str = r#"
    query($eventId: ID!, $page: Int!, $perPage: Int!) {
        event(id: $eventId) {
            phases {
                id name bracketType phaseOrder
                numSeeds groupCount state isExhibition
                phaseGroups(query: { page: $page, perPage: $perPage }) {
                    pageInfo { total totalPages }
                    nodes {
                        id displayIdentifier bracketType bracketUrl
                        numRounds startAt firstRoundTime state
                    }
                }
            }
        }
    }"#;
```

Add the `event_phases` method inside `impl StartggClient`, after `event_sets`:

```rust
#[instrument(skip(self))]
pub async fn event_phases(&self, event_id: i64) -> Result<Vec<PhaseNode>, StartggError> {
    let t = Instant::now();
    let mut per_page = 25i32;

    let result = 'pages: loop {
        let mut phases_acc: Vec<PhaseNode> = Vec::new();
        let mut page = 1i32;

        loop {
            let data: EventPhasesData = match self
                .gql(EVENT_PHASES_QUERY, EventPhasesVars { event_id, page, per_page })
                .await
            {
                Err(StartggError::ComplexityTooHigh { actual, limit }) if per_page > 1 => {
                    tracing::warn!(per_page, actual, limit, "complexity too high, halving perPage");
                    per_page /= 2;
                    continue 'pages;
                }
                other => other?,
            };

            let phases = data.event.map(|e| e.phases).unwrap_or_default();

            let max_total_pages = phases
                .iter()
                .filter_map(|p| p.phase_groups.as_ref())
                .filter_map(|pg| pg.page_info.as_ref())
                .filter_map(|pi| pi.total_pages)
                .max()
                .unwrap_or(1);

            if page == 1 {
                phases_acc = phases;
            } else {
                for phase in phases {
                    if let Some(existing) = phases_acc.iter_mut().find(|p| p.id == phase.id) {
                        if let (Some(existing_pg), Some(new_pg)) =
                            (existing.phase_groups.as_mut(), phase.phase_groups)
                        {
                            existing_pg.nodes.extend(new_pg.nodes);
                        }
                    }
                }
            }

            if page >= max_total_pages {
                break 'pages phases_acc;
            }
            page += 1;
        }
    };

    tracing::debug!(elapsed_ms = t.elapsed().as_millis(), "startgg query complete");
    Ok(result)
}
```

- [ ] **Step 6: Run the new test and verify it passes**

```bash
cd backend && cargo test -p common -- event_phases
```

Expected: `event_phases_merges_paginated_phase_groups` passes.

Run the full suite to check for regressions:

```bash
cd backend && cargo test -p common
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/common/src/startgg/queries.rs backend/crates/common/src/startgg/operations.rs backend/crates/common/src/startgg/mod.rs
git commit -m "feat(common): add event_phases operation with paginated phase groups"
```

---

### Task 3: Trim `TOURNAMENTS_BY_USER_QUERY` and update `import_event`

**Files:**
- Modify: `backend/crates/common/src/startgg/queries.rs`
- Modify: `backend/crates/common/src/startgg/operations.rs`
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Remove `phases` from `EventNode` in `queries.rs`**

In `backend/crates/common/src/startgg/queries.rs`, replace the `EventNode` struct (remove the `phases` field):

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
}
```

- [ ] **Step 2: Remove the `phases` block from `TOURNAMENTS_BY_USER_QUERY` in `operations.rs`**

In `backend/crates/common/src/startgg/operations.rs`, replace the `TOURNAMENTS_BY_USER_QUERY` constant:

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
                    }
                }
            }
        }
    }"#;
```

- [ ] **Step 3: Update `import_event` in `import.rs` to call `event_phases`**

In `backend/crates/worker/src/import.rs`, replace the `upsert_phases` call inside `import_event`. Find this section:

```rust
// Upsert phases and phase groups, building startgg_phase_group_id → UUID map
let phase_group_map = upsert_phases(
    pool,
    event_db_id,
    event.phases.as_deref().unwrap_or(&[]),
)
.await?;
```

Replace it with:

```rust
// Fetch phases and phase groups for this event
let phases = startgg.event_phases(event.id).await?;
let phase_group_map = upsert_phases(pool, event_db_id, &phases).await?;
```

- [ ] **Step 4: Verify the project compiles and tests pass**

```bash
cd backend && cargo test -p common
```

Expected: all common tests pass (the existing `tournaments_by_user_returns_page` test mock has no `phases` field, so it will deserialize cleanly).

Run the full backend suite to check the worker compiles:

```bash
bash backend/test.sh
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/common/src/startgg/queries.rs backend/crates/common/src/startgg/operations.rs backend/crates/worker/src/import.rs
git commit -m "feat(worker): fetch event phases separately, remove from tournaments_by_user query"
```

---

### Task 4: Add complexity retry to paginated import loops

**Files:**
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Update `collect_user_tournaments` to use the complexity retry pattern**

In `backend/crates/worker/src/import.rs`, replace the entire body of `collect_user_tournaments` (the `loop` block and the surrounding `page`/`scanned`/`newly_added` declarations):

```rust
async fn collect_user_tournaments(
    startgg: &StartggClient,
    user_id: i64,
    game_id: i64,
    after_date: Option<i64>,
    before_date: Option<i64>,
    seen: &mut HashMap<i64, TournamentNode>,
) -> anyhow::Result<()> {
    let mut per_page = 25i32;
    let mut scanned = 0usize;
    let mut newly_added = 0usize;

    'pages: loop {
        let mut page = 1i32;
        loop {
            let tournament_page = match startgg
                .tournaments_by_user(user_id, game_id, page, per_page)
                .await
            {
                Err(StartggError::ComplexityTooHigh { actual, limit }) if per_page > 1 => {
                    tracing::warn!(
                        per_page, actual, limit,
                        "complexity too high, halving perPage"
                    );
                    per_page /= 2;
                    continue 'pages;
                }
                other => other?,
            };

            for tournament in tournament_page.nodes {
                let start_ts = tournament.start_at.unwrap_or(0);
                if let Some(before) = before_date {
                    if start_ts > before {
                        continue;
                    }
                }
                if let Some(after) = after_date {
                    if start_ts < after {
                        continue;
                    }
                }
                scanned += 1;
                seen.entry(tournament.id).or_insert_with(|| {
                    newly_added += 1;
                    tournament
                });
            }

            let total_pages = tournament_page
                .page_info
                .as_ref()
                .and_then(|p| p.total_pages)
                .unwrap_or(1);
            if page >= total_pages {
                break 'pages;
            }
            page += 1;
        }
    }

    tracing::info!(scanned, newly_added, "user tournaments scanned");
    Ok(())
}
```

- [ ] **Step 2: Update `import_entrants` to use the complexity retry pattern**

In `backend/crates/worker/src/import.rs`, replace the entire `import_entrants` function:

```rust
#[instrument(skip(pool, startgg, account_map), fields(event_startgg_id))]
async fn import_entrants(
    pool: &PgPool,
    startgg: &StartggClient,
    event_db_id: Uuid,
    event_startgg_id: i64,
    account_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<HashMap<i64, Uuid>> {
    let mut entrant_map: HashMap<i64, Uuid> = HashMap::new();
    let mut per_page = 25i32;

    'pages: loop {
        let mut page = 1i32;
        loop {
            let entrant_page =
                match startgg.event_entrants(event_startgg_id, page, per_page).await {
                    Err(StartggError::ComplexityTooHigh { actual, limit }) if per_page > 1 => {
                        tracing::warn!(
                            per_page, actual, limit,
                            "complexity too high, halving perPage"
                        );
                        per_page /= 2;
                        continue 'pages;
                    }
                    other => other?,
                };

            for entrant in &entrant_page.nodes {
                let player_id: Option<Uuid> = entrant
                    .startgg_user_id()
                    .and_then(|uid| account_map.get(&uid))
                    .copied();

                let row = sqlx::query!(
                    r#"INSERT INTO entrants
                           (event_id, player_id, startgg_entrant_id, startgg_user_id,
                            seed, display_name, is_disqualified, final_placement)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                       ON CONFLICT (event_id, startgg_entrant_id) DO UPDATE SET
                           player_id       = COALESCE(EXCLUDED.player_id, entrants.player_id),
                           seed            = EXCLUDED.seed,
                           display_name    = EXCLUDED.display_name,
                           is_disqualified = EXCLUDED.is_disqualified,
                           final_placement = EXCLUDED.final_placement
                       RETURNING id"#,
                    event_db_id,
                    player_id,
                    entrant.id,
                    entrant.startgg_user_id(),
                    entrant.initial_seed_num,
                    entrant.display_name(),
                    entrant.is_disqualified.unwrap_or(false),
                    entrant.standing.as_ref().and_then(|s| s.placement),
                )
                .fetch_one(pool)
                .await?;

                entrant_map.insert(entrant.id, row.id);
            }

            tracing::debug!(
                page,
                entrant_count = entrant_page.nodes.len(),
                "entrants page imported"
            );

            let total_pages = entrant_page
                .page_info
                .as_ref()
                .and_then(|p| p.total_pages)
                .unwrap_or(1);
            if page >= total_pages {
                break 'pages;
            }
            page += 1;
        }
    }

    Ok(entrant_map)
}
```

- [ ] **Step 3: Update `import_sets` to use the complexity retry pattern**

In `backend/crates/worker/src/import.rs`, replace the entire `import_sets` function:

```rust
#[instrument(skip(pool, startgg, entrant_map, phase_group_map), fields(event_startgg_id))]
async fn import_sets(
    pool: &PgPool,
    startgg: &StartggClient,
    event_db_id: Uuid,
    event_startgg_id: i64,
    entrant_map: &HashMap<i64, Uuid>,
    phase_group_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<usize> {
    let mut per_page = 25i32;
    let mut total_sets = 0usize;

    'pages: loop {
        let mut page = 1i32;
        loop {
            let set_page = match startgg.event_sets(event_startgg_id, page, per_page).await {
                Ok(p) => p,
                Err(StartggError::ComplexityTooHigh { actual, limit }) if per_page > 1 => {
                    tracing::warn!(
                        per_page, actual, limit,
                        "complexity too high, halving perPage"
                    );
                    per_page /= 2;
                    continue 'pages;
                }
                Err(StartggError::Decode(msg)) => {
                    tracing::error!(event_startgg_id, page, "set page decode error: {msg}");
                    break 'pages;
                }
                Err(e) => return Err(e.into()),
            };

            let mut page_sets = 0usize;

            for set in &set_page.nodes {
                if set.has_placeholder.unwrap_or(false) {
                    continue;
                }

                let (Some(winner_sg_id), Some(loser_sg_id)) = (set.winner_id, set.loser_id())
                else {
                    continue;
                };
                let (Some(&winner_uuid), Some(&loser_uuid)) = (
                    entrant_map.get(&winner_sg_id),
                    entrant_map.get(&loser_sg_id),
                ) else {
                    tracing::warn!(set_id = set.id, "entrant not found for set, skipping");
                    continue;
                };

                let phase_group_id: Option<Uuid> = set.phase_group.as_ref().and_then(|pg| {
                    let uuid = phase_group_map.get(&pg.id).copied();
                    if uuid.is_none() {
                        tracing::warn!(
                            set_id = set.id,
                            pg_id = pg.id,
                            "phase_group not in map, storing NULL"
                        );
                    }
                    uuid
                });

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

            total_sets += page_sets;
            tracing::debug!(page, set_count = page_sets, "sets page imported");

            let total_pages = set_page
                .page_info
                .as_ref()
                .and_then(|p| p.total_pages)
                .unwrap_or(1);
            if page >= total_pages {
                break 'pages;
            }
            page += 1;
        }
    }

    Ok(total_sets)
}
```

- [ ] **Step 4: Run the full backend test suite**

```bash
bash backend/test.sh
```

Expected: all tests pass. The e2e tests exercise the full worker pipeline and verify the import still works end-to-end.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/worker/src/import.rs
git commit -m "feat(worker): add complexity retry with per_page halving to all paginated loops"
```
