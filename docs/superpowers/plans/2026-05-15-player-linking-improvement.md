# Player Linking Improvement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce player setup friction by enabling batch-add from a tournament, batch-add by handle, player rename, and normalizing slug storage to bare handles throughout.

**Architecture:** DB schema edits in-place (no prod data), followed by a mechanical slug→handle rename across worker and API, then new endpoints for tournament-entrant listing, bulk player creation, and rename. Two start.gg operations are added; all new API endpoints follow the existing Axum/sqlx pattern in `players.rs`.

**Tech Stack:** Rust, Axum 0.8, sqlx (compile-time query macros), `#[sqlx::test]` integration tests, wiremock for start.gg mocks.

---

## File Map

| File | Change |
|---|---|
| `backend/migrations/001_initial.sql` | Rename `slug`→`handle` in `startgg_accounts`, `tournaments`, `events`; make all NOT NULL |
| `backend/crates/common/src/models/mod.rs` | `StartggAccount.slug` → `handle: String` |
| `backend/crates/worker/src/import.rs` | Extract bare handles from `TournamentNode.slug` / `EventNode.slug` before DB write |
| `backend/crates/api/src/routes/players.rs` | Rename `slug`→`handle` in responses; add `normalize_handle`, `normalize_tournament_handle`; update `link_account`; add `tournament_entrants`, `bulk_add`, `by_handles`, `rename_player` handlers |
| `backend/crates/api/src/routes/tournaments.rs` | Update all SQL `t.slug`/`e.slug` → `t.handle`/`e.handle` and response field names |
| `backend/crates/api/src/routes/projects.rs` | Wire new routes |
| `backend/crates/common/src/startgg/queries.rs` | Add `TournamentEventsVars/Data`, `TournamentEntrantListVars/Data`, and public `TournamentEntrant` struct |
| `backend/crates/common/src/startgg/operations.rs` | Add `tournament_entrants` method |
| `backend/crates/api/tests/api.rs` | Update `seed_tournament_event` helper; add tests for new endpoints |
| `backend/.sqlx/` | Regenerated twice via `prepare-sqlx.sh` |

---

## Task 1: Update DB schema in-place

**Files:**
- Modify: `backend/migrations/001_initial.sql`

- [ ] **Step 1: Rename `slug` → `handle` in `startgg_accounts`**

In `001_initial.sql`, find the `startgg_accounts` table and change:
```sql
-- Before
slug            TEXT        NOT NULL,  -- e.g. "user/abc123"
-- After
handle          TEXT        NOT NULL,  -- e.g. "mang0"
```

- [ ] **Step 2: Rename `slug` → `handle` in `tournaments`**

```sql
-- Before
slug           TEXT        NOT NULL,
-- After
handle         TEXT        NOT NULL,  -- bare handle, e.g. "some-weekly"
```

- [ ] **Step 3: Rename `slug` → `handle` in `events`, make NOT NULL**

```sql
-- Before
slug          TEXT,
-- After
handle        TEXT        NOT NULL,  -- event segment only, e.g. "melee-singles"
```

- [ ] **Step 4: Commit**

```bash
git add backend/migrations/001_initial.sql
git commit -m "feat: rename slug→handle in DB schema, store bare handles only"
```

---

## Task 2: Update the `StartggAccount` model

**Files:**
- Modify: `backend/crates/common/src/models/mod.rs`

- [ ] **Step 1: Replace `slug` field with `handle`**

```rust
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
git commit -m "feat: rename StartggAccount.slug → handle"
```

---

## Task 3: Update worker import — extract bare handles before DB write

**Files:**
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Add handle-extraction helpers at top of file (after imports)**

```rust
fn extract_tournament_handle(slug: &str) -> &str {
    slug.trim_start_matches("tournament/")
}

fn extract_event_handle(slug: Option<&str>, event_id: i64) -> String {
    slug.and_then(|s| s.split('/').last())
        .map(|h| h.to_string())
        .unwrap_or_else(|| event_id.to_string())
}
```

- [ ] **Step 2: Use `extract_tournament_handle` in `import_tournament`**

In the `INSERT INTO tournaments` query, change the `slug` parameter to `handle` and apply the extractor. The `INSERT` currently passes `tournament.slug` for the `slug` column. Update to:

```sql
INSERT INTO tournaments
    (startgg_id, name, handle, city, addr_state, country_code,
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
RETURNING id
```

The third parameter (was `tournament.slug`) becomes:
```rust
extract_tournament_handle(&tournament.slug),
```

- [ ] **Step 3: Use `extract_event_handle` in `import_event`**

In the `INSERT INTO events` query, change `slug` → `handle` and apply the extractor:

