# Deployment Topology Testing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add full deployment topology testing — a `topology` crate whose single smoke test sends real HTTP calls against running `api` and `worker` binaries to exercise the job queue path (NOTIFY/LISTEN, PgListener, `SELECT ... FOR UPDATE SKIP LOCKED`), plus a `GET /health` endpoint to make readiness polling reliable.

**Architecture:** Three parts: (1) a `GET /health` route on the Axum router that returns 200 unconditionally; (2) a `crates/topology` package with a single `#[tokio::test]` gated behind `--features topology-tests` that registers a user, triggers a full import, polls until the worker marks the job done, and asserts data is visible via the API; (3) a `topology` CI job that builds the binaries, spins up Postgres, starts api + worker as background processes, and runs the topology test on every push.

**Tech Stack:** Rust, Axum (health handler), reqwest 0.13 (HTTP client), tokio, serde_json, GitHub Actions

> **Note on API port:** The spec doc references port 8080. The actual default from `Config` (`PORT` env var) is **3000**. All commands and CI env vars in this plan use 3000.

---

## File Map

| File | Change |
|------|--------|
| `backend/crates/api/src/routes/health.rs` | New — `async fn health() -> StatusCode` |
| `backend/crates/api/src/routes/mod.rs` | Add `pub mod health;` and `/health` route |
| `backend/crates/api/tests/api.rs` | Add `health_returns_200` integration test |
| `backend/Cargo.toml` | Add `"crates/topology"` to workspace members |
| `backend/crates/topology/Cargo.toml` | New — package with `topology-tests` feature flag |
| `backend/crates/topology/tests/smoke.rs` | New — `smoke_import_roundtrip` test |
| `.github/workflows/ci.yml` | Add `topology` CI job |

---

### Task 1: Add GET /health endpoint

**Files:**
- Create: `backend/crates/api/src/routes/health.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Add the failing test to api.rs**

In `backend/crates/api/tests/api.rs`, add this test before the first existing `#[sqlx::test]`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn health_returns_200(pool: PgPool) {
    let app = make_app(pool, "");
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd backend && cargo test -p api -- health_returns_200 2>&1 | tail -20
```

Expected: fails — `GET /health` returns 404 (route does not exist yet).

- [ ] **Step 3: Create health.rs**

Create `backend/crates/api/src/routes/health.rs`:

```rust
use axum::http::StatusCode;

pub async fn health() -> StatusCode {
    StatusCode::OK
}
```

- [ ] **Step 4: Register the module and route in mod.rs**

In `backend/crates/api/src/routes/mod.rs`, change:

```rust
pub mod account;
pub mod auth;
pub mod games;
pub mod import;
pub mod invite_links;
pub mod members;
pub mod players;
pub mod projects;
pub mod tournaments;
```

To:

```rust
pub mod account;
pub mod auth;
pub mod games;
pub mod health;
pub mod import;
pub mod invite_links;
pub mod members;
pub mod players;
pub mod projects;
pub mod tournaments;
```

And change the router function to:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health))
        .nest("/auth", auth::router())
        .nest("/account", account::router())
        .nest("/projects", projects::router())
        .route("/games", get(games::search_games))
        .route(
            "/invite/{token}/accept",
            post(invite_links::accept_invite_link),
        )
}
```

- [ ] **Step 5: Run the test to confirm it passes**

```bash
cd backend && cargo test -p api -- health_returns_200 2>&1 | tail -10
```

Expected:
```
test health_returns_200 ... ok

test result: ok. 1 passed; 0 failed
```

- [ ] **Step 6: Run the full test suite for regressions**

```bash
bash backend/test.sh 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/api/src/routes/health.rs \
        backend/crates/api/src/routes/mod.rs \
        backend/crates/api/tests/api.rs
git commit -m "feat(api): add GET /health endpoint for readiness checks"
```

---

### Task 2: Create the topology crate scaffold

**Files:**
- Create: `backend/crates/topology/Cargo.toml`
- Modify: `backend/Cargo.toml`
- Create: `backend/crates/topology/tests/smoke.rs`

- [ ] **Step 1: Create `backend/crates/topology/Cargo.toml`**

```toml
[package]
name = "topology"
version = "0.1.0"
edition = "2024"

[features]
topology-tests = []

[dev-dependencies]
reqwest   = { version = "0.13", features = ["json"] }
serde_json = "1"
tokio     = { version = "1", features = ["full"] }
```

- [ ] **Step 2: Add topology to the workspace**

In `backend/Cargo.toml`, change:

```toml
members  = ["crates/common", "crates/api", "crates/worker", "crates/e2e"]
```

To:

```toml
members  = ["crates/common", "crates/api", "crates/worker", "crates/e2e", "crates/topology"]
```

- [ ] **Step 3: Create the test skeleton**

Create `backend/crates/topology/tests/smoke.rs`:

