# Tournament Import Error Handling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface "tournament not found" as a real error and show a per-event informational note when brackets haven't been published yet in the "Add players from tournament" tab.

**Architecture:** Three layers — `StartggClient` (common crate) distinguishes null-tournament from empty-results; the API handler converts null to a 422; the frontend shows a per-event empty state message based on `state`. No new error variants, no API schema redesign.

**Tech Stack:** Rust (common + api crates), SvelteKit + TypeScript

---

## File Map

| File | Change |
|---|---|
| `backend/crates/common/src/startgg/queries.rs` | Add `state: Option<String>` to `TournamentAllEventNode` and `TournamentEventWithEntrants` |
| `backend/crates/common/src/startgg/operations.rs` | Add `state` to `TOURNAMENT_ALL_EVENTS_QUERY`; thread state through `tournament_events_with_entrants`; change `tournament_participants` return type to `Option<Vec<...>>` |
| `backend/crates/common/src/startgg/mod.rs` | Add test for not-found case; update existing `tournament_participants` tests for new return type |
| `backend/crates/api/src/routes/players.rs` | Add `state` to `TournamentEventResp`; handle `None` from `tournament_participants` |
| `web/src/lib/types.ts` | Add `state: string | null` to `TournamentEventData` |
| `web/src/lib/components/TournamentTab.svelte` | Add per-event empty state based on `state` |

---

## Task 1: Add `state` to StartggClient event types

**Files:**
- Modify: `backend/crates/common/src/startgg/queries.rs`
- Modify: `backend/crates/common/src/startgg/operations.rs`
- Modify: `backend/crates/common/src/startgg/mod.rs` (tests section)

- [ ] **Step 1.1: Add `state` to `TournamentAllEventNode` in queries.rs**

In `backend/crates/common/src/startgg/queries.rs`, find `TournamentAllEventNode` and add the field:

```rust
#[derive(Deserialize)]
pub(crate) struct TournamentAllEventNode {
    pub id: i64,
    pub name: String,
    pub state: Option<String>,
}
```

- [ ] **Step 1.2: Add `state` to `TournamentEventWithEntrants` in queries.rs**

Find `TournamentEventWithEntrants` and add the field:

```rust
#[derive(Debug, Clone)]
pub struct TournamentEventWithEntrants {
    pub id: i64,
    pub name: String,
    pub state: Option<String>,
    pub entrants: Vec<TournamentEntrantOrdered>,
}
```

- [ ] **Step 1.3: Add `state` to `TOURNAMENT_ALL_EVENTS_QUERY` in operations.rs**

Find the const in `backend/crates/common/src/startgg/operations.rs` and update it:

```rust
const TOURNAMENT_ALL_EVENTS_QUERY: &str = r#"
    query($slug: String!) {
        tournament(slug: $slug) {
            events {
                id name state
            }
        }
    }"#;
```

- [ ] **Step 1.4: Thread `state` through `tournament_events_with_entrants` in operations.rs**

In the `tournament_events_with_entrants` method, find the `result.push(...)` call at the end of the outer loop and update it:

```rust
result.push(TournamentEventWithEntrants {
    id: event_node.id,
    name: event_node.name,
    state: event_node.state,
    entrants,
});
```

- [ ] **Step 1.5: Write a failing test for state threading**

In `backend/crates/common/src/startgg/mod.rs`, inside the `tests` module, add after the existing `tournament_events_with_entrants` tests:

```rust
#[tokio::test]
async fn tournament_events_with_entrants_threads_event_state() {
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "tournament": {
                    "events": [
                        { "id": 101, "name": "Melee Singles", "state": "CREATED" },
                        { "id": 102, "name": "Ultimate Singles", "state": "ACTIVE" }
                    ]
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Entrants for event 101 — empty (brackets not published)
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": []
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Entrants for event 102
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
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

    let result = client(&mock.uri())
        .tournament_events_with_entrants("some-weekly")
        .await
        .unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].state.as_deref(), Some("CREATED"));
    assert_eq!(result[0].entrants.len(), 0);
    assert_eq!(result[1].state.as_deref(), Some("ACTIVE"));
    assert_eq!(result[1].entrants.len(), 1);
}
```

- [ ] **Step 1.6: Run the test and confirm it fails**

```bash
cd backend && cargo test -p common -- tournament_events_with_entrants_threads_event_state 2>&1 | tail -20
```

Expected: compile error or test failure (field `state` not yet wired).

- [ ] **Step 1.7: Run the test and confirm it passes now**

