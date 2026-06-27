# Live Crawler Integration Tests — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `live-tests` feature to `crates/e2e` containing one test that calls the real start.gg API via the crawler, runs the full worker import/compute pipeline, and asserts that stats and H2H endpoints return real results.

**Architecture:** `crawler::scraper::run` is called twice (once per event day) against the real start.gg API, writing into `global_*` tables. The existing `worker::import::run` + `worker::compute::run` pipeline then reads those tables. The Axum router is exercised via `tower::ServiceExt::oneshot`, identical to the existing `full_flow.rs` pattern.

**Tech Stack:** Rust, sqlx `#[sqlx::test]`, axum test-client pattern (tower oneshot), `crawler` crate, `worker` crate.

## Global Constraints

- All backend commands run from `backend/`
- Use `SQLX_OFFLINE=true` for all `cargo test` invocations — never run without it
- Use `cargo add` for any new dependencies — never edit version numbers in `Cargo.toml` by hand
- After any `sqlx::query!` change, run `bash backend/prepare-sqlx.sh`
- No new `sqlx::query!` macros are introduced in this plan — no sqlx prepare step needed

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Create | `backend/crates/e2e/tests/crawler_live.rs` | Live test: crawler → import → compute → assertions |
| Modify | `backend/crates/e2e/Cargo.toml` | Add `live-tests` feature + `crawler` dev-dep |
| Modify | `.github/workflows/ci.yml` | Re-add `live-tests` CI job |

---

## Task 1: Wire the `live-tests` feature and `crawler` dependency

**Files:**
- Modify: `backend/crates/e2e/Cargo.toml`

**Interfaces:**
- Produces: `live-tests` feature flag; `crawler` crate available as a dev-dep in `e2e`

- [ ] **Step 1: Add the feature flag and dev-dep**

Open `backend/crates/e2e/Cargo.toml`. Add `crawler` to `[dev-dependencies]` and add a `[features]` table:

```toml
[package]
name = "e2e"
version = "0.2.0"
edition = "2024"

[features]
live-tests = []

[dev-dependencies]
api    = { path = "../api" }
worker = { path = "../worker" }
crawler = { path = "../crawler" }
axum           = { version = "0.8.9" }
http-body-util = "0.1"
serde_json     = "1.0.149"
sqlx = { version = "0.8.6", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "macros", "migrate"] }
tokio = { version = "1.52.3", features = ["full"] }
tower = { version = "0.5", features = ["util"] }
uuid  = { version = "1.23.1", features = ["v4", "serde"] }
common = { path = "../common" }
```

- [ ] **Step 2: Verify it compiles**

```bash
cd backend && SQLX_OFFLINE=true cargo check -p e2e --features live-tests
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add backend/crates/e2e/Cargo.toml backend/Cargo.lock
git commit -m "feat(e2e): add live-tests feature flag and crawler dev-dep"
```

---

## Task 2: Write the live crawler integration test

**Files:**
- Create: `backend/crates/e2e/tests/crawler_live.rs`

**Interfaces:**
- Consumes:
  - `crawler::scraper::run(config: &crawler::cli::Config, pool: &PgPool, shutdown: &AtomicBool) -> anyhow::Result<()>`
  - `crawler::cli::Config` — all fields (see `backend/crates/crawler/src/cli.rs`)
  - `worker::import::run(pool: &PgPool, project_id: Uuid, job_id: Uuid, params: common::jobs::ImportParams) -> anyhow::Result<()>`
  - `worker::compute::run(pool: &PgPool, ranking_id: Uuid) -> anyhow::Result<()>`
  - `api::state::AppState { db: PgPool, cors_origin: String }`
  - `api::routes::router() -> Router`
- Produces: nothing (terminal test file)

- [ ] **Step 1: Create the test file**

Create `backend/crates/e2e/tests/crawler_live.rs` with the full contents below:

```rust
#![cfg(feature = "live-tests")]

// Live integration test: real start.gg API → crawler → global_* tables →
// worker import/compute → API stats/H2H assertions.
//
// Requires STARTGG_API_KEY in environment. Skips gracefully when absent.
//
// Run:
//   DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
//   STARTGG_API_KEY=<your-key> \
//   SQLX_OFFLINE=true \
//   cargo test -p e2e --features live-tests -- --nocapture

use std::sync::atomic::AtomicBool;

use api::{routes, state::AppState};
use axum::{Router, body::Body, http::Request, http::StatusCode};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// ── Golden dataset ────────────────────────────────────────────────────────────

const PLAYER1_SLUG: &str = "user/06b4042d"; // King
const PLAYER2_SLUG: &str = "user/54b7bbf3";

const MELEE_GAME_ID: u64 = 1;

// Smash Hannover Weekly #84 — 2025-11-04
// Smash Hannover Weekly #88 — 2025-12-02
const EVENT_DATES: [(i32, u32, u32); 2] = [(2025, 11, 4), (2025, 12, 2)];

// ── App factory ───────────────────────────────────────────────────────────────

fn make_app(pool: PgPool) -> Router {
    let state = AppState {
        db: pool,
        cors_origin: "http://localhost".to_string(),
    };
    routes::router().with_state(state)
}

// ── HTTP helpers ──────────────────────────────────────────────────────────────

async fn read_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn register(app: &Router, username: &str, password: &str) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "email": format!("{username}@test.com"),
                        "display_name": username,
                        "password": password
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = read_json(resp).await;
    format!("session_id={}", body["session_id"].as_str().unwrap())
}

async fn post_json(app: &Router, uri: &str, cookie: &str, body: Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_req(app: &Router, uri: &str, cookie: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

// ── Domain helpers ────────────────────────────────────────────────────────────

async fn create_project(app: &Router, cookie: &str, name: &str) -> String {
    let resp = post_json(app, "/projects", cookie, json!({"name": name})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    read_json(resp).await["id"].as_str().unwrap().to_string()
}

async fn create_player(app: &Router, cookie: &str, project_id: &str, name: &str) -> String {
    let resp = post_json(
        app,
        &format!("/projects/{project_id}/players"),
        cookie,
        json!({"name": name}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    read_json(resp).await["id"].as_str().unwrap().to_string()
}

async fn link_account(app: &Router, cookie: &str, project_id: &str, player_id: &str, handle: &str) {
    let resp = post_json(
        app,
        &format!("/projects/{project_id}/players/{player_id}/accounts"),
        cookie,
        json!({"handle": handle}),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "link_account failed for {handle}"
    );
}

async fn create_ranking(app: &Router, cookie: &str, project_id: &str, name: &str) -> String {
    let resp = post_json(
        app,
        &format!("/projects/{project_id}/rankings"),
        cookie,
        json!({"name": name}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    read_json(resp).await["id"].as_str().unwrap().to_string()
}

async fn add_player_to_ranking(
    app: &Router,
    cookie: &str,
    project_id: &str,
    ranking_id: &str,
    player_id: &str,
) {
    let resp = post_json(
        app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/players"),
        cookie,
        json!({"player_id": player_id}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
}

// ── Test ──────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn crawl_and_import_hannover_weekly(pool: PgPool) {
    let api_key = match std::env::var("STARTGG_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("STARTGG_API_KEY not set — skipping live test");
            return;
        }
    };

    // ── Step 1: Crawl real start.gg for two specific event days ──────────────

    let shutdown = AtomicBool::new(false);

    for (year, month, day) in EVENT_DATES {
        let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
        let config = crawler::cli::Config {
            database_url: "unused".into(),
            startgg_api_key: api_key.clone(),
            from_date: date,
            to_date: date,
            window_days: 1,
            delay_ms: 250,
            sets_per_page: 20,
            game_id: Some(MELEE_GAME_ID),
            rust_log: "off".into(),
            startgg_base_url: None,
        };
        crawler::scraper::run(&config, &pool, &shutdown)
            .await
            .unwrap_or_else(|e| panic!("crawler failed for {year}-{month:02}-{day:02}: {e}"));
    }

    // ── Step 2: Project setup ─────────────────────────────────────────────────

    let app = make_app(pool.clone());
    let cookie = register(&app, "user1", "pass1234").await;
    let project_id = create_project(&app, &cookie, "Hannover Weeklies").await;

    let king_pid = create_player(&app, &cookie, &project_id, "King").await;
    let player2_pid = create_player(&app, &cookie, &project_id, "Player2").await;

    link_account(&app, &cookie, &project_id, &king_pid, PLAYER1_SLUG).await;
    link_account(&app, &cookie, &project_id, &player2_pid, PLAYER2_SLUG).await;

    let ranking_id = create_ranking(&app, &cookie, &project_id, "Main").await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &king_pid).await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &player2_pid).await;

    // ── Step 3: Import + compute ──────────────────────────────────────────────

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/import"),
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let job_id: Uuid = read_json(resp).await["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let project_id_uuid: Uuid = project_id.parse().unwrap();
    let ranking_id_uuid: Uuid = ranking_id.parse().unwrap();

    worker::import::run(&pool, project_id_uuid, job_id, Default::default())
        .await
        .unwrap();
    worker::compute::run(&pool, ranking_id_uuid).await.unwrap();

    // ── Step 4: Assertions ────────────────────────────────────────────────────

    // At least two tournaments appear in the ranking (one per crawled event day).
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tournaments = read_json(resp).await;
    let tournament_list = tournaments.as_array().unwrap();
    assert!(
        tournament_list.len() >= 2,
        "expected ≥2 tournaments, got {}: {tournaments}",
        tournament_list.len()
    );

    // King has at least one set result (wins or losses).
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/stats"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    let king_stats = stats
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["player_id"] == king_pid)
        .unwrap_or_else(|| panic!("King not found in stats: {stats}"));
    let total_sets = king_stats["wins"].as_array().unwrap().len()
        + king_stats["losses"].as_array().unwrap().len();
    assert!(
        total_sets > 0,
        "King should have at least one set result, got stats: {king_stats}"
    );

    // H2H grid is non-empty.
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/head-to-head"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h2h = read_json(resp).await;
    assert!(
        !h2h.as_array().unwrap().is_empty(),
        "expected non-empty H2H grid: {h2h}"
    );
}
```