```rust
#![cfg(feature = "topology-tests")]

use reqwest::Client;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::sleep;

// ── Golden dataset (mirrors import_live.rs) ───────────────────────────────────
// These are completed past Smash Hannover Weeklies — data is immutable.

const PLAYER1_SLUG: &str = "user/06b4042d"; // gamerTag: "King"
const PLAYER2_SLUG: &str = "user/54b7bbf3";
const WEEKLY_100_NAME: &str = "Smash Hannover Weekly #100";
const WEEKLY_88_NAME: &str = "Smash Hannover Weekly #88";

// ── Helpers ───────────────────────────────────────────────────────────────────

fn api_url() -> String {
    std::env::var("API_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}

fn startgg_api_key() -> String {
    std::env::var("STARTGG_API_KEY").expect("STARTGG_API_KEY must be set to run topology tests")
}

async fn wait_for_api(client: &Client) {
    let url = format!("{}/health", api_url());
    for _ in 0..60 {
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        sleep(Duration::from_millis(500)).await;
    }
    panic!("API at {} did not become ready within 30s", api_url());
}

async fn register(client: &Client, email: &str, password: &str) -> String {
    let resp = client
        .post(format!("{}/auth/register", api_url()))
        .json(&json!({
            "email": email,
            "display_name": "topology-test-user",
            "password": password
        }))
        .send()
        .await
        .expect("register request failed");
    assert_eq!(resp.status().as_u16(), 201, "register should return 201");
    let body: Value = resp.json().await.unwrap();
    body["session_id"].as_str().unwrap().to_string()
}

async fn post_json(client: &Client, uri: &str, session_id: &str, body: Value) -> Value {
    client
        .post(format!("{}{uri}", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .json(&body)
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST {uri} failed: {e}"))
        .json()
        .await
        .unwrap_or_else(|e| panic!("POST {uri} response body was not JSON: {e}"))
}

async fn get_json(client: &Client, uri: &str, session_id: &str) -> Value {
    client
        .get(format!("{}{uri}", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .send()
        .await
        .unwrap_or_else(|e| panic!("GET {uri} failed: {e}"))
        .json()
        .await
        .unwrap_or_else(|e| panic!("GET {uri} response body was not JSON: {e}"))
}
```

- [ ] **Step 4: Verify the skeleton compiles**

```bash
cd backend && SQLX_OFFLINE=true cargo test -p topology --features topology-tests --no-run 2>&1 | tail -10
```

Expected: `Finished test` — no errors, no tests to run yet.

- [ ] **Step 5: Commit**

```bash
git add backend/Cargo.toml \
        backend/crates/topology/Cargo.toml \
        backend/crates/topology/tests/smoke.rs
git commit -m "feat(topology): scaffold topology crate with feature flag and HTTP helpers"
```

---

### Task 3: Write the smoke test

**Files:**
- Modify: `backend/crates/topology/tests/smoke.rs`

- [ ] **Step 1: Append the full test to smoke.rs**

At the end of `backend/crates/topology/tests/smoke.rs`, add:

```rust
// ── Test ─────────────────────────────────────────────────────────────────────

/// Exercises the full job-queue path: api inserts job + sends NOTIFY → worker
/// wakes via PgListener → claims job with SELECT ... FOR UPDATE SKIP LOCKED →
/// import runs → job marked done → data visible via API.
///
/// Requires: running api (default http://localhost:3000), running worker,
/// and STARTGG_API_KEY in the environment.
#[tokio::test]
async fn smoke_import_roundtrip() {
    let key = startgg_api_key();
    let client = Client::new();

    // 1. Wait for API to be up
    wait_for_api(&client).await;

    // 2. Register a user
    let session_id = register(&client, "topology@test.com", "password1234").await;

    // 3. Set the start.gg API key (endpoint validates key against start.gg)
    let resp = client
        .put(format!("{}/account/startgg-key", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .json(&json!({ "api_key": key }))
        .send()
        .await
        .expect("PUT /account/startgg-key failed");
    assert_eq!(
        resp.status().as_u16(),
        204,
        "PUT /account/startgg-key should return 204"
    );

    // 4. Create a Melee project
    let project = post_json(
        &client,
        "/projects",
        &session_id,
        json!({
            "name": "Topology Smoke Test",
            "game_id": 1,
            "game_name": "Super Smash Bros. Melee"
        }),
    )
    .await;
    let project_id = project["id"].as_str().expect("project.id missing").to_string();

    // 5. Add two Hannover players with their start.gg accounts
    for (name, slug) in [("Player1", PLAYER1_SLUG), ("Player2", PLAYER2_SLUG)] {
        let player = post_json(
            &client,
            &format!("/projects/{project_id}/players"),
            &session_id,
            json!({ "name": name }),
        )
        .await;
        let player_id = player["id"].as_str().expect("player.id missing").to_string();
        post_json(
            &client,
            &format!("/projects/{project_id}/players/{player_id}/accounts"),
            &session_id,
            json!({ "handle": slug }),
        )
        .await;
    }

    // 6. Trigger import — api inserts job row and sends NOTIFY jobs
    let resp = client
        .post(format!("{}/projects/{project_id}/import", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .send()
        .await
        .expect("POST /projects/{project_id}/import failed");
    assert!(
        resp.status().is_success(),
        "POST /import returned {}",
        resp.status()
    );

    // 7. Poll for job completion — up to 120s (60 × 2s)
    let mut last_status = String::from("unknown");
    for _ in 0..60 {
        sleep(Duration::from_secs(2)).await;
        let import = get_json(
            &client,
            &format!("/projects/{project_id}/import"),
            &session_id,
        )
        .await;
        last_status = import["status"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        match last_status.as_str() {
            "done" => break,
            "failed" => panic!(
                "import job failed: {}",
                import["error"].as_str().unwrap_or("(no error message)")
            ),
            _ => {} // "pending" or "running" — keep polling
        }
    }
    assert_eq!(
        last_status, "done",
        "import did not complete within 120s (last observed status: {last_status})"
    );

    // 8. Assert at least one known Hannover Weekly is in the tournament list
    let tournaments = get_json(
        &client,
        &format!("/projects/{project_id}/tournaments"),
        &session_id,
    )
    .await;
    let names: Vec<&str> = tournaments
        .as_array()
        .expect("GET /tournaments should return an array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(
        names.iter().any(|n| *n == WEEKLY_100_NAME || *n == WEEKLY_88_NAME),
        "expected '{}' or '{}' in tournaments; got: {:?}",
        WEEKLY_100_NAME,
        WEEKLY_88_NAME,
        names
    );

    // 9. Assert at least one set is recorded in stats
    let stats = get_json(
        &client,
        &format!("/projects/{project_id}/stats"),
        &session_id,
    )
    .await;
    let total_sets: usize = stats
        .as_array()
        .expect("GET /stats should return an array")
        .iter()
        .map(|p| {
            p["wins"].as_array().map(|a| a.len()).unwrap_or(0)
                + p["losses"].as_array().map(|a| a.len()).unwrap_or(0)
        })
        .sum();
    assert!(
        total_sets > 0,
        "expected at least one set in stats after import, got 0"
    );
}
```

