# Live Test Stats and H2H Assertions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `import_hannover_weekly_88_and_84` to filter imported events down to two known Melee Singles events and assert exact per-player stats, H2H summary counts, and H2H set details against real start.gg data.

**Architecture:** Two-commit implementation. First commit adds a temporary `discover_hannover_stats` test that prints all event IDs and full stats/H2H payloads so golden values can be read off. Second commit hardcodes those values as constants, replaces the loose `total_sets > 0` check with precise filtering and assertions, and removes the discovery function.

**Tech Stack:** Rust, sqlx, axum test utilities (Router::oneshot), serde_json, start.gg API (live)

---

## File Map

| File | Change |
|------|--------|
| `backend/crates/e2e/tests/import_live.rs` | Add `patch_json` helper; add `discover_hannover_stats`; add constants; add filtering + precise assertions to `import_hannover_weekly_88_and_84`; delete `discover_hannover_stats` |

---

### Task 1: Add the discovery test

**Files:**
- Modify: `backend/crates/e2e/tests/import_live.rs`

- [ ] **Step 1: Add `patch_json` helper after `get_req`**

In `backend/crates/e2e/tests/import_live.rs`, add this function immediately after the `get_req` function (around line 97):

```rust
async fn patch_json(app: &Router, uri: &str, cookie: &str, body: Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(uri)
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}
```

- [ ] **Step 2: Add `discover_hannover_stats` at the end of the file**

Append this function after the closing `}` of `import_hannover_weekly_88_and_84`:

```rust
/// Temporary discovery test — run once with --nocapture to read off golden
/// event IDs and stats values, then delete this function after Task 4.
///
/// Run with:
///   DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
///   STARTGG_API_KEY=<your-key> \
///   SQLX_OFFLINE=true \
///   cargo test -p e2e --features live-tests -- discover_hannover_stats --nocapture
#[sqlx::test(migrations = "../../migrations")]
async fn discover_hannover_stats(pool: PgPool) {
    let api_key = live_api_key();
    let startgg_client = StartggClient::new(api_key.clone());
    let app = make_app(pool.clone(), "https://api.start.gg/gql/alpha");

    let cookie = register(&app, "discoveruser", "pass1234").await;
    set_startgg_api_key(&pool, &cookie, &api_key).await;

    // Create a Melee project
    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({
            "name": "Discover H2H Stats",
            "game_id": 1,
            "game_name": "Super Smash Bros. Melee"
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project = read_json(resp).await;
    let project_id = project["id"].as_str().unwrap().to_string();

    // Add Player 1 (King)
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "King"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let king_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{king_id}/accounts"),
        &cookie,
        json!({"handle": PLAYER1_SLUG}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Add Player 2
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Player2"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let player2_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{player2_id}/accounts"),
        &cookie,
        json!({"handle": PLAYER2_SLUG}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Run import — same date window as import_hannover_weekly_88_and_84
    let project_uuid = Uuid::parse_str(&project_id).unwrap();
    worker::import::run(
        &pool,
        &startgg_client,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams {
            after_date: Some(1761582600),  // 2025-10-27
            before_date: Some(1765384200), // 2025-12-10
        },
    )
    .await
    .unwrap();

    // ── Print all events (to identify KEEP_EVENT_STARTGG_ID_* values) ─────────
    let resp = get_req(&app, &format!("/projects/{project_id}/tournaments"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tournaments = read_json(resp).await;
    eprintln!("\n=== TOURNAMENTS & EVENTS ===");
    for t in tournaments.as_array().unwrap() {
        eprintln!(
            "Tournament: {}  (handle: {})",
            t["name"].as_str().unwrap_or("?"),
            t["handle"].as_str().unwrap_or("?")
        );
        for e in t["events"].as_array().unwrap() {
            eprintln!(
                "  event  startgg_id={}  name={:?}  included={}",
                e["startgg_id"],
                e["name"].as_str().unwrap_or("?"),
                e["included"]
            );
        }
    }

    // ── Print full stats ───────────────────────────────────────────────────────
    let resp = get_req(&app, &format!("/projects/{project_id}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    eprintln!("\n=== STATS ===\n{}", serde_json::to_string_pretty(&stats).unwrap());

    // ── Print H2H summary ──────────────────────────────────────────────────────
    let resp = get_req(&app, &format!("/projects/{project_id}/head-to-head"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h2h = read_json(resp).await;
    eprintln!(
        "\n=== H2H SUMMARY ===\n{}",
        serde_json::to_string_pretty(&h2h).unwrap()
    );

    // ── Print H2H sets drilldown ───────────────────────────────────────────────
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/head-to-head/{king_id}/{player2_id}/sets"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sets = read_json(resp).await;
    eprintln!(
        "\n=== H2H SETS ({king_id} vs {player2_id}) ===\n{}",
        serde_json::to_string_pretty(&sets).unwrap()
    );

    panic!(
        "discovery complete — review output above, fill in golden constants, \
         extend import_hannover_weekly_88_and_84, then delete this function"
    );
}
```

