# Design: Live Crawler Integration Tests

**Date:** 2026-06-27
**Status:** Approved

## Problem

The mirror-backed rankings rewrite removed the only tests that called the real start.gg API (`import_live.rs` and the `live-tests` CI job). The crawler now owns all start.gg communication, but its integration tests use hand-written wiremock stubs with minimal toy data. This leaves two gaps:

- **GraphQL contract drift** — field renames, type coercions, or new required arguments in the real API go undetected until they corrupt the mirror in production.
- **Pipeline integration** — the chain from raw start.gg responses through `global_*` tables through worker import/compute to API stats is never exercised against real data.

## Solution

A new live integration test in `crates/e2e` that:

1. Calls the real start.gg API via `crawler::scraper::run`
2. Runs the worker import and compute pipeline against the populated `global_*` tables
3. Asserts that the API stats and H2H endpoints return sensible results

The test is gated behind a `live-tests` feature flag and skips gracefully when `STARTGG_API_KEY` is not set.

## Pipeline under test

```
real start.gg API
      ↓  crawler::scraper::run  (two targeted 1-day windows)
 global_* tables
      ↓  worker::import::run
project_sets / project_events
      ↓  worker::compute::run
  ranking stats
      ↓  GET /rankings/{id}/stats + head-to-head
  assertions
```

## Golden dataset

**Smash Hannover Weekly series** — the same golden dataset used by the old `import_live.rs`.

| Constant | Value |
|---|---|
| `PLAYER1_SLUG` | `"user/06b4042d"` (King) |
| `PLAYER2_SLUG` | `"user/54b7bbf3"` |
| `MELEE_GAME_ID` | `1` |
| `DATE_OF_WEEKLY_84` | `2025-11-04` |
| `DATE_OF_WEEKLY_88` | `2025-12-02` |

Two tournaments are crawled so there are guaranteed cross-tournament sets for testing H2H logic.

## Crawler configuration

Two sequential `scraper::run` calls, one per event day:

```rust
for event_date in [DATE_OF_WEEKLY_84, DATE_OF_WEEKLY_88] {
    let config = Config {
        startgg_api_key: api_key.clone(),
        from_date: event_date,
        to_date:   event_date,   // single 1-day window
        window_days: 1,
        delay_ms: 250,           // respect rate limits
        sets_per_page: 20,
        game_id: Some(MELEE_GAME_ID),
        startgg_base_url: None,  // real API
        ..
    };
    crawler::scraper::run(&config, &pool, &shutdown).await.unwrap();
}
```

Each run makes approximately: 1 tournament-list call + ~3 calls per tournament (phases, sets, standings). Total: ~10 API calls for both tournaments combined.

## Test structure

**File:** `backend/crates/e2e/tests/crawler_live.rs`

```
#![cfg(feature = "live-tests")]

// 1. Read STARTGG_API_KEY — return early (not fail) if absent
// 2. Run crawler for 2025-11-04 (Weekly #84), then 2025-12-02 (Weekly #88)
// 3. make_app → register → create project → create King + Player2
//    → link accounts → create ranking → add both players to ranking
// 4. worker::import::run → worker::compute::run
// 5. Assert:
//    - ≥2 tournaments appear in the ranking
//    - King has wins + losses > 0
//    - H2H grid contains a King ↔ Player2 entry
```

Assertions are intentionally loose for the initial implementation. A follow-up discovery run (`--nocapture`) can fill in exact hardcoded values (win counts, upset factors, set counts) the same way the old `import_live.rs` golden stats were established.

**HTTP helpers** (`register`, `post_json`, `get_req`, `create_project`, `create_player`, `link_account`, `create_ranking`, `add_player_to_ranking`, `read_json`) are duplicated from `full_flow.rs` into `crawler_live.rs`. No shared module — the helpers are short and self-contained.

## Cargo changes

**`e2e/Cargo.toml`:**
```toml
[features]
live-tests = []

[dev-dependencies]
crawler = { path = "../crawler" }
```

## CI

Re-add a `live-tests` job to `.github/workflows/ci.yml`:

```yaml
live-tests:
  runs-on: ubuntu-latest
  if: ${{ secrets.STARTGG_API_KEY != '' }}
  continue-on-error: true
  services:
    postgres:
      image: postgres:18
      env:
        POSTGRES_PASSWORD: postgres
      options: >-
        --health-cmd pg_isready
        --health-interval 5s
        --health-timeout 5s
        --health-retries 5
      ports:
        - 5432:5432
  env:
    DATABASE_URL: postgres://postgres:postgres@localhost:5432/postgres
    STARTGG_API_KEY: ${{ secrets.STARTGG_API_KEY }}
    SQLX_OFFLINE: "true"
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable
    - uses: Swatinem/rust-cache@v2
      with:
        workspaces: backend
    - name: Run live integration tests
      working-directory: backend
      run: cargo test -p e2e --features live-tests
```

`continue-on-error: true` prevents transient network failures from blocking the main pipeline. The job only runs when the `STARTGG_API_KEY` secret is configured.

## Run command (local)

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres \
STARTGG_API_KEY=<your-key> \
SQLX_OFFLINE=true \
cargo test -p e2e --features live-tests -- --nocapture
```