- [ ] **Step 2: Run the compile check (no API key needed yet)**

```bash
cd backend && SQLX_OFFLINE=true cargo test -p e2e --features live-tests --no-run
```

Expected: compiles cleanly with no errors. The test binary is built but not run.

- [ ] **Step 3: Run the live test**

```bash
DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
STARTGG_API_KEY=<your-key> \
SQLX_OFFLINE=true \
cargo test -p e2e --features live-tests -- crawl_and_import_hannover_weekly --nocapture
```

Expected: test passes. You will see crawler log lines for each of the two event days, then stats/H2H assertions succeed.

If `STARTGG_API_KEY` is not set, the test prints `"STARTGG_API_KEY not set — skipping live test"` and returns green.

- [ ] **Step 4: Verify the non-live suite is unaffected**

```bash
bash backend/test.sh
```

Expected: all tests pass (live test is skipped since `live-tests` feature is not active in the normal suite).

- [ ] **Step 5: Commit**

```bash
git add backend/crates/e2e/tests/crawler_live.rs
git commit -m "test(e2e): add live crawler integration test for Hannover Weekly series"
```

---

## Task 3: Re-add the `live-tests` CI job

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: `STARTGG_API_KEY` GitHub secret
- Produces: `live-tests` CI job that runs `cargo test -p e2e --features live-tests`

- [ ] **Step 1: Add the job**

In `.github/workflows/ci.yml`, add the following job after the `topology` job:

```yaml
  live-tests:
    needs: test
    if: ${{ github.event_name == 'push' && secrets.STARTGG_API_KEY != '' }}
    runs-on: ubuntu-latest
    timeout-minutes: 15
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
          workspaces: backend/

      - name: Run live integration tests
        working-directory: backend/
        run: cargo test -p e2e --features live-tests -- --nocapture
```

Note: `continue-on-error: true` means transient network failures (rate limits, start.gg downtime) don't block the main pipeline. `github.event_name == 'push'` limits it to main-branch pushes so it doesn't run on every PR.

- [ ] **Step 2: Verify CI YAML is valid**

```bash
# Confirm the file parses as valid YAML (requires python3 on PATH)
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo OK
```

Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: re-add live-tests job gated on STARTGG_API_KEY secret"
```