```sql
INSERT INTO events
    (tournament_id, startgg_id, name, handle, state, is_online, event_type,
     min_team_size, max_team_size, game_id, game_name, num_entrants, start_at)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
ON CONFLICT (startgg_id) DO UPDATE SET
    name          = EXCLUDED.name,
    handle        = EXCLUDED.handle,
    state         = EXCLUDED.state,
    is_online     = EXCLUDED.is_online,
    event_type    = EXCLUDED.event_type,
    min_team_size = EXCLUDED.min_team_size,
    max_team_size = EXCLUDED.max_team_size,
    num_entrants  = EXCLUDED.num_entrants,
    start_at      = EXCLUDED.start_at
RETURNING id
```

The fourth parameter (was `event.slug`) becomes:
```rust
extract_event_handle(event.slug.as_deref(), event.id),
```

- [ ] **Step 4: Commit**

```bash
git add backend/crates/worker/src/import.rs
git commit -m "feat: extract bare handles when writing tournaments/events to DB"
```

---

## Task 4: Update API routes — slug → handle (mechanical rename)

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`
- Modify: `backend/crates/api/src/routes/tournaments.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Update `players.rs` — rename `slug` → `handle` in request/response types and queries**

`LinkAccountRequest`:
```rust
#[derive(Deserialize)]
struct LinkAccountRequest {
    handle: String,
}
```

`AccountResponse`:
```rust
#[derive(Serialize)]
pub struct AccountResponse {
    pub id: Uuid,
    pub startgg_user_id: i64,
    pub handle: String,
    pub display_name: Option<String>,
}

impl From<StartggAccount> for AccountResponse {
    fn from(a: StartggAccount) -> Self {
        AccountResponse {
            id: a.id,
            startgg_user_id: a.startgg_user_id,
            handle: a.handle,
            display_name: a.display_name,
        }
    }
}
```

In `link_account` handler, update the INSERT query column name and the `user_by_slug` call (temporarily keep `format!("user/{}", body.handle)` — normalization comes in Task 5):
```rust
// call start.gg with full slug format
let sg_user = state
    .startgg
    .user_by_slug(&format!("user/{}", body.handle.trim()))
    .await?
    .ok_or_else(|| AppError::UnprocessableEntity("user not found on start.gg".into()))?;

let account = sqlx::query_as!(
    StartggAccount,
    "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
     VALUES ($1, $2, $3, $4)
     RETURNING id, player_id, startgg_user_id, handle, display_name, created_at",
    path.pid,
    sg_user.id,
    body.handle.trim(),
    sg_user.gamer_tag(),
)
```

Also update the `list_players` SELECT query:
```rust
sqlx::query_as!(
    StartggAccount,
    "SELECT id, player_id, startgg_user_id, handle, display_name, created_at
     FROM startgg_accounts
     WHERE player_id = ANY($1)",
    &player_ids as &[Uuid],
)
```

- [ ] **Step 2: Update `tournaments.rs` — rename all `slug` → `handle` in SQL and response types**

For every `t.slug` in SQL queries replace with `t.handle`, and `e.slug` with `e.handle`. For every struct field named `slug` or `tournament_slug` or `event_slug`, rename to `handle` / `tournament_handle` / `event_handle`.

The three queries in `tournaments.rs` that select `t.slug` and `e.slug` become:
```sql
-- everywhere t.slug appears:
t.handle          AS tournament_handle,
-- everywhere e.slug appears:
e.handle          AS "event_handle?: String"
-- (keep the nullable annotation if applicable)
```

Update all response struct fields accordingly:
```rust
pub tournament_handle: String,
pub event_handle: Option<String>,
```

And all the places these fields are referenced in the response-building code.

- [ ] **Step 3: Update `api/tests/api.rs` — `seed_tournament_event` helper**

```rust
async fn seed_tournament_event(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    startgg_tournament_id: i64,
    startgg_event_id: i64,
) -> (Uuid, Uuid) {
    let tournament_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO tournaments (startgg_id, name, handle, online)
         VALUES ($1, 'Test Tournament', 'test-tournament', false)
         RETURNING id",
        startgg_tournament_id,
    )
    ...
```

Also update the events INSERT to use `handle`:
```rust
sqlx::query_scalar!(
    "INSERT INTO events (tournament_id, startgg_id, name, handle, game_id)
     VALUES ($1, $2, 'Singles', 'singles', $3)
     RETURNING id",
    ...
```

- [ ] **Step 4: Commit**

```bash
git add backend/crates/api/src/routes/players.rs \
        backend/crates/api/src/routes/tournaments.rs \
        backend/crates/api/tests/api.rs
git commit -m "feat: rename slug→handle throughout API routes and test helpers"
```

---

## Task 5: First sqlx offline cache refresh

**Files:**
- Modify: `backend/.sqlx/` (auto-generated)

- [ ] **Step 1: Regenerate the offline query cache**

```bash
bash backend/prepare-sqlx.sh
```

