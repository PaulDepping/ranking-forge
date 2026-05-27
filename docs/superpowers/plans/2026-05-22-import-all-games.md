# Import All Games Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a ranking project has no game set, import all tournaments and events for each linked player regardless of game, storing per-event game metadata from start.gg.

**Architecture:** Two separate GQL queries — the existing filtered one (unchanged) and a new unfiltered one that requests `videogame { id name }` per event. The import worker branches on `project.game_id` to call the right query path. `game_id` in `import_event` becomes `Option<i64>`, resolved from the event's videogame when the project has no game.

**Tech Stack:** Rust, sqlx, wiremock (tests), start.gg GraphQL API

---

## File Map

| File | Change |
|------|--------|
| `backend/crates/common/src/startgg/queries.rs` | Add `EventVideogame` struct; add `videogame` field to `EventNode`; add `TournamentsByUserAllGamesVars` |
| `backend/crates/common/src/startgg/operations.rs` | Add `TOURNAMENTS_BY_USER_ALL_GAMES_QUERY` const; add `tournaments_by_user_all_games` method; import new types |
| `backend/crates/common/src/startgg/mod.rs` | Re-export `EventVideogame` |
| `backend/crates/worker/src/import.rs` | Remove early-return guard; change `game_id` params to `Option<i64>`; add `collect_user_tournaments_all_games`; branch in `run` |
| `backend/crates/e2e/tests/full_flow.rs` | Add `import_no_game_filter_flow` test |

---

### Task 1: Add `EventVideogame` struct, update `EventNode`, add `TournamentsByUserAllGamesVars`

**Files:**
- Modify: `backend/crates/common/src/startgg/queries.rs`

- [ ] **Step 1: Write the failing test**

In `backend/crates/common/src/startgg/queries/tests.rs`, add:

```rust
#[test]
fn event_node_deserializes_videogame() {
    let json = r#"{
        "id": 2001,
        "name": "Melee Singles",
        "numEntrants": 8,
        "startAt": 1700040000,
        "slug": "tournament/test/event/melee-singles",
        "state": "COMPLETED",
        "isOnline": false,
        "type": 1,
        "teamRosterSize": null,
        "videogame": { "id": 1, "name": "Super Smash Bros. Melee" }
    }"#;
    let node: EventNode = serde_json::from_str(json).unwrap();
    let vg = node.videogame.unwrap();
    assert_eq!(vg.id, 1);
    assert_eq!(vg.name, "Super Smash Bros. Melee");
}

#[test]
fn event_node_videogame_is_none_when_absent() {
    let json = r#"{
        "id": 2001,
        "name": "Melee Singles",
        "numEntrants": 8,
        "startAt": null,
        "slug": null,
        "state": null,
        "isOnline": null,
        "type": null,
        "teamRosterSize": null
    }"#;
    let node: EventNode = serde_json::from_str(json).unwrap();
    assert!(node.videogame.is_none());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd backend && cargo test -p common -- event_node_deserializes_videogame event_node_videogame_is_none_when_absent 2>&1 | tail -20
```

Expected: compile error — `EventNode` has no `videogame` field.

- [ ] **Step 3: Add `EventVideogame` and update `EventNode`**

In `backend/crates/common/src/startgg/queries.rs`, after the `TeamRosterSize` struct, add:

```rust
#[derive(Deserialize, Debug, Clone)]
pub struct EventVideogame {
    pub id: i64,
    pub name: String,
}
```

Then add the field to `EventNode`:

```rust
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
    pub videogame: Option<EventVideogame>,  // new
}
```

Also add `TournamentsByUserAllGamesVars` after `TournamentsByUserVars`:

```rust
#[derive(Serialize)]
pub(crate) struct TournamentsByUserAllGamesVars {
    #[serde(rename = "userId")]
    pub user_id: i64,
    pub page: i32,
    #[serde(rename = "perPage")]
    pub per_page: i32,
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd backend && cargo test -p common -- event_node_deserializes_videogame event_node_videogame_is_none_when_absent 2>&1 | tail -10
```

Expected: both tests pass.

- [ ] **Step 5: Re-export `EventVideogame` from `mod.rs`**

