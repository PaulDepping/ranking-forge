# Tournament Import Deduplication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate redundant start.gg API calls when multiple players attended the same tournament by collecting unique tournaments across all players first, then importing each one exactly once.

**Architecture:** Split `import.rs`'s main loop into two phases: Phase 1 iterates all players and accumulates a `HashMap<i64, TournamentNode>` (keyed by `tournament.startgg_id`, first-write wins); Phase 2 iterates that map and calls `import_tournament` for each unique entry. `import_tournament`, `import_event`, `import_entrants`, and `import_sets` are untouched.

**Tech Stack:** Rust, tokio, sqlx, wiremock (tests only)

---

## Files

- Modify: `backend/crates/worker/src/import.rs` — split the per-player loop into two phases
- Modify: `backend/crates/e2e/tests/full_flow.rs` — add `.expect()` call-count assertions to entrants/sets mocks

---

## Task 1: Tighten the e2e test to assert dedup (TDD red)

The existing `full_import_flow` test already has two players sharing one tournament — it's the perfect harness for this assertion. Add exact call-count expectations to the `entrants` and `sets` wiremock stubs so the test fails before the fix.

**Files:**
- Modify: `backend/crates/e2e/tests/full_flow.rs`

- [ ] **Step 1: Add `.expect()` to the three relevant mocks**

In `full_flow.rs`, the `tournaments_by_user`, `event_entrants`, and `event_sets` mocks are each mounted with `.mount(&mock).await`. Chain `.expect(N)` before `.mount()` on each:

```rust
    // tournaments_by_user — called once per linked player (Mango + Armada = 2)
    Mock::given(method("POST"))
        .and(body_string_contains("userId"))
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
                                "startAt": 1700040000_i64
                            }]
                        }]
                    }
                }
            }
        })))
        .expect(2)   // ← once per player
        .mount(&mock)
        .await;

    // event_entrants: must be called exactly once after dedup
    Mock::given(method("POST"))
        .and(body_string_contains("entrants(query:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "total": 2, "totalPages": 1 },
                        "nodes": [
                            {
                                "id": 3001_i64,
                                "initialSeedNum": 2,
                                "isDisqualified": false,
                                "standing": { "placement": 2 },
                                "participants": [{ "gamerTag": "Mango", "user": { "id": 12345_i64 } }]
                            },
                            {
                                "id": 3002_i64,
                                "initialSeedNum": 7,
                                "isDisqualified": false,
                                "standing": { "placement": 1 },
                                "participants": [{ "gamerTag": "Armada", "user": { "id": 67890_i64 } }]
                            }
                        ]
                    }
                }
            }
        })))
        .expect(1)   // ← dedup: only once regardless of player count
        .mount(&mock)
        .await;

    // event_sets: must be called exactly once after dedup
    Mock::given(method("POST"))
        .and(body_string_contains("sets(page:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "sets": {
                        "pageInfo": { "total": 1, "totalPages": 1 },
                        "nodes": [{
                            "id": 4001_i64,
                            "winnerId": 3002_i64,
                            "round": 1,
                            "fullRoundText": "Round 1",
                            "totalGames": 5,
                            "completedAt": 1700050000_i64,
                            "vodUrl": null,
                            "slots": [
                                {
                                    "entrant": { "id": 3002_i64 },
                                    "standing": { "stats": { "score": { "value": 3.0 } } }
                                },
                                {
                                    "entrant": { "id": 3001_i64 },
                                    "standing": { "stats": { "score": { "value": 1.0 } } }
                                }
                            ]
                        }]
                    }
                }
            }
        })))
        .expect(1)   // ← dedup: only once regardless of player count
        .mount(&mock)
        .await;
```