Expected: script spins up a Postgres container, runs migrations, runs `cargo sqlx prepare --workspace`, exits 0. Watch for any `error[E0...]` lines — if the previous tasks missed a `slug` reference, the compile step inside `prepare-sqlx.sh` will catch it.

- [ ] **Step 2: Verify build**

```bash
cd backend && SQLX_OFFLINE=true cargo build 2>&1 | grep -E "^error"
```

Expected: no output (zero errors).

- [ ] **Step 3: Commit the refreshed cache**

```bash
git add backend/.sqlx/
git commit -m "chore: regenerate sqlx offline cache after slug→handle rename"
```

---

## Task 6: Handle normalization utilities (TDD)

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`

- [ ] **Step 1: Write failing unit tests for `normalize_handle`**

Add at the bottom of `players.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::normalize_handle;

    #[test]
    fn normalize_bare_handle() {
        assert_eq!(normalize_handle("mang0"), "mang0");
    }

    #[test]
    fn normalize_full_slug() {
        assert_eq!(normalize_handle("user/mang0"), "mang0");
    }

    #[test]
    fn normalize_full_url() {
        assert_eq!(normalize_handle("https://www.start.gg/user/mang0"), "mang0");
    }

    #[test]
    fn normalize_trims_whitespace() {
        assert_eq!(normalize_handle("  mang0  "), "mang0");
    }
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cd backend && cargo test -p api normalize_handle 2>&1 | grep -E "FAILED|error"
```

Expected: compile error — `normalize_handle` not yet defined.

- [ ] **Step 3: Implement `normalize_handle` in `players.rs`** (add before the handlers section)

```rust
fn normalize_handle(input: &str) -> String {
    let s = input.trim();
    let s = s
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.start.gg/")
        .trim_start_matches("start.gg/")
        .trim_start_matches("user/");
    s.to_string()
}
```

- [ ] **Step 4: Run tests to confirm pass**

```bash
cd backend && cargo test -p api normalize_handle
```

Expected: `test routes::players::tests::normalize_bare_handle ... ok` (and the other three).

- [ ] **Step 5: Write failing unit tests for `normalize_tournament_handle`**

Add to the `tests` module:

```rust
use super::normalize_tournament_handle;

#[test]
fn normalize_tournament_bare() {
    assert_eq!(normalize_tournament_handle("some-weekly"), "some-weekly");
}

#[test]
fn normalize_tournament_full_slug() {
    assert_eq!(normalize_tournament_handle("tournament/some-weekly"), "some-weekly");
}

#[test]
fn normalize_tournament_full_url() {
    assert_eq!(
        normalize_tournament_handle("https://www.start.gg/tournament/some-weekly/event/singles"),
        "some-weekly"
    );
}
```

- [ ] **Step 6: Implement `normalize_tournament_handle`**

```rust
fn normalize_tournament_handle(input: &str) -> String {
    let s = input.trim();
    let s = s
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.start.gg/")
        .trim_start_matches("start.gg/")
        .trim_start_matches("tournament/");
    s.split('/').next().unwrap_or(s).to_string()
}
```

- [ ] **Step 7: Run tests to confirm pass**

```bash
cd backend && cargo test -p api normalize_tournament_handle
```

Expected: all pass.

- [ ] **Step 8: Wire `normalize_handle` into `link_account`**

In the `link_account` handler, replace the existing `body.handle.trim()` references with `normalize_handle(&body.handle)`:

```rust
let normalized = normalize_handle(&body.handle);

let sg_user = state
    .startgg
    .user_by_slug(&format!("user/{normalized}"))
    .await?
    .ok_or_else(|| AppError::UnprocessableEntity("user not found on start.gg".into()))?;

let account = sqlx::query_as!(
    StartggAccount,
    "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
     VALUES ($1, $2, $3, $4)
     RETURNING id, player_id, startgg_user_id, handle, display_name, created_at",
    path.pid,
    sg_user.id,
    normalized,
    sg_user.gamer_tag(),
)
```

- [ ] **Step 9: Commit**

```bash
git add backend/crates/api/src/routes/players.rs
git commit -m "feat: add handle normalization and apply to link_account endpoint"
```

---

## Task 7: Add `tournament_entrants` start.gg operation (TDD)

**Files:**
- Modify: `backend/crates/common/src/startgg/queries.rs`
- Modify: `backend/crates/common/src/startgg/operations.rs`
- Modify: `backend/crates/common/src/startgg/mod.rs` (tests)

- [ ] **Step 1: Write failing wiremock test**

In `backend/crates/common/src/startgg/mod.rs`, add to the existing `#[cfg(test)]` block:

```rust
#[tokio::test]
async fn tournament_entrants_returns_list() {
    let mock = MockServer::start().await;

    // First call: tournament events
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "tournament": {
                    "events": [{ "id": 200 }]
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Second call: event entrants (one page)
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": [{
                            "participants": [{
                                "gamerTag": "Mang0",
                                "user": { "id": 12345, "slug": "user/mang0" }
                            }]
                        }]
                    }
                }
            }
        })))
        .mount(&mock)
        .await;

    let result = client(&mock.uri())
        .tournament_entrants("some-weekly", 1)
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].startgg_user_id, 12345);
    assert_eq!(result[0].handle, "mang0");
    assert_eq!(result[0].name, "Mang0");
}

#[tokio::test]
async fn tournament_entrants_deduplicates_across_events() {
    let mock = MockServer::start().await;

    // First call: tournament with two events
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "tournament": {
                    "events": [{ "id": 200 }, { "id": 201 }]
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Calls for event 200 and event 201 — same user appears in both
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": [{
                            "participants": [{
                                "gamerTag": "Mang0",
                                "user": { "id": 12345, "slug": "user/mang0" }
                            }]
                        }]
                    }
                }
            }
        })))
        .mount(&mock)
        .await;

    let result = client(&mock.uri())
        .tournament_entrants("some-weekly", 1)
        .await
        .unwrap();

    assert_eq!(result.len(), 1, "same user from two events should appear once");
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cd backend && cargo test -p common tournament_entrants 2>&1 | grep -E "FAILED|error\[E"
```

Expected: compile error — `tournament_entrants` method not defined.

- [ ] **Step 3: Add query structs to `queries.rs`**

Add after the existing entrants section:

```rust
// ── Tournament events ─────────────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct TournamentEventsVars {
    pub slug: String,
    #[serde(rename = "gameId")]
    pub game_id: i64,
}

#[derive(Deserialize)]
pub(crate) struct TournamentEventsData {
    pub tournament: Option<TournamentWithEventIds>,
}

#[derive(Deserialize)]
pub(crate) struct TournamentWithEventIds {
    pub events: Option<Vec<TournamentEventId>>,
}

#[derive(Deserialize)]
pub(crate) struct TournamentEventId {
    pub id: i64,
}

// ── Tournament entrant list ───────────────────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct TournamentEntrantListVars {
    #[serde(rename = "eventId")]
    pub event_id: i64,
    pub page: i32,
    #[serde(rename = "perPage")]
    pub per_page: i32,
}

#[derive(Deserialize)]
pub(crate) struct TournamentEntrantListData {
    pub event: Option<EventWithEntrantList>,
}

#[derive(Deserialize)]
pub(crate) struct EventWithEntrantList {
    pub entrants: TournamentEntrantPage,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentEntrantPage {
    pub page_info: Option<PageInfo>,
    pub nodes: Vec<TournamentEntrantNode>,
}

#[derive(Deserialize)]
pub(crate) struct TournamentEntrantNode {
    pub participants: Vec<TournamentEntrantParticipant>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TournamentEntrantParticipant {
    pub gamer_tag: String,
    pub user: Option<TournamentEntrantUser>,
}

#[derive(Deserialize)]
pub(crate) struct TournamentEntrantUser {
    pub id: i64,
    pub slug: Option<String>,
}

/// Public return type from `StartggClient::tournament_entrants`.
#[derive(Debug, Clone)]
pub struct TournamentEntrant {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
}
```

- [ ] **Step 4: Add operation to `operations.rs`**

Add the two query constants and the method:

```rust
const TOURNAMENT_EVENTS_QUERY: &str = r#"
    query($slug: String!, $gameId: ID!) {
        tournament(slug: $slug) {
            events(filter: { videogameId: [$gameId] }) { id }
        }
    }"#;

const TOURNAMENT_ENTRANT_LIST_QUERY: &str = r#"
    query($eventId: ID!, $page: Int!, $perPage: Int!) {
        event(id: $eventId) {
            entrants(query: { page: $page, perPage: $perPage }) {
                pageInfo { totalPages }
                nodes {
                    participants {
                        gamerTag
                        user { id slug }
                    }
                }
            }
        }
    }"#;
```

Add the imports at the top of `operations.rs` (add to the existing `use super::queries::` line):
```rust
use super::queries::{
    ...,
    TournamentEntrant, TournamentEntrantListData, TournamentEntrantListVars,
    TournamentEntrantPage, TournamentEventsData, TournamentEventsVars,
};
```

Add the method to the `impl StartggClient` block:

```rust
#[instrument(skip(self))]
pub async fn tournament_entrants(
    &self,
    tournament_handle: &str,
    game_id: i64,
) -> Result<Vec<TournamentEntrant>, StartggError> {
    let t = Instant::now();

    let events_data: TournamentEventsData = self
        .gql(
            TOURNAMENT_EVENTS_QUERY,
            TournamentEventsVars {
                slug: tournament_handle.to_string(),
                game_id,
            },
        )
        .await?;

    let event_ids: Vec<i64> = events_data
        .tournament
        .and_then(|t| t.events)
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.id)
        .collect();

    if event_ids.is_empty() {
        return Ok(vec![]);
    }

    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut result: Vec<TournamentEntrant> = Vec::new();

    for event_id in event_ids {
        let mut per_page = 25i32;

        'pages: loop {
            let mut page = 1i32;
            loop {
                let data: TournamentEntrantListData = match self
                    .gql(
                        TOURNAMENT_ENTRANT_LIST_QUERY,
                        TournamentEntrantListVars { event_id, page, per_page },
                    )
                    .await
                {
                    Err(StartggError::ComplexityTooHigh { actual, limit })
                        if per_page > 1 =>
                    {
                        tracing::warn!(per_page, actual, limit, "complexity too high, halving perPage");
                        per_page /= 2;
                        continue 'pages;
                    }
                    other => other?,
                };

                let entrant_page = data
                    .event
                    .map(|e| e.entrants)
                    .unwrap_or_else(|| TournamentEntrantPage { page_info: None, nodes: vec![] });

                let total_pages = entrant_page
                    .page_info
                    .as_ref()
                    .and_then(|pi| pi.total_pages)
                    .unwrap_or(1);

                for node in entrant_page.nodes {
                    for participant in node.participants {
                        if let Some(user) = participant.user {
                            if seen.insert(user.id) {
                                let handle = user
                                    .slug
                                    .as_deref()
                                    .and_then(|s| s.strip_prefix("user/"))
                                    .map(|h| h.to_string())
                                    .unwrap_or_else(|| user.id.to_string());
                                result.push(TournamentEntrant {
                                    startgg_user_id: user.id,
                                    handle,
                                    name: participant.gamer_tag,
                                });
                            }
                        }
                    }
                }

                if page >= total_pages {
                    break 'pages;
                }
                page += 1;
            }
        }
    }

    tracing::debug!(elapsed_ms = t.elapsed().as_millis(), "startgg query complete");
    Ok(result)
}
```

- [ ] **Step 5: Run tests to confirm pass**

```bash
cd backend && cargo test -p common tournament_entrants
```

