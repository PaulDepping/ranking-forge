# Tournament Tab — All Registrants + Per-Event Filtering — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the game-filtered tournament entrant endpoint with one that returns all registrants (including spectators) plus per-event entrants with seed/placement; rework `TournamentTab.svelte` to show "All" + per-event tabs with a seed/placement sort toggle.

**Architecture:** Two new `StartggClient` functions (`tournament_participants`, `tournament_events_with_entrants`) replace the game-filtered path in the API route. The frontend uses shadcn `Tabs` for event switching with a shared `Set<number>` selection across tabs. A single fetch call returns everything; per-event sort is derived client-side.

**Tech Stack:** Rust/Axum (backend), Svelte 5 runes + shadcn-svelte (frontend), wiremock (backend tests), Playwright (e2e tests).

**Spec:** `docs/superpowers/specs/2026-05-17-tournament-tab-all-registrants-design.md`

---

## File Map

| File | Change |
|---|---|
| `backend/crates/common/src/startgg/queries.rs` | Add new query/response structs; extend `TournamentEntrantNode` with seed/placement fields |
| `backend/crates/common/src/startgg/operations.rs` | Add two new GraphQL query constants and two new `StartggClient` methods |
| `backend/crates/common/src/startgg/mod.rs` | Export new public output types |
| `backend/crates/api/src/routes/players.rs` | New response types; rewrite `list_tournament_entrants` handler |
| `backend/crates/api/tests/api.rs` | Update two existing tests; add one new test |
| `web/src/lib/types.ts` | Add `TournamentParticipant`, `TournamentEntrantOrdered`, `TournamentEventData`, `TournamentData`; remove old `TournamentEntrant` |
| `web/tests/mock-api.js` | Update `MOCK_ENTRANTS` to new response shape |
| `web/src/lib/components/TournamentTab.svelte` | Full rework: event tabs, sort toggle, shared selection |
| `web/tests/projects.test.ts` | Add e2e test for the new flow |

---

## Task 1: Extend query types in `common`

**Files:**
- Modify: `backend/crates/common/src/startgg/queries.rs`
- Modify: `backend/crates/common/src/startgg/mod.rs`

- [ ] **Step 1: Extend `TournamentEntrantNode` with seed and placement fields**

In `queries.rs`, the existing struct lacks seed/placement. Add `#[serde(rename_all = "camelCase")]` so `initialSeedNum` deserializes correctly, and add the two new optional fields along with a new standing struct:

```rust
// Replace the existing TournamentEntrantNode and add the standing struct:

#[derive(Deserialize)]
pub(crate) struct TournamentEntrantNodeStanding {
    pub placement: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TournamentEntrantNode {
    pub participants: Option<Vec<TournamentEntrantParticipant>>,
    pub initial_seed_num: Option<i32>,
    pub standing: Option<TournamentEntrantNodeStanding>,
}
```

- [ ] **Step 2: Add participant query structs**

Append to `queries.rs` after the `// ── Tournament entrants` section:

```rust
// ── Tournament participants (all registrants) ─────────────────────────────────

#[derive(Serialize)]
pub(crate) struct TournamentParticipantsVars {
    pub slug: String,
    pub page: i32,
    #[serde(rename = "perPage")]
    pub per_page: i32,
}

#[derive(Deserialize)]
pub(crate) struct TournamentParticipantsData {
    pub tournament: Option<TournamentWithParticipants>,
}

#[derive(Deserialize)]
pub(crate) struct TournamentWithParticipants {
    pub participants: Option<TournamentParticipantPage>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TournamentParticipantPage {
    pub page_info: PageInfo,
    pub nodes: Vec<TournamentParticipantNode>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TournamentParticipantNode {
    pub gamer_tag: String,
    pub user: Option<TournamentParticipantUser>,
}

#[derive(Deserialize)]
pub(crate) struct TournamentParticipantUser {
    pub id: i64,
    pub slug: String,
}

/// Public output type for `tournament_participants`.
#[derive(Debug, Clone)]
pub struct TournamentParticipant {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
}

// ── Tournament all-events (no game filter) ────────────────────────────────────

#[derive(Serialize)]
pub(crate) struct TournamentAllEventsVars {
    pub slug: String,
}

#[derive(Deserialize)]
pub(crate) struct TournamentAllEventsData {
    pub tournament: Option<TournamentWithAllEvents>,
}

#[derive(Deserialize)]
pub(crate) struct TournamentWithAllEvents {
    pub events: Option<Vec<TournamentAllEventNode>>,
}

#[derive(Deserialize)]
pub(crate) struct TournamentAllEventNode {
    pub id: i64,
    pub name: String,
}

/// Public output type for `tournament_events_with_entrants`.
#[derive(Debug, Clone)]
pub struct TournamentEntrantOrdered {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
    pub seed: Option<i32>,
    pub placement: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct TournamentEventWithEntrants {
    pub id: i64,
    pub name: String,
    pub entrants: Vec<TournamentEntrantOrdered>,
}
```

- [ ] **Step 3: Export new public types from `mod.rs`**