- [ ] **Step 2: Verify the test compiles**

```bash
cd backend && SQLX_OFFLINE=true cargo test -p topology --features topology-tests --no-run 2>&1 | tail -10
```

Expected: `Finished test` with no errors.

- [ ] **Step 3: Run the test locally (manual, optional)**

Requires a running Postgres, api, and worker. Build the binaries first:

```bash
cd backend && SQLX_OFFLINE=true cargo build -p api -p worker
```

Start the stack in separate terminals:

```bash
# Terminal 1 — Postgres
docker run -d --name rf-topology-manual \
  -e POSTGRES_PASSWORD=postgres \
  -p 15432:5432 postgres:18
until docker exec rf-topology-manual pg_isready -U postgres -q 2>/dev/null; do sleep 0.1; done

# Terminal 2 — api
DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
STARTGG_API_KEY=<your-key> \
CORS_ORIGIN=http://localhost \
./backend/target/debug/api

# Terminal 3 — worker
DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
STARTGG_API_KEY=<your-key> \
./backend/target/debug/worker
```

Run the test:

```bash
cd backend
STARTGG_API_KEY=<your-key> \
cargo test -p topology --features topology-tests -- --nocapture
```

Expected: `smoke_import_roundtrip ... ok` after 2–5 minutes.

Clean up:
```bash
docker rm -f rf-topology-manual
```

- [ ] **Step 4: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/topology/tests/smoke.rs
git commit -m "test(topology): add smoke_import_roundtrip end-to-end topology test"
```

---

### Task 4: Add the topology CI job

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Append the topology job to ci.yml**

In `.github/workflows/ci.yml`, append after the closing line of the `live-tests` job (at the same indentation as `live-tests:`):

```yaml
  topology:
    needs: test
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: backend/

      - name: Start Postgres
        run: |
          docker run -d --name rf-topology \
            -e POSTGRES_PASSWORD=postgres \
            -p 15432:5432 \
            postgres:18
          until docker exec rf-topology pg_isready -U postgres -q 2>/dev/null; do
            sleep 0.1
          done

      - name: Build api and worker
        working-directory: backend/
        env:
          SQLX_OFFLINE: "true"
        run: cargo build -p api -p worker

      - name: Start api
        shell: bash
        working-directory: backend/
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost:15432/postgres
          STARTGG_API_KEY: ${{ secrets.STARTGG_API_KEY }}
          CORS_ORIGIN: http://localhost
        run: ./target/debug/api &

      - name: Start worker
        shell: bash
        working-directory: backend/
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost:15432/postgres
          STARTGG_API_KEY: ${{ secrets.STARTGG_API_KEY }}
        run: ./target/debug/worker &

      - name: Run topology tests
        working-directory: backend/
        env:
          API_URL: http://localhost:3000
          STARTGG_API_KEY: ${{ secrets.STARTGG_API_KEY }}
          SQLX_OFFLINE: "true"
        run: cargo test -p topology --features topology-tests
```

- [ ] **Step 2: Verify ci.yml is valid YAML**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo "YAML OK"
```

Expected: `YAML OK`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add topology job for deployment topology smoke test"
```