- [ ] **Step 3: Verify it compiles**

```bash
cd backend && SQLX_OFFLINE=true cargo test -p e2e --no-run 2>&1 | tail -10
```

Expected: `Finished` with no errors. The test won't run yet.

- [ ] **Step 4: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/e2e/tests/import_live.rs
git commit -m "test(e2e): add discover_hannover_stats to collect golden stats/H2H values"
```

---

### Task 2: Run the discovery test and record golden values

**Files:** (no code changes — this is a run-and-record step)

- [ ] **Step 1: Start a local Postgres + API + worker if needed**

The `sqlx::test` macro spins up its own isolated schema, so no separate DB is required. The test calls the real start.gg API directly via `worker::import::run`, so only `STARTGG_API_KEY` is needed.

- [ ] **Step 2: Run the discovery test**

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
STARTGG_API_KEY=<your-key> \
SQLX_OFFLINE=true \
cargo test -p e2e --features live-tests -- discover_hannover_stats --nocapture 2>&1
```

Expected: the test "fails" with the panic message, but the output above the panic contains the four `===` blocks.

- [ ] **Step 3: Record the keep-list event IDs**

In the `=== TOURNAMENTS & EVENTS ===` block, find the two lines for "Melee Singles" (or the equivalent main bracket event) — one under "Smash Hannover Weekly #88" and one under "Smash Hannover Weekly #84". Note the `startgg_id` value on each line. These become `KEEP_EVENT_STARTGG_ID_88` and `KEEP_EVENT_STARTGG_ID_84`.

Example (your values will differ):
```
Tournament: Smash Hannover Weekly #88  (handle: smash-hannover-weekly-88)
  event  startgg_id=654321  name="Melee Singles"  included=true
Tournament: Smash Hannover Weekly #84  (handle: smash-hannover-weekly-84)
  event  startgg_id=654789  name="Melee Singles"  included=true
```

- [ ] **Step 4: Record per-player stats**

In the `=== STATS ===` block, find the entry for King (`player_id` matching the UUID printed earlier). Note the full shape of `wins` and `losses` arrays. For each entry record: `opponent_name`, `upset_factor`, `tournament_name`, `event_name`, `round_name`, `winner_seed`, `loser_seed`, `is_dq`, `startgg_set_id`.

Note: these are stats across *all* imported events (before filtering). The values may include sets from events other than the target two. That's fine — in Task 3 the filtering step will reduce the stats to only the target events, so the actual assertion values will be a subset of what you see here. Re-run the full suite after filtering in Task 3 to see the filtered values; or note only the entries whose `event_name` matches the Melee Singles events you identified in Step 3.

- [ ] **Step 5: Record H2H summary counts**

In the `=== H2H SUMMARY ===` block, find the entry where `player_id` is King's UUID and `opponent_id` is Player2's UUID. Note `wins` and `losses`. These are the values you will assert, but again only after filtering in Task 3 reduces them to the target events.

- [ ] **Step 6: Record H2H sets**

In the `=== H2H SETS ===` block, note the array length and all fields of each entry: `is_win`, `tournament_name`, `event_name`, `round_name`, `opponent_name`. These are the values to assert in Task 4.

---

### Task 3: Add constants and event filtering

**Files:**
- Modify: `backend/crates/e2e/tests/import_live.rs`