The `pub use queries::{...}` block in `mod.rs` currently ends with `TournamentEntrant,`. Extend it to also export the new types:

```rust
pub use queries::{
    EntrantNode, EntrantPage, EntrantStanding, EventNode, GameNode, PageInfo, Participant,
    ParticipantUser, PhaseGroupNode, PhaseNode, ScoreValue, SetNode, SetPage, SetPhaseGroup,
    SetSlot, SlotEntrant, SlotStanding, SlotStats, TeamRosterSize, TournamentEntrant,
    TournamentEntrantOrdered, TournamentEventWithEntrants, TournamentNode, TournamentPage,
    TournamentParticipant,
};
```

(Keep all existing exports; add `TournamentEntrantOrdered`, `TournamentEventWithEntrants`, `TournamentParticipant` to the list.)

- [ ] **Step 4: Verify existing tests still pass**

Run: `cd backend && cargo test -p common`

Expected: all tests pass. The new optional fields on `TournamentEntrantNode` are backward-compatible — existing mock responses that omit `initialSeedNum`/`standing` will deserialize to `None`.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/common/src/startgg/queries.rs backend/crates/common/src/startgg/mod.rs
git commit -m "feat(common): add participant/all-events query types and ordered entrant output types"
```

---

## Task 2: Add new StartGG GraphQL constants

**Files:**
- Modify: `backend/crates/common/src/startgg/operations.rs`

- [ ] **Step 1: Update `TOURNAMENT_ENTRANT_LIST_QUERY` to fetch seed and placement**

The current query doesn't request `initialSeedNum` or `standing`. Replace it:

```rust
const TOURNAMENT_ENTRANT_LIST_QUERY: &str = r#"
    query($eventId: ID!, $page: Int!, $perPage: Int!) {
        event(id: $eventId) {
            entrants(query: { page: $page, perPage: $perPage }) {
                pageInfo { totalPages }
                nodes {
                    initialSeedNum
                    standing { placement }
                    participants {
                        gamerTag
                        user { id slug }
                    }
                }
            }
        }
    }"#;
```

- [ ] **Step 2: Add two new query constants**

Append after `TOURNAMENT_ENTRANT_LIST_QUERY`:

```rust
const TOURNAMENT_PARTICIPANTS_QUERY: &str = r#"
    query($slug: String!, $page: Int!, $perPage: Int!) {
        tournament(slug: $slug) {
            participants(query: { page: $page, perPage: $perPage }) {
                pageInfo { totalPages }
                nodes {
                    gamerTag
                    user { id slug }
                }
            }
        }
    }"#;

const TOURNAMENT_ALL_EVENTS_QUERY: &str = r#"
    query($slug: String!) {
        tournament(slug: $slug) {
            events {
                id name
            }
        }
    }"#;
```

- [ ] **Step 3: Add new imports to `operations.rs`**

The new functions use types that need to be imported. Add to the existing `use super::queries::{...}` import:

```rust
use super::queries::{
    EntrantPage, EventEntrantsData, EventEntrantsVars, EventPhasesData, EventPhasesVars,
    EventSetsData, EventSetsVars, GameNode, GameSearchData, GameSearchVars, PhaseNode, SetPage,
    TournamentAllEventsData, TournamentAllEventsVars, TournamentAllEventNode,
    TournamentEntrant, TournamentEntrantListData, TournamentEntrantListVars,
    TournamentEntrantOrdered, TournamentEventWithEntrants,
    TournamentEventsData, TournamentEventsVars,
    TournamentPage, TournamentParticipant,
    TournamentParticipantsData, TournamentParticipantsVars,
    TournamentsByUserData, TournamentsByUserVars,
    UserBySlugData, UserBySlugVars, UserNode,
};
```

- [ ] **Step 4: Verify it compiles**

Run: `cd backend && cargo build -p common`

Expected: compiles without errors (no logic added yet, just constants and imports).

- [ ] **Step 5: Commit**

```bash
git add backend/crates/common/src/startgg/operations.rs
git commit -m "feat(common): add TOURNAMENT_PARTICIPANTS_QUERY and TOURNAMENT_ALL_EVENTS_QUERY constants"
```

---

## Task 3: Implement `tournament_participants` with tests

**Files:**
- Modify: `backend/crates/common/src/startgg/operations.rs`
- Modify: `backend/crates/common/src/startgg/mod.rs` (tests section)

- [ ] **Step 1: Write the failing test**

The tests live in the `#[cfg(test)] mod tests` block at the bottom of `mod.rs`. Add after the last `tournament_entrants_*` test:

```rust
// ── tournament_participants ───────────────────────────────────────────────────

#[tokio::test]
async fn tournament_participants_returns_all_with_user() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "tournament": {
                    "participants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": [
                            { "gamerTag": "Mang0", "user": { "id": 1001, "slug": "user/mang0" } },
                            { "gamerTag": "Spectator", "user": { "id": 9999, "slug": "user/spec" } },
                            { "gamerTag": "Guest", "user": null }
                        ]
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let result = client(&mock.uri())
        .tournament_participants("some-weekly")
        .await
        .unwrap();

    // Guest (no user) is skipped; Mang0 and Spectator are both included
    assert_eq!(result.len(), 2);
    let handles: Vec<&str> = result.iter().map(|p| p.handle.as_str()).collect();
    assert!(handles.contains(&"mang0"));
    assert!(handles.contains(&"spec"));
}

#[tokio::test]
async fn tournament_participants_paginates() {
    let mock = MockServer::start().await;

    // Page 1
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "tournament": {
                    "participants": {
                        "pageInfo": { "totalPages": 2 },
                        "nodes": [
                            { "gamerTag": "Mang0", "user": { "id": 1001, "slug": "user/mang0" } }
                        ]
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Page 2
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "tournament": {
                    "participants": {
                        "pageInfo": { "totalPages": 2 },
                        "nodes": [
                            { "gamerTag": "Armada", "user": { "id": 1002, "slug": "user/armada" } }
                        ]
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let result = client(&mock.uri())
        .tournament_participants("some-weekly")
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd backend && cargo test -p common tournament_participants`

Expected: compile error "no method named `tournament_participants`"

- [ ] **Step 3: Implement `tournament_participants`**

Add after `tournament_entrants` in `operations.rs`:

```rust
pub async fn tournament_participants(
    &self,
    tournament_handle: &str,
) -> Result<Vec<TournamentParticipant>, StartggError> {
    let t = Instant::now();
    let mut result = Vec::new();
    let mut page = 1i32;
    let per_page = 64i32;

    loop {
        let data: TournamentParticipantsData = self
            .gql(
                TOURNAMENT_PARTICIPANTS_QUERY,
                TournamentParticipantsVars {
                    slug: tournament_handle.to_string(),
                    page,
                    per_page,
                },
            )
            .await?;

        let participant_page = match data.tournament.and_then(|t| t.participants) {
            Some(p) => p,
            None => break,
        };

        for node in participant_page.nodes {
            let Some(user) = node.user else { continue };
            let handle = user.slug.trim_start_matches("user/").to_string();
            result.push(TournamentParticipant {
                startgg_user_id: user.id,
                handle,
                name: node.gamer_tag,
            });
        }

        let total_pages = participant_page.page_info.total_pages.unwrap_or(1);
        if page >= total_pages {
            break;
        }
        page += 1;
    }

    tracing::debug!(elapsed_ms = t.elapsed().as_millis(), "startgg query complete");
    Ok(result)
}
```

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cd backend && cargo test -p common tournament_participants`

Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/common/src/startgg/operations.rs backend/crates/common/src/startgg/mod.rs
git commit -m "feat(common): implement tournament_participants"
```

---

## Task 4: Implement `tournament_events_with_entrants` with tests

**Files:**
- Modify: `backend/crates/common/src/startgg/operations.rs`
- Modify: `backend/crates/common/src/startgg/mod.rs` (tests section)

- [ ] **Step 1: Write the failing test**

Add after the `tournament_participants` tests in `mod.rs`:

```rust
// ── tournament_events_with_entrants ───────────────────────────────────────────

#[tokio::test]
async fn tournament_events_with_entrants_returns_events_and_ordered_entrants() {
    let mock = MockServer::start().await;

    // Request 1: all events query
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "tournament": {
                    "events": [
                        { "id": 101, "name": "Melee Singles" }
                    ]
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Request 2: entrants for event 101
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": [
                            {
                                "initialSeedNum": 2,
                                "standing": { "placement": 1 },
                                "participants": [{
                                    "gamerTag": "Mang0",
                                    "user": { "id": 1001, "slug": "user/mang0" }
                                }]
                            },
                            {
                                "initialSeedNum": 1,
                                "standing": { "placement": 2 },
                                "participants": [{
                                    "gamerTag": "Armada",
                                    "user": { "id": 1002, "slug": "user/armada" }
                                }]
                            }
                        ]
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let result = client(&mock.uri())
        .tournament_events_with_entrants("some-weekly")
        .await
        .unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "Melee Singles");
    assert_eq!(result[0].entrants.len(), 2);

    let mang0 = result[0].entrants.iter().find(|e| e.handle == "mang0").unwrap();
    assert_eq!(mang0.seed, Some(2));
    assert_eq!(mang0.placement, Some(1));
}

#[tokio::test]
async fn tournament_events_with_entrants_handles_null_standing() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": { "tournament": { "events": [{ "id": 101, "name": "Melee Singles" }] } }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": [{
                            "initialSeedNum": null,
                            "standing": null,
                            "participants": [{
                                "gamerTag": "Mang0",
                                "user": { "id": 1001, "slug": "user/mang0" }
                            }]
                        }]
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let result = client(&mock.uri())
        .tournament_events_with_entrants("some-weekly")
        .await
        .unwrap();

    let mang0 = &result[0].entrants[0];
    assert_eq!(mang0.seed, None);
    assert_eq!(mang0.placement, None);
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cd backend && cargo test -p common tournament_events_with_entrants`

Expected: compile error "no method named `tournament_events_with_entrants`"