(Steps 1.1–1.4 implement everything this test needs.)

```bash
cd backend && cargo test -p common -- tournament_events_with_entrants_threads_event_state 2>&1 | tail -10
```

Expected: `test ... ok`

- [ ] **Step 1.8: Run the full common test suite**

```bash
cd backend && cargo test -p common 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 1.9: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/common/src/startgg/queries.rs \
        backend/crates/common/src/startgg/operations.rs \
        backend/crates/common/src/startgg/mod.rs
git commit -m "feat(common): thread event state through tournament_events_with_entrants"
```

---

## Task 2: Change `tournament_participants` to return `Option`

**Files:**
- Modify: `backend/crates/common/src/startgg/operations.rs`
- Modify: `backend/crates/common/src/startgg/mod.rs` (tests)

- [ ] **Step 2.1: Write a failing test for the not-found case**

In `backend/crates/common/src/startgg/mod.rs`, inside the `tests` module, add after the existing `tournament_participants` tests:

```rust
#[tokio::test]
async fn tournament_participants_returns_none_when_not_found() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(mock_ok(json!({
            "data": { "tournament": null }
        })))
        .mount(&mock)
        .await;

    let result = client(&mock.uri())
        .tournament_participants("nonexistent-slug")
        .await
        .unwrap();
    assert!(result.is_none());
}
```

- [ ] **Step 2.2: Run the test and confirm it fails**

```bash
cd backend && cargo test -p common -- tournament_participants_returns_none_when_not_found 2>&1 | tail -20
```

Expected: compile error (return type mismatch) or test failure.

- [ ] **Step 2.3: Change the return type and implementation in operations.rs**

Find `tournament_participants` in `backend/crates/common/src/startgg/operations.rs` and replace the entire method body:

```rust
#[instrument(skip(self))]
pub async fn tournament_participants(
    &self,
    tournament_handle: &str,
) -> Result<Option<Vec<TournamentParticipant>>, StartggError> {
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

        let Some(tournament) = data.tournament else {
            if page == 1 {
                return Ok(None);
            }
            break;
        };

        let participant_page = match tournament.participants {
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

    tracing::debug!(
        elapsed_ms = t.elapsed().as_millis(),
        "startgg query complete"
    );
    Ok(Some(result))
}
```

- [ ] **Step 2.4: Update the existing `tournament_participants` tests for the new return type**

In `backend/crates/common/src/startgg/mod.rs`, find `tournament_participants_returns_all_with_user` and `tournament_participants_paginates`. Change `.unwrap()` to `.unwrap().expect("expected Some(participants)")` on the final `.await` call in each test:

```rust
// tournament_participants_returns_all_with_user
let result = client(&mock.uri())
    .tournament_participants("some-weekly")
    .await
    .unwrap()
    .expect("expected Some(participants)");
assert_eq!(result.len(), 2);

// tournament_participants_paginates
let result = client(&mock.uri())
    .tournament_participants("some-weekly")
    .await
    .unwrap()
    .expect("expected Some(participants)");
assert_eq!(result.len(), 2);
```

- [ ] **Step 2.5: Run all tournament_participants tests**

```bash
cd backend && cargo test -p common -- tournament_participants 2>&1 | tail -20
```

Expected: all three tests pass (`returns_all_with_user`, `paginates`, `returns_none_when_not_found`).

- [ ] **Step 2.6: Run the full common test suite**

```bash
cd backend && cargo test -p common 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 2.7: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/common/src/startgg/operations.rs \
        backend/crates/common/src/startgg/mod.rs
git commit -m "feat(common): tournament_participants returns None when tournament not found"
```

---

## Task 3: Update the API handler

**Files:**
- Modify: `backend/crates/api/src/routes/players.rs`

- [ ] **Step 3.1: Add `state` to `TournamentEventResp`**

Find `TournamentEventResp` in `backend/crates/api/src/routes/players.rs` and add the field:

```rust
#[derive(Serialize)]
pub struct TournamentEventResp {
    pub id: i64,
    pub name: String,
    pub state: Option<String>,
    pub entrants: Vec<TournamentEntrantOrderedResp>,
}
```

- [ ] **Step 3.2: Handle `None` from `tournament_participants` and thread `state`**

Find `list_tournament_entrants` in the same file. Replace the two `startgg` calls and the response construction:

```rust
let participants = match startgg.tournament_participants(&handle).await? {
    Some(p) => p,
    None => {
        return Err(AppError::UnprocessableEntity(
            "Tournament not found on start.gg".into(),
        ))
    }
};

let events = startgg
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
        state: e.state,
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
```