Expected: both tests pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/common/src/startgg/
git commit -m "feat: add tournament_entrants start.gg operation"
```

---

## Task 8: Add `GET /projects/:id/tournament-entrants` endpoint (TDD)

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`
- Modify: `backend/crates/api/src/routes/projects.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Write failing integration test**

Add to `api/tests/api.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_tournament_entrants_returns_list(pool: PgPool) {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": { "tournament": { "events": [{ "id": 200 }] } }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": { "event": { "entrants": {
                "pageInfo": { "totalPages": 1 },
                "nodes": [{ "participants": [{ "gamerTag": "Mang0", "user": { "id": 12345, "slug": "user/mang0" } }] }]
            }}}
        })))
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "u1", "pass").await;

    let resp = post_json(&app, "/projects", &cookie,
        json!({"name": "PR", "game_id": 1, "game_name": "Melee"})).await;
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = get_req(&app,
        &format!("/projects/{project_id}/tournament-entrants?tournament=some-weekly"),
        &cookie).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    let list = body.as_array().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["handle"], "mang0");
    assert_eq!(list[0]["name"], "Mang0");
    assert_eq!(list[0]["startgg_user_id"], 12345);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_tournament_entrants_requires_auth(pool: PgPool) {
    let app = make_app(pool, "");
    let resp = get_req(&app, "/projects/00000000-0000-0000-0000-000000000000/tournament-entrants?tournament=x", "").await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_tournament_entrants_requires_game_id(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "u1", "pass").await;
    let resp = post_json(&app, "/projects", &cookie,
        json!({"name": "PR"})).await;
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = get_req(&app,
        &format!("/projects/{project_id}/tournament-entrants?tournament=x"),
        &cookie).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
```

Also add `use wiremock::{MockServer, Mock, ResponseTemplate, matchers::method};` at the top of the test file if not already present. Check existing imports — `MockServer` is likely already there from other tests.

- [ ] **Step 2: Run to confirm failure**

```bash
cd backend && cargo test -p api test_tournament_entrants 2>&1 | grep -E "FAILED|error\[E"
```

Expected: compile error — route and handler not defined.

- [ ] **Step 3: Add response type and handler to `players.rs`**

Add response type:
```rust
#[derive(Serialize)]
pub struct TournamentEntrantResponse {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
}
```

Add query params struct:
```rust
#[derive(Deserialize)]
struct TournamentEntrantsQuery {
    tournament: String,
}
```

Add handler:
```rust
pub async fn list_tournament_entrants(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    axum::extract::Query(params): axum::extract::Query<TournamentEntrantsQuery>,
) -> Result<impl IntoResponse> {
    let project = require_project(&state.db, project_id, user.id).await?;

    let game_id = project.game_id.ok_or_else(|| {
        AppError::UnprocessableEntity("project has no game_id set".into())
    })?;

    let handle = normalize_tournament_handle(&params.tournament);

    let entrants = state
        .startgg
        .tournament_entrants(&handle, game_id)
        .await?;

    let resp: Vec<TournamentEntrantResponse> = entrants
        .into_iter()
        .map(|e| TournamentEntrantResponse {
            startgg_user_id: e.startgg_user_id,
            handle: e.handle,
            name: e.name,
        })
        .collect();

    Ok(Json(resp))
}
```

Note: `require_project` returns `Result<Project>` (see `projects.rs:52`), so `project.game_id` is available directly.

- [ ] **Step 4: Wire route in `projects.rs`**

Add to the `router()` function:
```rust
.route(
    "/{id}/tournament-entrants",
    get(crate::routes::players::list_tournament_entrants),
)
```

- [ ] **Step 5: Run tests to confirm pass**

```bash
bash backend/test.sh 2>&1 | grep -E "test_tournament_entrants.*ok|FAILED"
```

Expected: all three `test_tournament_entrants_*` tests pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/players.rs \
        backend/crates/api/src/routes/projects.rs \
        backend/crates/api/tests/api.rs
git commit -m "feat: add GET /projects/:id/tournament-entrants endpoint"
```

---

## Task 9: Add `POST /projects/:id/players/bulk` endpoint (TDD)

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Write failing integration tests**

Add to `api/tests/api.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_bulk_add_creates_players(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "u1", "pass").await;

    let resp = post_json(&app, "/projects", &cookie,
        json!({"name": "PR", "game_id": 1, "game_name": "Melee"})).await;
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/bulk"),
        &cookie,
        json!({ "players": [
            { "name": "Mang0", "startgg_user_id": 12345, "handle": "mang0" },
            { "name": "Armada", "startgg_user_id": 67890, "handle": "armada" }
        ]}),
    ).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    let results = body.as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["status"], "created");
    assert_eq!(results[0]["handle"], "mang0");
    assert_eq!(results[1]["status"], "created");

    // Verify players are retrievable
    let resp = get_req(&app, &format!("/projects/{project_id}/players"), &cookie).await;
    let players = read_json(resp).await;
    assert_eq!(players.as_array().unwrap().len(), 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_bulk_add_skips_duplicate_user_id(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "u1", "pass").await;

    let resp = post_json(&app, "/projects", &cookie,
        json!({"name": "PR", "game_id": 1, "game_name": "Melee"})).await;
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    // Add once
    post_json(&app, &format!("/projects/{project_id}/players/bulk"), &cookie,
        json!({ "players": [{ "name": "Mang0", "startgg_user_id": 12345, "handle": "mang0" }] }),
    ).await;

    // Add same user again
    let resp = post_json(&app, &format!("/projects/{project_id}/players/bulk"), &cookie,
        json!({ "players": [{ "name": "Mang0", "startgg_user_id": 12345, "handle": "mang0" }] }),
    ).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body[0]["status"], "skipped");

    // Still only one player
    let resp = get_req(&app, &format!("/projects/{project_id}/players"), &cookie).await;
    assert_eq!(read_json(resp).await.as_array().unwrap().len(), 1);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_bulk_add_requires_auth(pool: PgPool) {
    let app = make_app(pool, "");
    let resp = post_json(&app,
        "/projects/00000000-0000-0000-0000-000000000000/players/bulk", "",
        json!({ "players": [] })).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cd backend && cargo test -p api test_bulk_add 2>&1 | grep -E "FAILED|error\[E"
```

Expected: compile error.

- [ ] **Step 3: Add types and handler to `players.rs`**

```rust
#[derive(Deserialize)]
pub struct BulkAddEntry {
    pub name: String,
    pub startgg_user_id: i64,
    pub handle: String,
}

#[derive(Deserialize)]
pub struct BulkAddRequest {
    pub players: Vec<BulkAddEntry>,
}

#[derive(Serialize)]
pub struct BulkAddResult {
    pub handle: String,
    pub name: String,
    pub status: &'static str,
}
```

Handler:

```rust
async fn bulk_add_players(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<BulkAddRequest>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

    let mut results = Vec::with_capacity(body.players.len());

    for entry in body.players {
        // Capture owned values before any moves
        let handle = normalize_handle(&entry.handle);
        let name = entry.name;
        let user_id = entry.startgg_user_id;

        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                SELECT 1 FROM startgg_accounts sa
                JOIN players p ON p.id = sa.player_id
                WHERE p.project_id = $1 AND sa.startgg_user_id = $2
            ) AS "exists!""#,
            project_id,
            user_id,
        )
        .fetch_one(&state.db)
        .await?;

        if exists {
            results.push(BulkAddResult { handle, name, status: "skipped" });
            continue;
        }

        let player = sqlx::query_as!(
            Player,
            "INSERT INTO players (project_id, name) VALUES ($1, $2)
             RETURNING id, project_id, name, created_at",
            project_id,
            &name,
        )
        .fetch_one(&state.db)
        .await?;

        sqlx::query!(
            "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
             VALUES ($1, $2, $3, $4)",
            player.id,
            user_id,
            &handle,
            &name,
        )
        .execute(&state.db)
        .await?;

        results.push(BulkAddResult { handle, name, status: "created" });
    }

    Ok(Json(results))
}
```

- [ ] **Step 4: Wire route in `players.rs` router**

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_players).post(add_player))
        .route("/bulk", post(bulk_add_players))
        .route("/{pid}", delete(delete_player))
        .route("/{pid}/accounts", post(link_account))
        .route("/{pid}/accounts/{aid}", delete(unlink_account))
}
```

- [ ] **Step 5: Run tests to confirm pass**

```bash
bash backend/test.sh 2>&1 | grep -E "test_bulk_add.*ok|FAILED"
```

Expected: all three `test_bulk_add_*` pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/players.rs backend/crates/api/tests/api.rs
git commit -m "feat: add POST /projects/:id/players/bulk endpoint"
```