- [ ] **Step 3: Implement `tournament_events_with_entrants`**

Add after `tournament_participants` in `operations.rs`:

```rust
pub async fn tournament_events_with_entrants(
    &self,
    tournament_handle: &str,
) -> Result<Vec<TournamentEventWithEntrants>, StartggError> {
    let t = Instant::now();

    let events_data: TournamentAllEventsData = self
        .gql(
            TOURNAMENT_ALL_EVENTS_QUERY,
            TournamentAllEventsVars { slug: tournament_handle.to_string() },
        )
        .await?;

    let event_nodes: Vec<TournamentAllEventNode> = events_data
        .tournament
        .and_then(|t| t.events)
        .unwrap_or_default();

    let mut result = Vec::new();

    for event_node in event_nodes {
        let mut entrants = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut page = 1i32;
        let mut per_page = 64i32;

        'pages: loop {
            let data: TournamentEntrantListData = match self
                .gql(
                    TOURNAMENT_ENTRANT_LIST_QUERY,
                    TournamentEntrantListVars { event_id: event_node.id, page, per_page },
                )
                .await
            {
                Err(StartggError::ComplexityTooHigh { actual, limit }) if per_page > 1 => {
                    tracing::warn!(per_page, actual, limit, "complexity too high, halving perPage");
                    per_page /= 2;
                    continue 'pages;
                }
                other => other?,
            };

            let entrant_data = match data.event.and_then(|e| e.entrants) {
                Some(d) => d,
                None => break,
            };

            for node in entrant_data.nodes {
                let seed = node.initial_seed_num;
                let placement = node.standing.as_ref().and_then(|s| s.placement);
                let Some(participants) = node.participants else { continue };
                for participant in participants {
                    let Some(user) = participant.user else { continue };
                    if seen.insert(user.id) {
                        let handle = user.slug.trim_start_matches("user/").to_string();
                        entrants.push(TournamentEntrantOrdered {
                            startgg_user_id: user.id,
                            handle,
                            name: participant.gamer_tag,
                            seed,
                            placement,
                        });
                    }
                }
            }

            let total_pages = entrant_data.page_info.total_pages.unwrap_or(1);
            if page >= total_pages {
                break;
            }
            page += 1;
        }

        result.push(TournamentEventWithEntrants {
            id: event_node.id,
            name: event_node.name,
            entrants,
        });
    }

    tracing::debug!(elapsed_ms = t.elapsed().as_millis(), "startgg query complete");
    Ok(result)
}
```

- [ ] **Step 4: Run all common tests**

Run: `cd backend && cargo test -p common`