- [ ] **Step 3.3: Build the api crate to confirm no compile errors**

```bash
cd backend && cargo build -p api 2>&1 | tail -20
```

Expected: `Finished` with no errors.

- [ ] **Step 3.4: Run the api test suite**

```bash
cd backend && cargo test -p common 2>&1 | tail -10
```

(The `list_tournament_entrants` route has no dedicated integration test — compile success and common tests passing is the verification here.)

- [ ] **Step 3.5: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/api/src/routes/players.rs
git commit -m "feat(api): return 422 for unknown tournament slug; thread event state"
```

---

## Task 4: Update frontend types and TournamentTab

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/lib/components/TournamentTab.svelte`

- [ ] **Step 4.1: Add `state` to `TournamentEventData` in types.ts**

Find `TournamentEventData` in `web/src/lib/types.ts` and add the field:

```typescript
export interface TournamentEventData {
  id: number;
  name: string;
  state: string | null;
  entrants: TournamentEntrantOrdered[];
}
```

- [ ] **Step 4.2: Add per-event empty state to TournamentTab.svelte**

In `web/src/lib/components/TournamentTab.svelte`, find the `<ScrollArea>` block and wrap it with a conditional. Replace just the ScrollArea element (leave everything else around it unchanged):

```svelte
{#if activeTab !== "all" && visibleEntrants.length === 0}
  {@const currentEvent = tournamentData.events.find(
    (e) => String(e.id) === activeTab,
  )}
  <p class="py-6 text-center text-sm text-muted-foreground">
    {currentEvent?.state === "CREATED"
      ? "This event's brackets haven't been published yet"
      : "No entrants found for this event"}
  </p>
{:else}
  <ScrollArea class="h-52 rounded-md border">
    <div class="divide-y">
      {#each filteredEntrants as entrant (entrant.startgg_user_id)}
        {@const isAdded = alreadyAddedIds.has(entrant.startgg_user_id)}
        <div
          class="flex items-center gap-3 px-3 py-2 text-sm"
          class:opacity-50={isAdded}
        >
          <Checkbox
            id="entrant-{entrant.startgg_user_id}"
            checked={selected.has(entrant.startgg_user_id)}
            disabled={isAdded}
            onCheckedChange={() =>
              !isAdded && toggleEntrant(entrant.startgg_user_id)}
          />
          {#if activeTab !== "all"}
            <span
              class="w-8 text-right text-xs text-muted-foreground flex-shrink-0"
            >
              {formatRank(entrant)}
            </span>
          {/if}
          <Label
            for="entrant-{entrant.startgg_user_id}"
            class="flex flex-1 items-center gap-2 {isAdded
              ? 'cursor-default'
              : 'cursor-pointer'}"
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
{/if}
```

- [ ] **Step 4.3: Run the frontend unit tests**

```bash
cd web && npm run test:unit 2>&1 | tail -20
```

Expected: all tests pass (no TournamentTab unit tests exist; this confirms no import regressions).

- [ ] **Step 4.4: Run the frontend formatter**

```bash
cd web && npm run format
```

- [ ] **Step 4.5: Commit**

```bash
git add web/src/lib/types.ts \
        web/src/lib/components/TournamentTab.svelte
git commit -m "feat(web): show per-event empty state for unpublished brackets"
```

---

## Task 5: End-to-end verification

- [ ] **Step 5.1: Run the full test suite**

From the repo root:

```bash
bash test.sh 2>&1 | tail -30
```

Expected: PASS for all sections (backend, frontend unit, frontend e2e).

- [ ] **Step 5.2: Manual smoke test — not found**

Start the dev stack (`docker compose up -d`, then `cargo run --bin api` and `cargo run --bin worker` and `cd web && npm run dev`). Open the "Add players" dialog → "From tournament" tab. Enter a nonsense slug like `zzz-does-not-exist`. Click Fetch.

Expected: red error text "Tournament not found on start.gg" appears below the input. No entrant list is shown.

- [ ] **Step 5.3: Manual smoke test — unpublished event**

Enter the slug of a real tournament that has an event in `CREATED` state. Click Fetch.

Expected: The "All" tab shows participants. The event tab(s) with no entrants show "This event's brackets haven't been published yet" instead of a blank scroll area.

- [ ] **Step 5.4: Manual smoke test — normal tournament**

Enter a slug for a completed tournament. Click Fetch.

Expected: existing behaviour unchanged — participants in "All" tab, entrants with seed/placement in event tabs.