---

## Task 10: Add `POST /projects/:id/players/by-handles` endpoint (TDD)

This endpoint accepts raw handles (no pre-resolution), calls `user_by_slug` per handle, and creates players.

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Write failing integration test**

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_by_handles_creates_players(pool: PgPool) {
    let mock = MockServer::start().await;

    // Mock user_by_slug for mang0
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": { "user": { "id": 12345, "player": { "gamerTag": "Mang0" } } }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Mock user_by_slug for unknown handle → null
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({ "data": { "user": null } })))
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "u1", "pass").await;

    let resp = post_json(&app, "/projects", &cookie,
        json!({"name": "PR", "game_id": 1, "game_name": "Melee"})).await;
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/by-handles"),
        &cookie,
        json!({ "handles": ["mang0", "user/unknown"] }),
    ).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    let results = body.as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["status"], "created");
    assert_eq!(results[0]["name"], "Mang0");
    assert_eq!(results[1]["status"], "not_found");

    let resp = get_req(&app, &format!("/projects/{project_id}/players"), &cookie).await;
    assert_eq!(read_json(resp).await.as_array().unwrap().len(), 1);
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cd backend && cargo test -p api test_by_handles 2>&1 | grep -E "FAILED|error\[E"
```

Expected: compile error.

- [ ] **Step 3: Add types and handler to `players.rs`**

```rust
#[derive(Deserialize)]
pub struct ByHandlesRequest {
    pub handles: Vec<String>,
}

#[derive(Serialize)]
pub struct ByHandlesResult {
    pub handle: String,
    pub name: Option<String>,
    pub status: &'static str,
}
```

Handler:

```rust
async fn add_players_by_handles(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ByHandlesRequest>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

    let mut results = Vec::with_capacity(body.handles.len());

    for raw in body.handles {
        let handle = normalize_handle(&raw);

        let sg_user = state
            .startgg
            .user_by_slug(&format!("user/{handle}"))
            .await?;

        let Some(sg_user) = sg_user else {
            results.push(ByHandlesResult { handle, name: None, status: "not_found" });
            continue;
        };

        let gamer_tag = sg_user.gamer_tag().unwrap_or(&handle).to_string();
        let user_id = sg_user.id;

        let exists = sqlx::query_scalar!(
            r#"SELECT EXISTS(
                SELECT 1 FROM startgg_accounts sa
                JOIN players p ON p.id = sa.player_id
                WHERE p.project_id = $1 AND sa.startgg_user_id = $2
            ) AS "exists!""#,
            project_id,
            user_id,
        )
        .fetch_one(&state.db)
        .await?;

        if exists {
            results.push(ByHandlesResult { handle, name: Some(gamer_tag), status: "skipped" });
            continue;
        }

        let player = sqlx::query_as!(
            Player,
            "INSERT INTO players (project_id, name) VALUES ($1, $2)
             RETURNING id, project_id, name, created_at",
            project_id,
            &gamer_tag,
        )
        .fetch_one(&state.db)
        .await?;

        sqlx::query!(
            "INSERT INTO startgg_accounts (player_id, startgg_user_id, handle, display_name)
             VALUES ($1, $2, $3, $4)",
            player.id,
            user_id,
            &handle,
            &gamer_tag,
        )
        .execute(&state.db)
        .await?;

        results.push(ByHandlesResult {
            handle,
            name: Some(gamer_tag),
            status: "created",
        });
    }

    Ok(Json(results))
}
```

- [ ] **Step 4: Wire route in `players.rs` router**

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_players).post(add_player))
        .route("/bulk", post(bulk_add_players))
        .route("/by-handles", post(add_players_by_handles))
        .route("/{pid}", delete(delete_player))
        .route("/{pid}/accounts", post(link_account))
        .route("/{pid}/accounts/{aid}", delete(unlink_account))
}
```