- [ ] **Step 1: Add the keep-list constants near the top of the file**

In `backend/crates/e2e/tests/import_live.rs`, add after the last existing constant (`WEEKLY_84_HANDLE`):

```rust
// ── Event filtering constants (from discover_hannover_stats Task 2 Step 3) ────
const KEEP_EVENT_STARTGG_ID_88: i64 = 0; // replace with startgg_id of "Melee Singles" at #88
const KEEP_EVENT_STARTGG_ID_84: i64 = 0; // replace with startgg_id of "Melee Singles" at #84
```

Replace each `0` with the values recorded in Task 2 Step 3.

- [ ] **Step 2: Replace `total_sets > 0` with the event filtering loop**

In `import_hannover_weekly_88_and_84`, remove the entire `total_sets > 0` block:

```rust
    // Assert: total set count > 0 across all player stats
    let resp = get_req(&app, &format!("/projects/{project_id}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    let stats_arr = stats.as_array().unwrap();

    let total_sets: usize = stats_arr
        .iter()
        .map(|s| {
            s["wins"].as_array().map(|a| a.len()).unwrap_or(0)
                + s["losses"].as_array().map(|a| a.len()).unwrap_or(0)
        })
        .sum();

    assert!(
        total_sets > 0,
        "Expected total set count > 0 after importing both players, got 0"
    );
```

Add in its place (the `tournaments` variable is already bound above, reuse it):

```rust
    // ── Filter: exclude all events except the two target Melee Singles events ──
    let keep_ids = [KEEP_EVENT_STARTGG_ID_88, KEEP_EVENT_STARTGG_ID_84];
    for tournament in tournaments.as_array().unwrap() {
        for event in tournament["events"].as_array().unwrap() {
            let startgg_id = event["startgg_id"].as_i64().unwrap_or(0);
            if !keep_ids.contains(&startgg_id) {
                let event_uuid = event["id"].as_str().unwrap();
                let resp = patch_json(
                    &app,
                    &format!("/projects/{project_id}/events/{event_uuid}"),
                    &cookie,
                    json!({"included": false}),
                )
                .await;
                assert!(
                    resp.status().is_success(),
                    "PATCH event/{event_uuid} returned {}",
                    resp.status()
                );
            }
        }
    }
```

- [ ] **Step 3: Verify it compiles**