In `backend/crates/common/src/startgg/mod.rs`, add `EventVideogame` to the `pub use queries::{ ... }` list:

```rust
pub use queries::{
    EntrantNode, EntrantPage, EntrantStanding, EventNode, EventVideogame, GameNode, PageInfo,
    Participant, ParticipantUser, PhaseGroupNode, PhaseGroupPage, PhaseNode, ScoreValue, SetNode,
    SetPage, SetPhaseGroup, SetSlot, SlotEntrant, SlotStanding, SlotStats, TeamRosterSize,
    TournamentEntrant, TournamentEntrantOrdered, TournamentEventWithEntrants, TournamentNode,
    TournamentPage, TournamentParticipant, UserNode,
};
```

- [ ] **Step 6: Verify the workspace compiles**

```bash
cd backend && cargo build 2>&1 | tail -20
```

Expected: no errors.

- [ ] **Step 7: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/common/src/startgg/queries.rs \
        backend/crates/common/src/startgg/mod.rs \
        backend/crates/common/src/startgg/queries/tests.rs
git commit -m "feat(common): add EventVideogame to EventNode; add TournamentsByUserAllGamesVars"
```

---

### Task 2: Add all-games GQL query and `StartggClient` method

**Files:**
- Modify: `backend/crates/common/src/startgg/operations.rs`

- [ ] **Step 1: Add `TOURNAMENTS_BY_USER_ALL_GAMES_QUERY` const**

In `backend/crates/common/src/startgg/operations.rs`, after `TOURNAMENTS_BY_USER_QUERY`, add:

```rust
const TOURNAMENTS_BY_USER_ALL_GAMES_QUERY: &str = r#"
    query($userId: ID!, $page: Int!, $perPage: Int!) {
        user(id: $userId) {
            tournaments(query: {
                page: $page
                perPage: $perPage
            }) {
                pageInfo { total totalPages }
                nodes {
                    id name slug
                    city addrState countryCode
                    venueName venueAddress
                    timezone isOnline numAttendees
                    lat lng state
                    startAt endAt
                    events {
                        id name numEntrants startAt
                        slug state isOnline type
                        teamRosterSize { minPlayers maxPlayers }
                        videogame { id name }
                    }
                }
            }
        }
    }"#;