Expected: all tests pass including the new ones.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/common/src/startgg/operations.rs backend/crates/common/src/startgg/mod.rs
git commit -m "feat(common): implement tournament_events_with_entrants"
```

---

## Task 5: Update API route — new response types + handler

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Write the failing test for the new response shape**

In `api/tests/api.rs`, the old `test_list_tournament_entrants` test asserts a flat array. Replace the entire `test_list_tournament_entrants` function (starting at `#[sqlx::test` on line ~1417) with this updated version:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_list_tournament_entrants(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;

    // Mock 1: tournament_participants query
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "tournament": {
                    "participants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": [
                            { "gamerTag": "Mang0", "user": { "id": 1001, "slug": "user/mang0" } },
                            { "gamerTag": "Spectator", "user": { "id": 9999, "slug": "user/spec" } }
                        ]
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Mock 2: tournament_events_with_entrants — all events query
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "tournament": {
                    "events": [{ "id": 999, "name": "Melee Singles" }]
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Mock 3: entrant list for event 999
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": [{
                            "initialSeedNum": 1,
                            "standing": { "placement": 1 },
                            "participants": [{
                                "gamerTag": "Mang0",
                                "user": { "id": 1001, "slug": "user/mang0" }
                            }]
                        }]
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;

    // Project without game_id — endpoint must work without one
    let pid = create_project(&app, &cookie).await;

    let resp = get_req(
        &app,
        &format!("/projects/{pid}/tournament-entrants?tournament=some-weekly"),
        &cookie,
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;

    let participants = body["all_participants"].as_array().unwrap();
    assert_eq!(participants.len(), 2);
    assert!(participants.iter().any(|p| p["handle"] == "mang0"));
    assert!(participants.iter().any(|p| p["handle"] == "spec"));

    let events = body["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["name"], "Melee Singles");

    let entrants = events[0]["entrants"].as_array().unwrap();
    assert_eq!(entrants.len(), 1);
    assert_eq!(entrants[0]["handle"], "mang0");
    assert_eq!(entrants[0]["seed"], 1);
    assert_eq!(entrants[0]["placement"], 1);
}
```

Also update `test_list_tournament_entrants_normalizes_url` (line ~1489) to use the new 3-mock pattern and assert the new shape:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_list_tournament_entrants_normalizes_url(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;

    // Participants query
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "tournament": { "participants": { "pageInfo": { "totalPages": 1 }, "nodes": [] } } }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // All events query
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "tournament": { "events": [] } }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = get_req(
        &app,
        &format!(
            "/projects/{pid}/tournament-entrants?tournament=https%3A%2F%2Fwww.start.gg%2Ftournament%2Fsome-weekly%2Fevent%2Fmelee-singles"
        ),
        &cookie,
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    // Normalized to "some-weekly"; empty but correct shape
    assert!(body["all_participants"].as_array().unwrap().is_empty());
    assert!(body["events"].as_array().unwrap().is_empty());
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cd backend && cargo test -p api -- test_list_tournament_entrants 2>&1 | head -30`

Expected: compile error because the handler still returns the old flat array.

- [ ] **Step 3: Add new response types to `players.rs`**

Replace the `// ── Tournament entrants` section's response types (the `TournamentEntrantResponse` struct and `TournamentEntrantsQuery`). Keep `TournamentEntrantsQuery` unchanged, add new response structs:

```rust
// ── Tournament entrants ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct TournamentEntrantsQuery {
    pub tournament: String,
}

#[derive(Serialize)]
pub struct TournamentDataResponse {
    pub all_participants: Vec<TournamentParticipantResp>,
    pub events: Vec<TournamentEventResp>,
}

#[derive(Serialize)]
pub struct TournamentParticipantResp {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct TournamentEventResp {
    pub id: i64,
    pub name: String,
    pub entrants: Vec<TournamentEntrantOrderedResp>,
}

#[derive(Serialize)]
pub struct TournamentEntrantOrderedResp {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
    pub seed: Option<i32>,
    pub placement: Option<i32>,
}
```

You'll also need to add the `common` types to the imports at the top of `players.rs`. The existing import is:
```rust
use common::models::{Player, StartggAccount};
```
Add:
```rust
use common::{TournamentEntrantOrdered, TournamentEventWithEntrants, TournamentParticipant};
```
(Or just prefix them as `common::TournamentParticipant` inline — either works.)

- [ ] **Step 4: Rewrite `list_tournament_entrants` handler**

Replace the existing `list_tournament_entrants` function body:

```rust
pub async fn list_tournament_entrants(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<TournamentEntrantsQuery>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, id, user.id).await?;

    let handle = normalize_tournament_handle(&q.tournament);

    let participants = state
        .startgg
        .tournament_participants(&handle)
        .await
        .map_err(AppError::from)?;

    let events = state
        .startgg
        .tournament_events_with_entrants(&handle)
        .await
        .map_err(AppError::from)?;

    let all_participants: Vec<TournamentParticipantResp> = participants
        .into_iter()
        .map(|p| TournamentParticipantResp {
            startgg_user_id: p.startgg_user_id,
            handle: p.handle,
            name: p.name,
        })
        .collect();

    let events: Vec<TournamentEventResp> = events
        .into_iter()
        .map(|e| TournamentEventResp {
            id: e.id,
            name: e.name,
            entrants: e
                .entrants
                .into_iter()
                .map(|en| TournamentEntrantOrderedResp {
                    startgg_user_id: en.startgg_user_id,
                    handle: en.handle,
                    name: en.name,
                    seed: en.seed,
                    placement: en.placement,
                })
                .collect(),
        })
        .collect();

    Ok(Json(TournamentDataResponse { all_participants, events }))
}
```

Also remove the now-unused `game_id` lookup. The old handler called `project.game_id.ok_or_else(...)` — delete that line and the `require_project` call already suffices for auth.

- [ ] **Step 5: Run the API tests**

Run: `cd backend && cargo test -p api -- test_list_tournament_entrants`

Expected: both tests pass.

- [ ] **Step 6: Run the full backend test suite**

Run: `bash backend/test.sh`

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/api/src/routes/players.rs backend/crates/api/tests/api.rs
git commit -m "feat(api): rewrite list_tournament_entrants to return all participants + per-event entrants"
```

---

## Task 6: Update frontend types and mock API

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/tests/mock-api.js`

- [ ] **Step 1: Update `types.ts`**

The existing `TournamentEntrant` interface (around line 31) is only used in `TournamentTab.svelte`. Replace it and add the new types:

```typescript
// Replace existing TournamentEntrant:
export interface TournamentParticipant {
  startgg_user_id: number;
  handle: string;
  name: string;
}

// Add new types:
export interface TournamentEntrantOrdered {
  startgg_user_id: number;
  handle: string;
  name: string;
  seed: number | null;
  placement: number | null;
}

export interface TournamentEventData {
  id: number;
  name: string;
  entrants: TournamentEntrantOrdered[];
}

export interface TournamentData {
  all_participants: TournamentParticipant[];
  events: TournamentEventData[];
}
```

- [ ] **Step 2: Update `mock-api.js`**

`MOCK_ENTRANTS` (line 21) is currently a flat array. Replace it with the new shape:

```javascript
const MOCK_ENTRANTS = {
    all_participants: [
        { startgg_user_id: 1001, handle: 'mang0', name: 'Mang0' },
        { startgg_user_id: 1002, handle: 'armada', name: 'Armada' },
        { startgg_user_id: 9999, handle: 'spectator', name: 'Spectator' }
    ],
    events: [
        {
            id: 101,
            name: 'Melee Singles',
            entrants: [
                { startgg_user_id: 1001, handle: 'mang0', name: 'Mang0', seed: 1, placement: 2 },
                { startgg_user_id: 1002, handle: 'armada', name: 'Armada', seed: 2, placement: 1 }
            ]
        },
        {
            id: 102,
            name: 'Doubles',
            entrants: [
                { startgg_user_id: 1001, handle: 'mang0', name: 'Mang0', seed: 1, placement: 1 }
            ]
        }
    ]
};
```

- [ ] **Step 3: Run e2e tests to confirm nothing broke**

Run: `cd web && npm run test:e2e`

Expected: all existing e2e tests pass. (The `TournamentTab.svelte` will fail to compile if it still references `TournamentEntrant` — fix the import to use `TournamentParticipant` as a temporary patch: just update the import line in `TournamentTab.svelte` to `import type { Player, TournamentParticipant } from '$lib/types';` and change the `entrants` state type to `TournamentParticipant[]` without any other logic changes yet.)

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/types.ts web/tests/mock-api.js web/src/lib/components/TournamentTab.svelte
git commit -m "feat(web): update types and mock API for new tournament-entrants response shape"
```

---

## Task 7: Rework `TournamentTab.svelte`

**Files:**
- Modify: `web/src/lib/components/TournamentTab.svelte`

- [ ] **Step 1: Replace the component with the full rework**

Write the complete new `TournamentTab.svelte`:

```svelte
<script lang="ts">
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';
    import { Badge } from '$lib/components/ui/badge';
    import { Checkbox } from '$lib/components/ui/checkbox';
    import { ScrollArea } from '$lib/components/ui/scroll-area';
    import * as Tabs from '$lib/components/ui/tabs';
    import { PUBLIC_API_URL } from '$env/static/public';
    import { makeApi } from '$lib/api';
    import { invalidateAll } from '$app/navigation';
    import type { Player, TournamentData, TournamentParticipant, TournamentEntrantOrdered } from '$lib/types';

    let {
        projectId,
        players,
        onClose
    }: { projectId: string; players: Player[]; onClose: () => void } = $props();

    let tournamentInput = $state('');
    let loading = $state(false);
    let fetchError = $state<string | null>(null);
    let tournamentData = $state<TournamentData | null>(null);
    let activeTab = $state('all');
    let sortMode = $state<'placement' | 'seed'>('placement');
    let search = $state('');
    let selected = $state(new Set<number>());
    let submitting = $state(false);
    let addError = $state<string | null>(null);

    const alreadyAddedIds = $derived(
        new Set(players.flatMap((p) => p.accounts.map((a) => a.startgg_user_id)))
    );

    // Flat lookup of every known entrant by startgg_user_id (for add-selected)
    const allEntrantMap = $derived.by(() => {
        const map = new Map<number, { startgg_user_id: number; handle: string; name: string }>();
        if (!tournamentData) return map;
        for (const p of tournamentData.all_participants) map.set(p.startgg_user_id, p);
        for (const ev of tournamentData.events) {
            for (const e of ev.entrants) {
                if (!map.has(e.startgg_user_id)) map.set(e.startgg_user_id, e);
            }
        }
        return map;
    });

    type DisplayEntrant = (TournamentParticipant | TournamentEntrantOrdered) & { seed?: number | null; placement?: number | null };

    const visibleEntrants = $derived.by((): DisplayEntrant[] => {
        if (!tournamentData) return [];
        if (activeTab === 'all') {
            return [...tournamentData.all_participants].sort((a, b) =>
                a.name.localeCompare(b.name)
            );
        }
        const ev = tournamentData.events.find((e) => String(e.id) === activeTab);
        if (!ev) return [];
        return [...ev.entrants].sort((a, b) => {
            const va = sortMode === 'placement' ? a.placement : a.seed;
            const vb = sortMode === 'placement' ? b.placement : b.seed;
            if (va == null && vb == null) return 0;
            if (va == null) return 1;
            if (vb == null) return -1;
            return va - vb;
        });
    });

    const filteredEntrants = $derived(
        visibleEntrants.filter((e) => {
            const q = search.toLowerCase();
            return e.name.toLowerCase().includes(q) || e.handle.toLowerCase().includes(q);
        })
    );

    const selectedCount = $derived(selected.size);
    const alreadyAddedCount = $derived(
        filteredEntrants.filter((e) => alreadyAddedIds.has(e.startgg_user_id)).length
    );
    const selectableFiltered = $derived(
        filteredEntrants.filter((e) => !alreadyAddedIds.has(e.startgg_user_id))
    );
    const allSelected = $derived(
        selectableFiltered.length > 0 &&
        selectableFiltered.every((e) => selected.has(e.startgg_user_id))
    );

    function toggleAll(checked: boolean) {
        const next = new Set(selected);
        if (checked) {
            for (const e of selectableFiltered) next.add(e.startgg_user_id);
        } else {
            for (const e of selectableFiltered) next.delete(e.startgg_user_id);
        }
        selected = next;
    }

    function toggleEntrant(id: number) {
        const next = new Set(selected);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        selected = next;
    }

    function formatRank(e: DisplayEntrant): string {
        const n = sortMode === 'placement' ? e.placement : e.seed;
        if (n == null) return '—';
        if (sortMode === 'seed') return `#${n}`;
        // ordinal for placement
        const mod100 = n % 100;
        const mod10 = n % 10;
        if (mod100 >= 11 && mod100 <= 13) return `${n}th`;
        if (mod10 === 1) return `${n}st`;
        if (mod10 === 2) return `${n}nd`;
        if (mod10 === 3) return `${n}rd`;
        return `${n}th`;
    }

    async function fetchTournamentData() {
        if (!tournamentInput.trim()) return;
        loading = true;
        fetchError = null;
        tournamentData = null;
        activeTab = 'all';
        selected = new Set();
        search = '';
        const api = makeApi(fetch, PUBLIC_API_URL);
        const res = await api.get(
            `/projects/${projectId}/tournament-entrants?tournament=${encodeURIComponent(tournamentInput.trim())}`
        );
        loading = false;
        if (res.ok) {
            tournamentData = await res.json();
        } else {
            const err = await res.json().catch(() => ({ message: 'Failed to fetch entrants' }));
            fetchError = err.message;
        }
    }

    async function addSelected() {
        const entries = [...selected]
            .map((id) => allEntrantMap.get(id))
            .filter((e): e is NonNullable<typeof e> => e != null)
            .map((e) => ({ name: e.name, startgg_user_id: e.startgg_user_id, handle: e.handle }));
        if (!entries.length) return;
        submitting = true;
        addError = null;
        const api = makeApi(fetch, PUBLIC_API_URL);
        const res = await api.post(`/projects/${projectId}/players/bulk`, { players: entries });
        submitting = false;
        if (res.ok) {
            await invalidateAll();
            onClose();
        } else {
            const err = await res.json().catch(() => ({ message: 'Failed to add players' }));
            addError = err.message;
        }
    }
</script>

<div class="space-y-3">
    <div class="flex gap-2">
        <div class="flex-1">
            <Label for="tournament-input" class="sr-only">Tournament URL or slug</Label>
            <Input
                id="tournament-input"
                bind:value={tournamentInput}
                placeholder="genesis-9 or start.gg/tournament/genesis-9"
                disabled={loading}
            />
        </div>
        <Button onclick={fetchTournamentData} disabled={loading || !tournamentInput.trim()}>
            {loading ? 'Fetching…' : 'Fetch'}
        </Button>
    </div>

    {#if fetchError}
        <p class="text-sm text-destructive">{fetchError}</p>
    {/if}

    {#if tournamentData}
        <!-- Event tabs -->
        <Tabs.Root bind:value={activeTab}>
            <div class="overflow-x-auto">
                <Tabs.List class="w-max min-w-full">
                    <Tabs.Trigger value="all">All</Tabs.Trigger>
                    {#each tournamentData.events as ev (ev.id)}
                        <Tabs.Trigger value={String(ev.id)}>{ev.name}</Tabs.Trigger>
                    {/each}
                </Tabs.List>
            </div>
        </Tabs.Root>

        <!-- Search + sort toggle row -->
        <div class="flex gap-2 items-center">
            <Input bind:value={search} placeholder="Search entrants…" class="flex-1" />
            {#if activeTab !== 'all'}
                <div class="flex rounded-md border overflow-hidden flex-shrink-0">
                    <Button
                        variant={sortMode === 'placement' ? 'default' : 'ghost'}
                        size="sm"
                        class="rounded-none h-8 text-xs"
                        onclick={() => (sortMode = 'placement')}
                    >Placement</Button>
                    <Button
                        variant={sortMode === 'seed' ? 'default' : 'ghost'}
                        size="sm"
                        class="rounded-none border-l h-8 text-xs"
                        onclick={() => (sortMode = 'seed')}
                    >Seed</Button>
                </div>
            {/if}
        </div>

        <!-- Select all -->
        <div class="flex items-center gap-2">
            <Checkbox id="select-all" checked={allSelected} onCheckedChange={toggleAll} />
            <Label for="select-all" class="cursor-pointer text-sm font-normal">Select all</Label>
        </div>

        <ScrollArea class="h-52 rounded-md border">
            <div class="divide-y">
                {#each filteredEntrants as entrant (entrant.startgg_user_id)}
                    {@const isAdded = alreadyAddedIds.has(entrant.startgg_user_id)}
                    <div class="flex items-center gap-3 px-3 py-2 text-sm" class:opacity-50={isAdded}>
                        <Checkbox
                            id="entrant-{entrant.startgg_user_id}"
                            checked={selected.has(entrant.startgg_user_id)}
                            disabled={isAdded}
                            onCheckedChange={() => !isAdded && toggleEntrant(entrant.startgg_user_id)}
                        />
                        {#if activeTab !== 'all'}
                            <span class="w-8 text-right text-xs text-muted-foreground flex-shrink-0">
                                {formatRank(entrant)}
                            </span>
                        {/if}
                        <Label
                            for="entrant-{entrant.startgg_user_id}"
                            class="flex flex-1 items-center gap-2 {isAdded ? 'cursor-default' : 'cursor-pointer'}"
                        >
                            <span class="font-medium">{entrant.name}</span>
                            <span class="text-muted-foreground">{entrant.handle}</span>
                        </Label>
                        {#if isAdded}
                            <Badge variant="secondary" class="text-xs">already added</Badge>
                        {/if}
                    </div>
                {/each}
            </div>
        </ScrollArea>

        {#if addError}<p class="text-sm text-destructive">{addError}</p>{/if}
        <div class="flex items-center justify-between">
            <span class="text-sm text-muted-foreground">
                {selectedCount} selected · {alreadyAddedCount} already added
            </span>
            <Button onclick={addSelected} disabled={selectedCount === 0 || submitting}>
                {submitting ? 'Adding…' : `Add ${selectedCount} player${selectedCount === 1 ? '' : 's'}`}
            </Button>
        </div>
    {/if}
</div>
```

- [ ] **Step 2: Run e2e tests**

Run: `cd web && npm run test:e2e`

Expected: all existing tests pass. The "Add players dialog opens with three tabs" test still works because `TournamentTab` still exists and renders inside the same dialog structure.

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/components/TournamentTab.svelte
git commit -m "feat(web): rework TournamentTab with all-registrants, per-event tabs, and seed/placement toggle"
```

---

## Task 8: Add e2e test for the new tournament tab flow

**Files:**
- Modify: `web/tests/projects.test.ts`

- [ ] **Step 1: Write the e2e test**

Add at the end of `projects.test.ts`:

```typescript
test('From tournament tab fetches data and shows All tab and event tabs', async ({ page }) => {
    await page.goto('/projects/proj-1/players');
    await page.waitForLoadState('networkidle');
    await page.getByRole('button', { name: 'Add players' }).click();
    await page.getByRole('tab', { name: 'From tournament' }).click();

    // Type a tournament and fetch
    await page.getByPlaceholder('genesis-9 or start.gg/tournament/genesis-9').fill('genesis-9');
    await page.getByRole('button', { name: 'Fetch' }).click();

    // "All" tab is visible and active
    await expect(page.getByRole('tab', { name: 'All' })).toBeVisible();

    // Event tabs from mock data are visible
    await expect(page.getByRole('tab', { name: 'Melee Singles' })).toBeVisible();
    await expect(page.getByRole('tab', { name: 'Doubles' })).toBeVisible();

    // All tab shows all 3 participants (including spectator from mock)
    await expect(page.getByText('Mang0').first()).toBeVisible();
    await expect(page.getByText('Spectator')).toBeVisible();
});

test('From tournament tab: switching to event tab shows sort toggle', async ({ page }) => {
    await page.goto('/projects/proj-1/players');
    await page.waitForLoadState('networkidle');
    await page.getByRole('button', { name: 'Add players' }).click();
    await page.getByRole('tab', { name: 'From tournament' }).click();

    await page.getByPlaceholder('genesis-9 or start.gg/tournament/genesis-9').fill('genesis-9');
    await page.getByRole('button', { name: 'Fetch' }).click();

    // Sort toggle is not visible on All tab
    await expect(page.getByRole('button', { name: 'Placement' })).not.toBeVisible();

    // Switch to Melee Singles event tab
    await page.getByRole('tab', { name: 'Melee Singles' }).click();

    // Sort toggle appears
    await expect(page.getByRole('button', { name: 'Placement' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Seed' })).toBeVisible();
});

test('From tournament tab: selections persist across tab switches', async ({ page }) => {
    await page.goto('/projects/proj-1/players');
    await page.waitForLoadState('networkidle');
    await page.getByRole('button', { name: 'Add players' }).click();
    await page.getByRole('tab', { name: 'From tournament' }).click();

    await page.getByPlaceholder('genesis-9 or start.gg/tournament/genesis-9').fill('genesis-9');
    await page.getByRole('button', { name: 'Fetch' }).click();

    // Select Mang0 on the All tab
    await page.getByRole('tab', { name: 'All' }).click();
    const mang0Row = page.locator('[id^="entrant-1001"]');
    await mang0Row.click();

    // Selected count shows 1
    await expect(page.getByText('1 selected')).toBeVisible();

    // Switch to Melee Singles tab — Mang0 should still be checked
    await page.getByRole('tab', { name: 'Melee Singles' }).click();
    await expect(page.getByText('1 selected')).toBeVisible();
});
```

- [ ] **Step 2: Run the new tests**

Run: `cd web && npm run test:e2e -- --grep "From tournament"`

Expected: all 3 new tests pass.

- [ ] **Step 3: Run the full test suite**

Run: `bash test.sh`

Expected: all tests pass (backend + frontend unit + e2e).

- [ ] **Step 4: Commit**

```bash
git add web/tests/projects.test.ts
git commit -m "test(web): add e2e tests for tournament tab all-registrants and per-event tabs"
```