```bash
cd backend && SQLX_OFFLINE=true cargo test -p e2e --no-run 2>&1 | tail -10
```

Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/e2e/tests/import_live.rs
git commit -m "test(e2e): add event filtering to import_hannover_weekly_88_and_84"
```

---

### Task 4: Add stats and H2H assertions

**Files:**
- Modify: `backend/crates/e2e/tests/import_live.rs`

Each assertion below shows `0` or `""` as the initial value. Replace every one with the value recorded in Task 2 (Steps 4–6) before running. The per-entry win/loss blocks are shown for a count of 1 each — if the discovery output shows 0 entries for a player's wins or losses, delete that block; if it shows 2 or more, copy the block and change `[0]` to `[1]`, etc.

- [ ] **Step 1: Add the stats assertions block**

Immediately after the filtering loop added in Task 3, add:

```rust
    // ── Stats ──────────────────────────────────────────────────────────────────
    let resp = get_req(&app, &format!("/projects/{project_id}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    let stats_arr = stats.as_array().unwrap();

    let king_stats = stats_arr
        .iter()
        .find(|s| s["player_id"] == json!(player1_id))
        .expect("King not found in stats");
    let player2_stats = stats_arr
        .iter()
        .find(|s| s["player_id"] == json!(player2_id))
        .expect("Player2 not found in stats");

    // Replace each 0 with the count from Task 2 Step 4 (filtered to the two target events)
    assert_eq!(king_stats["wins"].as_array().unwrap().len(),     0_usize, "king wins count");
    assert_eq!(king_stats["losses"].as_array().unwrap().len(),   0_usize, "king losses count");
    assert_eq!(player2_stats["wins"].as_array().unwrap().len(),  0_usize, "player2 wins count");
    assert_eq!(player2_stats["losses"].as_array().unwrap().len(),0_usize, "player2 losses count");

    // King's wins[0] — delete this block if king wins count is 0;
    // copy with [1], [2], … if count > 1.
    {
        let w = &king_stats["wins"][0];
        assert_eq!(w["opponent_name"],   json!("Player2"), "king win[0] opponent");
        assert_eq!(w["upset_factor"],    json!(0_i64),     "king win[0] upset_factor");    // Task 2 Step 4
        assert_eq!(w["tournament_name"], json!(""),        "king win[0] tournament_name"); // Task 2 Step 4
        assert_eq!(w["event_name"],      json!(""),        "king win[0] event_name");      // Task 2 Step 4
        assert_eq!(w["round_name"],      json!(""),        "king win[0] round_name");      // Task 2 Step 4
        assert_eq!(w["winner_seed"],     json!(0_i64),     "king win[0] winner_seed");     // Task 2 Step 4
        assert_eq!(w["loser_seed"],      json!(0_i64),     "king win[0] loser_seed");      // Task 2 Step 4
        assert_eq!(w["is_dq"],           json!(false),     "king win[0] is_dq");           // Task 2 Step 4
        assert_eq!(w["startgg_set_id"],  json!(0_i64),     "king win[0] startgg_set_id");  // Task 2 Step 4
    }

    // King's losses[0] — delete if king losses count is 0; copy for [1], [2], …
    {
        let l = &king_stats["losses"][0];
        assert_eq!(l["opponent_name"],   json!("Player2"), "king loss[0] opponent");
        assert_eq!(l["upset_factor"],    json!(0_i64),     "king loss[0] upset_factor");
        assert_eq!(l["tournament_name"], json!(""),        "king loss[0] tournament_name");
        assert_eq!(l["event_name"],      json!(""),        "king loss[0] event_name");
        assert_eq!(l["round_name"],      json!(""),        "king loss[0] round_name");
        assert_eq!(l["winner_seed"],     json!(0_i64),     "king loss[0] winner_seed");
        assert_eq!(l["loser_seed"],      json!(0_i64),     "king loss[0] loser_seed");
        assert_eq!(l["is_dq"],           json!(false),     "king loss[0] is_dq");
        assert_eq!(l["startgg_set_id"],  json!(0_i64),     "king loss[0] startgg_set_id");
    }

    // Player2's wins[0] — delete if player2 wins count is 0; copy for [1], [2], …
    {
        let w = &player2_stats["wins"][0];
        assert_eq!(w["opponent_name"],   json!("King"),    "player2 win[0] opponent");
        assert_eq!(w["upset_factor"],    json!(0_i64),     "player2 win[0] upset_factor");
        assert_eq!(w["tournament_name"], json!(""),        "player2 win[0] tournament_name");
        assert_eq!(w["event_name"],      json!(""),        "player2 win[0] event_name");
        assert_eq!(w["round_name"],      json!(""),        "player2 win[0] round_name");
        assert_eq!(w["winner_seed"],     json!(0_i64),     "player2 win[0] winner_seed");
        assert_eq!(w["loser_seed"],      json!(0_i64),     "player2 win[0] loser_seed");
        assert_eq!(w["is_dq"],           json!(false),     "player2 win[0] is_dq");
        assert_eq!(w["startgg_set_id"],  json!(0_i64),     "player2 win[0] startgg_set_id");
    }

    // Player2's losses[0] — delete if player2 losses count is 0; copy for [1], [2], …
    {
        let l = &player2_stats["losses"][0];
        assert_eq!(l["opponent_name"],   json!("King"),    "player2 loss[0] opponent");
        assert_eq!(l["upset_factor"],    json!(0_i64),     "player2 loss[0] upset_factor");
        assert_eq!(l["tournament_name"], json!(""),        "player2 loss[0] tournament_name");
        assert_eq!(l["event_name"],      json!(""),        "player2 loss[0] event_name");
        assert_eq!(l["round_name"],      json!(""),        "player2 loss[0] round_name");
        assert_eq!(l["winner_seed"],     json!(0_i64),     "player2 loss[0] winner_seed");
        assert_eq!(l["loser_seed"],      json!(0_i64),     "player2 loss[0] loser_seed");
        assert_eq!(l["is_dq"],           json!(false),     "player2 loss[0] is_dq");
        assert_eq!(l["startgg_set_id"],  json!(0_i64),     "player2 loss[0] startgg_set_id");
    }
```

- [ ] **Step 2: Add the H2H summary assertions block**

Immediately after the stats block:

```rust
    // ── H2H summary ───────────────────────────────────────────────────────────
    let resp = get_req(&app, &format!("/projects/{project_id}/head-to-head"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h2h = read_json(resp).await;
    let h2h_arr = h2h.as_array().unwrap();

    let king_vs_p2 = h2h_arr
        .iter()
        .find(|e| e["player_id"] == json!(player1_id) && e["opponent_id"] == json!(player2_id))
        .expect("king→player2 entry missing from H2H summary");
    let p2_vs_king = h2h_arr
        .iter()
        .find(|e| e["player_id"] == json!(player2_id) && e["opponent_id"] == json!(player1_id))
        .expect("player2→king entry missing from H2H summary");

    // Replace each 0 with the value from Task 2 Step 5
    assert_eq!(king_vs_p2["wins"],   json!(0_i64), "king→p2 wins");
    assert_eq!(king_vs_p2["losses"], json!(0_i64), "king→p2 losses");
    assert_eq!(p2_vs_king["wins"],   json!(0_i64), "p2→king wins");
    assert_eq!(p2_vs_king["losses"], json!(0_i64), "p2→king losses");
```

- [ ] **Step 3: Add the H2H sets drilldown assertions block**

Immediately after the H2H summary block:

```rust
    // ── H2H sets drilldown ────────────────────────────────────────────────────
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/head-to-head/{player1_id}/{player2_id}/sets"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sets_body = read_json(resp).await;
    let sets_arr = sets_body.as_array().unwrap();

    // Replace 0 with the array length from Task 2 Step 6
    assert_eq!(sets_arr.len(), 0_usize, "H2H sets count");

    // sets_arr[0] spot-check — delete if count is 0; copy for [1], [2], …
    // Replace each value from Task 2 Step 6
    assert_eq!(sets_arr[0]["is_win"],          json!(false), "sets[0] is_win");         // Task 2 Step 6
    assert_eq!(sets_arr[0]["tournament_name"], json!(""),    "sets[0] tournament_name"); // Task 2 Step 6
    assert_eq!(sets_arr[0]["event_name"],      json!(""),    "sets[0] event_name");      // Task 2 Step 6
    assert_eq!(sets_arr[0]["round_name"],      json!(""),    "sets[0] round_name");      // Task 2 Step 6
    assert_eq!(sets_arr[0]["opponent_name"],   json!(""),    "sets[0] opponent_name");   // Task 2 Step 6
```

- [ ] **Step 4: Verify it compiles**

```bash
cd backend && SQLX_OFFLINE=true cargo test -p e2e --no-run 2>&1 | tail -10
```

Expected: `Finished` with no errors. The `0` / `""` placeholder values compile fine; they will fail at runtime until replaced with real values from Task 2.

- [ ] **Step 5: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/e2e/tests/import_live.rs
git commit -m "test(e2e): add stats and H2H assertions to import_hannover_weekly_88_and_84"
```

---

### Task 5: Delete the discovery test and verify the live suite

**Files:**
- Modify: `backend/crates/e2e/tests/import_live.rs`

- [ ] **Step 1: Delete `discover_hannover_stats`**

Remove the entire `discover_hannover_stats` function from `import_live.rs` — everything from its doc comment (`/// Temporary discovery test`) through its closing `}`.

- [ ] **Step 2: Verify it compiles**

```bash
cd backend && SQLX_OFFLINE=true cargo test -p e2e --no-run 2>&1 | tail -10
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Run the full live test suite**

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
STARTGG_API_KEY=<your-key> \
SQLX_OFFLINE=true \
cargo test -p e2e --features live-tests 2>&1 | tail -20
```

Expected: three tests run and pass — `import_hannover_weekly_100`, `import_hannover_weekly_88_and_84`. No `discover_hannover_stats` in the output.

- [ ] **Step 4: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/e2e/tests/import_live.rs
git commit -m "test(e2e): remove discover_hannover_stats scaffold, live golden stats assertions complete"
```