```

- [ ] **Step 2: Import the new vars struct**

In `backend/crates/common/src/startgg/operations.rs`, update the `use super::queries::{ ... }` import block to include `TournamentsByUserAllGamesVars`:

```rust
use super::queries::{
    CurrentUserData, EntrantPage, EventEntrantsData, EventEntrantsVars, EventPhasesData,
    EventPhasesVars, EventSetsData, EventSetsVars, GameNode, GameSearchData, GameSearchVars,
    NoVars, PhaseNode, SetPage, TournamentAllEventNode, TournamentAllEventsData,
    TournamentAllEventsVars, TournamentEntrant, TournamentEntrantListData,
    TournamentEntrantListVars, TournamentEntrantOrdered, TournamentEventWithEntrants,
    TournamentEventsData, TournamentEventsVars, TournamentPage, TournamentParticipant,
    TournamentParticipantsData, TournamentParticipantsVars, TournamentsByUserAllGamesVars,
    TournamentsByUserData, TournamentsByUserVars, UserBySlugData, UserBySlugVars, UserNode,
};
```

- [ ] **Step 3: Add `tournaments_by_user_all_games` method to `StartggClient`**

In `backend/crates/common/src/startgg/operations.rs`, after the existing `tournaments_by_user` method, add:

```rust
#[instrument(skip(self))]
pub async fn tournaments_by_user_all_games(
    &self,
    user_id: i64,
    page: i32,
    per_page: i32,
) -> Result<TournamentPage, StartggError> {
    let t = Instant::now();
    let data: TournamentsByUserData = self
        .gql(
            TOURNAMENTS_BY_USER_ALL_GAMES_QUERY,
            TournamentsByUserAllGamesVars {
                user_id,
                page,
                per_page,
            },
        )
        .await?;
    tracing::debug!(
        elapsed_ms = t.elapsed().as_millis(),
        "startgg query complete"
    );
    Ok(data
        .user
        .map(|u| u.tournaments)
        .unwrap_or_else(|| TournamentPage {
            page_info: None,
            nodes: vec![],
        }))
}
```

- [ ] **Step 4: Verify the workspace compiles**

```bash
cd backend && cargo build 2>&1 | tail -20
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/common/src/startgg/operations.rs
git commit -m "feat(common): add tournaments_by_user_all_games GQL query and method"
```

---

### Task 3: Update `import_event` and `import_tournament` to take `Option<i64>` game_id

**Files:**
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Update `import_event` signature and game resolution**

In `backend/crates/worker/src/import.rs`, find `import_event` and change the signature and game resolution at the top of the function body:

Change:
```rust
async fn import_event(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    tournament_db_id: Uuid,
    event: &EventNode,
    game_id: i64,
    game_name: Option<&str>,
    account_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<()> {
    let start_at = event.start_at.map(ts_to_dt);
```

To:
```rust
async fn import_event(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    tournament_db_id: Uuid,
    event: &EventNode,
    game_id: Option<i64>,
    game_name: Option<&str>,
    account_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<()> {
    let effective_game_id = game_id.or_else(|| event.videogame.as_ref().map(|v| v.id));
    let effective_game_name =
        game_name.or_else(|| event.videogame.as_ref().map(|v| v.name.as_str()));
    let start_at = event.start_at.map(ts_to_dt);
```

Then in the `sqlx::query!` call inside the same function, replace the two `game_id` and `game_name` bind params:

Change:
```rust
        game_id,
        game_name,
```

To:
```rust
        effective_game_id,
        effective_game_name,
```

- [ ] **Step 2: Update `import_tournament` to pass `Option<i64>` game_id**

In the same file, update the `import_tournament` signature:

Change:
```rust
async fn import_tournament(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    tournament: &TournamentNode,
    game_id: i64,
    game_name: Option<&str>,
    account_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<()> {
```

To:
```rust
async fn import_tournament(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    tournament: &TournamentNode,
    game_id: Option<i64>,
    game_name: Option<&str>,
    account_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<()> {
```

The call to `import_event` inside `import_tournament` passes `game_id` directly — no change needed there since both are now `Option<i64>`.

- [ ] **Step 3: Verify the workspace compiles**

```bash
cd backend && cargo build 2>&1 | tail -20
```

Expected: possible errors at call sites in `run` where `game_id` (still `i64` after the unwrap) is passed. Those will be fixed in Task 4.

- [ ] **Step 4: Update call site in `run` for the existing Some(game_id) path**

In `import.rs`, the `run` function currently calls:

```rust
import_tournament(
    pool,
    startgg,
    project_id,
    tournament,
    game_id,
    project.game_name.as_deref(),
    &account_map,
)
.await?;
```

Where `game_id` is an unwrapped `i64`. Wrap it temporarily so it compiles:

```rust
import_tournament(
    pool,
    startgg,
    project_id,
    tournament,
    Some(game_id),
    project.game_name.as_deref(),
    &account_map,
)
.await?;
```

Note: Task 4 will replace this call site entirely when it rewrites the `run` function's branching logic — the `Some(game_id)` wrapping is a short-lived intermediate state.

- [ ] **Step 5: Verify the workspace compiles cleanly**

```bash
cd backend && cargo build 2>&1 | tail -20
```

Expected: no errors.

- [ ] **Step 6: Update the sqlx offline cache**

```bash
bash backend/prepare-sqlx.sh
```

Expected: `Done` — no errors.

- [ ] **Step 7: Run the full backend test suite to verify nothing is broken**

```bash
bash backend/test.sh 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/worker/src/import.rs backend/.sqlx/
git commit -m "refactor(worker): change game_id to Option<i64> in import_event and import_tournament"
```

---

### Task 4: Add `collect_user_tournaments_all_games` and branch in `run`

**Files:**
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Add `collect_user_tournaments_all_games`**

In `backend/crates/worker/src/import.rs`, after `collect_user_tournaments`, add:

```rust
#[instrument(skip(startgg, seen), fields(startgg_user_id = user_id))]
async fn collect_user_tournaments_all_games(
    startgg: &StartggClient,
    user_id: i64,
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
                .tournaments_by_user_all_games(user_id, page, per_page)
                .await
            {
                Err(StartggError::ComplexityTooHigh { actual, limit }) if per_page > 1 => {
                    tracing::warn!(
                        per_page,
                        actual,
                        limit,
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

    tracing::info!(scanned, newly_added, "user tournaments scanned (all games)");
    Ok(())
}
```

- [ ] **Step 2: Update `run` to remove the early-return guard and branch on `game_id`**

In the `run` function, replace:

```rust
    let Some(game_id) = project.game_id else {
        tracing::warn!(%project_id, "project has no game_id set, skipping import");
        return Ok(());
    };

    // Build startgg_user_id → player_id map for this project
    // ...
    // Phase 1: discover all unique tournaments across all players
    let mut seen: HashMap<i64, TournamentNode> = HashMap::new();
    let total_players = user_ids.len();
    for (i, user_id) in user_ids.iter().enumerate() {
        collect_user_tournaments(
            startgg,
            *user_id,
            game_id,
            params.after_date,
            params.before_date,
            &mut seen,
        )
        .await?;
        update_progress(pool, job_id, "scanning", i + 1, total_players).await?;
    }
```

With:

```rust
    // Build startgg_user_id → player_id map for this project
    // ...
    // Phase 1: discover all unique tournaments across all players
    let mut seen: HashMap<i64, TournamentNode> = HashMap::new();
    let total_players = user_ids.len();
    for (i, user_id) in user_ids.iter().enumerate() {
        if let Some(game_id) = project.game_id {
            collect_user_tournaments(
                startgg,
                *user_id,
                game_id,
                params.after_date,
                params.before_date,
                &mut seen,
            )
            .await?;
        } else {
            collect_user_tournaments_all_games(
                startgg,
                *user_id,
                params.after_date,
                params.before_date,
                &mut seen,
            )
            .await?;
        }
        update_progress(pool, job_id, "scanning", i + 1, total_players).await?;
    }
```

Also update Phase 2's `import_tournament` call to pass `project.game_id` directly (it's already `Option<i64>` now):

```rust
    for (i, (_, tournament)) in seen.iter().enumerate() {
        import_tournament(
            pool,
            startgg,
            project_id,
            tournament,
            project.game_id,
            project.game_name.as_deref(),
            &account_map,
        )
        .await?;
        update_progress(pool, job_id, "importing", i + 1, total_tournaments).await?;
    }
```

(Remove the `Some(game_id)` wrapping added in Task 3 Step 4 — `project.game_id` is already `Option<i64>`.)

- [ ] **Step 3: Verify the workspace compiles**

```bash
cd backend && cargo build 2>&1 | tail -20
```

Expected: no errors.

- [ ] **Step 4: Run the full backend test suite**

```bash
bash backend/test.sh 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/worker/src/import.rs
git commit -m "feat(worker): import all games when project has no game_id set"
```

---

### Task 5: Add e2e test for the all-games import path

**Files:**
- Modify: `backend/crates/e2e/tests/full_flow.rs`

- [ ] **Step 1: Write the test**

Add the following test at the end of `backend/crates/e2e/tests/full_flow.rs`:

```rust
/// Regression: a project with no game_id should import all tournaments/events
/// and store per-event game_id/game_name from the start.gg videogame field.
#[sqlx::test(migrations = "../../migrations")]
async fn import_no_game_filter_flow(pool: PgPool) {
    let mock = MockServer::start().await;

    // user_by_slug("mango")
    Mock::given(method("POST"))
        .and(body_string_contains("\"mango\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "user": { "id": 12345_i64, "player": { "gamerTag": "Mango" } } }
        })))
        .mount(&mock)
        .await;

    // tournaments_by_user_all_games — identified by the videogame sub-selection
    Mock::given(method("POST"))
        .and(body_string_contains("videogame { id name }"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "user": {
                    "tournaments": {
                        "pageInfo": { "total": 1, "totalPages": 1 },
                        "nodes": [{
                            "id": 1001_i64,
                            "name": "Test Tournament",
                            "slug": "tournament/test-2024",
                            "city": "San Jose",
                            "addrState": "CA",
                            "countryCode": "US",
                            "venueName": null,
                            "venueAddress": null,
                            "timezone": "America/Los_Angeles",
                            "isOnline": false,
                            "numAttendees": 8,
                            "startAt": 1700000000_i64,
                            "endAt":   1700086400_i64,
                            "events": [{
                                "id": 2001_i64,
                                "name": "Melee Singles",
                                "numEntrants": 2,
                                "startAt": 1700040000_i64,
                                "slug": "tournament/test-2024/event/melee-singles",
                                "state": "COMPLETED",
                                "isOnline": false,
                                "type": 1,
                                "teamRosterSize": null,
                                "videogame": { "id": 1, "name": "Super Smash Bros. Melee" }
                            }]
                        }]
                    }
                }
            }
        })))
        .mount(&mock)
        .await;

    // event phases
    Mock::given(method("POST"))
        .and(body_string_contains("phaseGroups(query:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "phases": [{
                        "id": 5001_i64,
                        "name": "Bracket",
                        "bracketType": "DOUBLE_ELIMINATION",
                        "phaseOrder": 1,
                        "numSeeds": 1,
                        "groupCount": 1,
                        "state": "COMPLETED",
                        "isExhibition": false,
                        "phaseGroups": {
                            "pageInfo": { "total": 1, "totalPages": 1 },
                            "nodes": [{
                                "id": 6001_i64,
                                "displayIdentifier": "1",
                                "bracketType": "DOUBLE_ELIMINATION",
                                "bracketUrl": null,
                                "numRounds": null,
                                "startAt": null,
                                "firstRoundTime": null,
                                "state": 3
                            }]
                        }
                    }]
                }
            }
        })))
        .mount(&mock)
        .await;

    // event entrants
    Mock::given(method("POST"))
        .and(body_string_contains("entrants(query:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "total": 1, "totalPages": 1 },
                        "nodes": [{
                            "id": 3001_i64,
                            "initialSeedNum": 1,
                            "isDisqualified": false,
                            "standing": { "placement": 1 },
                            "participants": [{ "gamerTag": "Mango", "user": { "id": 12345_i64 } }]
                        }]
                    }
                }
            }
        })))
        .mount(&mock)
        .await;

    // event sets — empty bracket (no sets to import)
    Mock::given(method("POST"))
        .and(body_string_contains("sets(page:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "sets": {
                        "pageInfo": { "total": 0, "totalPages": 1 },
                        "nodes": []
                    }
                }
            }
        })))
        .mount(&mock)
        .await;

    let base_url = mock.uri();
    let app = make_app(pool.clone(), &base_url);

    // ── Setup ─────────────────────────────────────────────────────────────────

    let cookie = register(&app, "gameuser", "pass1234").await;
    set_startgg_api_key(&pool, &cookie, "test-key").await;

    // Create project with NO game_id
    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({ "name": "All Games PR" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project = read_json(resp).await;
    let project_id = project["id"].as_str().unwrap().to_string();

    // Add Mango and link start.gg account
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let mango_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{mango_id}/accounts"),
        &cookie,
        json!({"handle": "user/mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // ── Import ────────────────────────────────────────────────────────────────

    let project_uuid = Uuid::parse_str(&project_id).unwrap();
    let startgg_worker = StartggClient::new_with_base_url("test-key".into(), base_url.into());
    worker::import::run(
        &pool,
        &startgg_worker,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    // ── Assertions ────────────────────────────────────────────────────────────

    // Event was created with per-event game_id and game_name from start.gg
    let event_row = sqlx::query!(
        r#"SELECT e.game_id, e.game_name
           FROM events e
           JOIN tournaments t ON t.id = e.tournament_id
           WHERE t.startgg_id = 1001"#
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(event_row.game_id, Some(1_i64));
    assert_eq!(event_row.game_name.as_deref(), Some("Super Smash Bros. Melee"));
}
```

- [ ] **Step 2: Run the new test to verify it passes**

```bash
bash backend/test.sh 2>&1 | tail -30
```

Expected: all tests pass including `import_no_game_filter_flow`.

- [ ] **Step 3: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/e2e/tests/full_flow.rs
git commit -m "test(e2e): add import_no_game_filter_flow test for all-games import path"
```