- [ ] **Step 5: Run tests to confirm pass**

```bash
bash backend/test.sh 2>&1 | grep -E "test_by_handles.*ok|FAILED"
```

Expected: `test_by_handles_creates_players ... ok`.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/players.rs backend/crates/api/tests/api.rs
git commit -m "feat: add POST /projects/:id/players/by-handles endpoint"
```

---

## Task 11: Add `PATCH /projects/:id/players/:pid` rename endpoint (TDD)

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Write failing integration tests**

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_rename_player_updates_name(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "u1", "pass").await;

    let resp = post_json(&app, "/projects", &cookie,
        json!({"name": "PR", "game_id": 1, "game_name": "Melee"})).await;
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(&app, &format!("/projects/{project_id}/players"), &cookie,
        json!({"name": "Mang0"})).await;
    let player_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = patch_json(&app, &format!("/projects/{project_id}/players/{player_id}"),
        &cookie, json!({"name": "MANG0"})).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body["name"], "MANG0");

    // Verify persisted
    let resp = get_req(&app, &format!("/projects/{project_id}/players"), &cookie).await;
    let players = read_json(resp).await;
    assert_eq!(players[0]["name"], "MANG0");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_rename_player_empty_name_returns_422(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "u1", "pass").await;

    let resp = post_json(&app, "/projects", &cookie,
        json!({"name": "PR", "game_id": 1, "game_name": "Melee"})).await;
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(&app, &format!("/projects/{project_id}/players"), &cookie,
        json!({"name": "Mang0"})).await;
    let player_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = patch_json(&app, &format!("/projects/{project_id}/players/{player_id}"),
        &cookie, json!({"name": "  "})).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_rename_player_wrong_project_returns_404(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "u1", "pass").await;

    let resp = patch_json(&app,
        "/projects/00000000-0000-0000-0000-000000000000/players/00000000-0000-0000-0000-000000000001",
        &cookie, json!({"name": "X"})).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 2: Run to confirm failure**

```bash
cd backend && cargo test -p api test_rename_player 2>&1 | grep -E "FAILED|error\[E"
```

Expected: compile error.

- [ ] **Step 3: Add type and handler to `players.rs`**

```rust
#[derive(Deserialize)]
struct RenamePlayerRequest {
    name: String,
}
```

Handler:

```rust
async fn rename_player(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<ProjectPlayerPath>,
    Json(body): Json<RenamePlayerRequest>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, path.id, user.id).await?;

    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity("name must not be empty".into()));
    }

    let player = sqlx::query_as!(
        Player,
        "UPDATE players SET name = $1
         WHERE id = $2 AND project_id = $3
         RETURNING id, project_id, name, created_at",
        body.name.trim(),
        path.pid,
        path.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let accounts = sqlx::query_as!(
        StartggAccount,
        "SELECT id, player_id, startgg_user_id, handle, display_name, created_at
         FROM startgg_accounts WHERE player_id = $1",
        player.id,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(PlayerResponse {
        id: player.id,
        project_id: player.project_id,
        name: player.name,
        created_at: player.created_at,
        accounts: accounts.into_iter().map(AccountResponse::from).collect(),
    }))
}
```

- [ ] **Step 4: Wire route in `players.rs` router**

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_players).post(add_player))
        .route("/bulk", post(bulk_add_players))
        .route("/by-handles", post(add_players_by_handles))
        .route("/{pid}", delete(delete_player).patch(rename_player))
        .route("/{pid}/accounts", post(link_account))
        .route("/{pid}/accounts/{aid}", delete(unlink_account))
}
```

- [ ] **Step 5: Run tests to confirm pass**

```bash
bash backend/test.sh 2>&1 | grep -E "test_rename_player.*ok|FAILED"
```

Expected: all three `test_rename_player_*` pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/api/src/routes/players.rs backend/crates/api/tests/api.rs
git commit -m "feat: add PATCH /projects/:id/players/:pid rename endpoint"
```

---

## Task 12: Final sqlx cache refresh and full test suite

**Files:**
- Modify: `backend/.sqlx/` (auto-generated)

- [ ] **Step 1: Regenerate offline cache**

```bash
bash backend/prepare-sqlx.sh
```

Expected: exits 0. This captures the new `sqlx::query!` macros from Tasks 8–11.

- [ ] **Step 2: Run full test suite**

```bash
bash test.sh
```

Expected: all tests pass — backend (common + api + e2e) and frontend (unit + e2e).

- [ ] **Step 3: Commit the refreshed cache**

```bash
git add backend/.sqlx/
git commit -m "chore: regenerate sqlx offline cache for new player endpoints"
```