Replace the three corresponding mock blocks in `full_import_flow` (lines 133–233 in the current file) with the code above. The only changes are adding `.expect(2)` to the `userId` mock and `.expect(1)` to the `entrants` and `sets` mocks.

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd backend && DATABASE_URL=postgres://doesnt_matter cargo test -p e2e -- full_import_flow 2>&1 | tail -20
```

Expected: the test fails because wiremock sees `event_entrants` and `event_sets` each called 2× (once per player) but expects 1×. The failure message will mention "Expected mock to be called 1 time(s), but it was called 2 time(s)."

- [ ] **Step 3: Commit the failing test**

```bash
git add backend/crates/e2e/tests/full_flow.rs
git commit -m "test(e2e): assert entrants+sets fetched once when players share tournament"
```

---

## Task 2: Implement two-phase collect-then-import

**Files:**
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Replace `import_user_tournaments` with `collect_user_tournaments`**

The new function drops `pool`, `project_id`, `game_name`, and `account_map` from its signature (it no longer imports anything) and accumulates into a caller-owned `&mut HashMap<i64, TournamentNode>`.

Replace the entire `import_user_tournaments` function (lines 77–136) with:

```rust
#[instrument(skip(startgg, seen), fields(startgg_user_id = user_id, game_id))]
async fn collect_user_tournaments(
    startgg: &StartggClient,
    user_id: i64,
    game_id: i64,
    after_date: Option<i64>,
    before_date: Option<i64>,
    seen: &mut HashMap<i64, TournamentNode>,
) -> anyhow::Result<()> {
    let mut page = 1i32;
    let mut collected = 0usize;
    'pages: loop {
        let tournament_page = startgg
            .tournaments_by_user(user_id, game_id, page, 25)
            .await?;

        for tournament in tournament_page.nodes {
            let start_ts = tournament.start_at.unwrap_or(0);
            if let Some(before) = before_date {
                if start_ts > before {
                    continue;
                }
            }
            if let Some(after) = after_date {
                if start_ts < after {
                    break 'pages;
                }
            }
            seen.entry(tournament.id).or_insert_with(|| {
                collected += 1;
                tournament
            });
        }

        let total_pages = tournament_page
            .page_info
            .as_ref()
            .and_then(|p| p.total_pages)
            .unwrap_or(1);
        if page >= total_pages {
            break;
        }
        page += 1;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    tracing::info!(collected, "user tournaments collected");
    Ok(())
}
```

Note: `tournament_page.nodes` is now consumed by value (`.nodes` → iterating owned `Vec`) so the loop variable is `tournament: TournamentNode` rather than `&TournamentNode`. This avoids cloning when inserting into the map.

- [ ] **Step 2: Update `run` to use the two-phase approach**

Replace the `for user_id in user_ids` loop inside `run` (lines 59–74) with:

```rust
    tracing::info!(player_count = user_ids.len(), "starting import");

    // Phase 1: discover all unique tournaments across all players
    let mut seen: HashMap<i64, TournamentNode> = HashMap::new();
    for user_id in user_ids {
        collect_user_tournaments(
            startgg,
            user_id,
            game_id,
            params.after_date,
            params.before_date,
            &mut seen,
        )
        .await?;
    }
    tracing::info!(unique_tournament_count = seen.len(), "collection complete, starting import");

    // Phase 2: import each unique tournament exactly once
    for (_, tournament) in &seen {
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
    }
```

- [ ] **Step 3: Verify the file compiles**

```bash
cd backend && cargo build -p worker 2>&1
```

Expected: compiles with no errors. Fix any type errors before proceeding (the most likely issue is if `TournamentNode` didn't derive `Clone` for the `or_insert_with` closure — check `common/src/startgg/queries.rs`; `TournamentNode` already derives `Clone` so this should be fine).

- [ ] **Step 4: Run the e2e test to confirm it passes**

```bash
cd backend && DATABASE_URL=postgres://doesnt_matter cargo test -p e2e -- full_import_flow 2>&1 | tail -20
```

Expected: `test full_import_flow ... ok`

- [ ] **Step 5: Run the full backend test suite**

```bash
bash backend/test.sh 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/worker/src/import.rs
git commit -m "feat(worker): deduplicate tournament imports across players

Previously each player triggered a full re-import of every shared
tournament. Now tournaments are collected into a HashMap keyed by
startgg_id (first occurrence wins) before any importing begins, so
event_entrants and event_sets are fetched exactly once per tournament
regardless of how many project players attended."
```
