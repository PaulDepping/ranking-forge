# start.gg API Testing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two-layer API testing: offline GraphQL query syntax validation against the SDL schema (always runs), and live full-pipeline import tests against real Smash Hannover Weekly data (opt-in via `--features live-tests`).

**Architecture:** Layer 1 — `graphql-parser` dev-dep in `common` validates all 7 query string constants as part of normal `cargo test -p common`, with no network access. Layer 2 — `import_live.rs` in `e2e`, gated by `#![cfg(feature = "live-tests")]`, calls `worker::import::run()` against the real start.gg API and an ephemeral `#[sqlx::test]` Postgres DB, asserting specific results from hardcoded past Smash Hannover Weeklies.

**Tech Stack:** Rust, `graphql-parser` (query syntax validation), `sqlx::test` (ephemeral Postgres per test), `worker::import::run` (import pipeline called as library code), GitHub Actions secret (`STARTGG_API_KEY`)

---

### Task 1: Move schema.graphql into the common crate

**Files:**
- Move: `docs/startgg/schema.graphql` → `backend/crates/common/src/startgg/schema.graphql`
- Modify: `docs/startgg/fetch-schema.sh`
- Modify: `docs/startgg/project-notes.md`

- [ ] **Step 1: Move the schema file**

```bash
git mv docs/startgg/schema.graphql backend/crates/common/src/startgg/schema.graphql
```

- [ ] **Step 2: Update fetch-schema.sh output path**

In `docs/startgg/fetch-schema.sh`, change the last two lines:

```bash
# Before:
  > "$SCRIPT_DIR/schema.graphql"
echo "Done. Schema written to docs/startgg/schema.graphql"

# After:
  > "$REPO_ROOT/backend/crates/common/src/startgg/schema.graphql"
echo "Done. Schema written to backend/crates/common/src/startgg/schema.graphql"
```

- [ ] **Step 3: Update the schema path reference in project-notes.md**

In `docs/startgg/project-notes.md`, find the opening line referencing `schema.graphql` and change it to reference `backend/crates/common/src/startgg/schema.graphql`.

- [ ] **Step 4: Verify existing tests still pass**

```bash
cd backend && cargo test -p common
```

Expected: all tests pass — no code references the old path yet.

- [ ] **Step 5: Commit**

```bash
git add docs/startgg/fetch-schema.sh \
        docs/startgg/project-notes.md \
        backend/crates/common/src/startgg/schema.graphql
git commit -m "refactor(common): move schema.graphql into startgg module source tree"
```

---

### Task 2: Add offline schema validation tests

**Files:**
- Modify: `backend/crates/common/Cargo.toml`
- Rename: `backend/crates/common/src/startgg/operations.rs` → `backend/crates/common/src/startgg/operations/mod.rs`
- Create: `backend/crates/common/src/startgg/operations/tests.rs`

- [ ] **Step 1: Add graphql-parser as a dev-dependency**

```bash
cd backend && cargo add --dev graphql-parser -p common
```

- [ ] **Step 2: Convert operations.rs to a directory module**

```bash
mkdir backend/crates/common/src/startgg/operations
git mv backend/crates/common/src/startgg/operations.rs \
       backend/crates/common/src/startgg/operations/mod.rs
```

The `mod operations;` declaration in `startgg/mod.rs` works unchanged — Rust resolves it to either `operations.rs` or `operations/mod.rs` automatically.

- [ ] **Step 3: Declare the tests submodule at the end of operations/mod.rs**

Append to `backend/crates/common/src/startgg/operations/mod.rs`:

```rust
#[cfg(test)]
mod tests;
```

- [ ] **Step 4: Create operations/tests.rs**

Create `backend/crates/common/src/startgg/operations/tests.rs`:

```rust
use super::{
    EVENT_ENTRANTS_QUERY, EVENT_PHASES_QUERY, EVENT_SETS_QUERY, GAME_SEARCH_QUERY,
    TOURNAMENTS_BY_USER_ALL_GAMES_QUERY, TOURNAMENTS_BY_USER_QUERY, USER_BY_SLUG_QUERY,
};

fn assert_query_parses(query: &'static str) {
    graphql_parser::parse_query::<String>(query)
        .unwrap_or_else(|e| panic!("query failed to parse: {e}"));
}

// Validates the schema file itself is well-formed SDL — catches corruption or
// truncation of schema.graphql.
#[test]
fn schema_parses() {
    graphql_parser::parse_schema::<String>(include_str!("../schema.graphql"))
        .unwrap_or_else(|e| panic!("schema.graphql failed to parse: {e}"));
}

// Note: graphql-parser validates syntax only, not field names against the
// schema. A field-name typo (e.g. `displayNme`) would not be caught here.
// These tests catch: garbled syntax, mismatched braces, invalid variable
// declarations, and unknown query keywords. Full schema-aware validation
// would require apollo-compiler or similar — that is left as future work.

#[test]
fn game_search_query_is_valid() {
    assert_query_parses(GAME_SEARCH_QUERY);
}

#[test]
fn user_by_slug_query_is_valid() {
    assert_query_parses(USER_BY_SLUG_QUERY);
}

#[test]
fn tournaments_by_user_query_is_valid() {
    assert_query_parses(TOURNAMENTS_BY_USER_QUERY);
}

#[test]
fn tournaments_by_user_all_games_query_is_valid() {
    assert_query_parses(TOURNAMENTS_BY_USER_ALL_GAMES_QUERY);
}

#[test]
fn event_entrants_query_is_valid() {
    assert_query_parses(EVENT_ENTRANTS_QUERY);
}

#[test]
fn event_sets_query_is_valid() {
    assert_query_parses(EVENT_SETS_QUERY);
}

#[test]
fn event_phases_query_is_valid() {
    assert_query_parses(EVENT_PHASES_QUERY);
}
```

- [ ] **Step 5: Run the new tests**

```bash
cd backend && cargo test -p common -- operations::tests
```

Expected output (8 tests, all passing):
```
test startgg::operations::tests::schema_parses ... ok
test startgg::operations::tests::game_search_query_is_valid ... ok
test startgg::operations::tests::user_by_slug_query_is_valid ... ok
test startgg::operations::tests::tournaments_by_user_query_is_valid ... ok
test startgg::operations::tests::tournaments_by_user_all_games_query_is_valid ... ok
test startgg::operations::tests::event_entrants_query_is_valid ... ok
test startgg::operations::tests::event_sets_query_is_valid ... ok
test startgg::operations::tests::event_phases_query_is_valid ... ok

test result: ok. 8 passed; 0 failed
```

- [ ] **Step 6: Run the full common test suite to check for regressions**

```bash
cd backend && cargo test -p common
```

Expected: all prior tests still pass alongside the 8 new ones.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/common/Cargo.toml \
        backend/crates/common/src/startgg/operations/ \
        backend/Cargo.lock
git commit -m "test(common): add offline GraphQL schema validation tests for all 7 query constants"
```

---

### Task 3: Write the discovery helper and identify the golden dataset

This task writes a discovery test, runs it against the real API, and collects the data needed for Task 4. It ends with a required user checkpoint.

**Files:**
- Modify: `backend/crates/e2e/Cargo.toml`
- Create: `backend/crates/e2e/tests/import_live.rs`

- [ ] **Step 1: Add the live-tests feature to e2e Cargo.toml**

In `backend/crates/e2e/Cargo.toml`, add after `[package]`:

```toml
[features]
live-tests = []
```

- [ ] **Step 2: Create import_live.rs with the discovery test**

Create `backend/crates/e2e/tests/import_live.rs`:

```rust
#![cfg(feature = "live-tests")]

use common::startgg::StartggClient;

// ── Discovery helper ──────────────────────────────────────────────────────────
// Run once with a known Hannover player slug to identify golden tournament data.
// Command:
//   DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
//   STARTGG_API_KEY=<your-key> \
//   SQLX_OFFLINE=true \
//   cargo test -p e2e --features live-tests -- discover_hannover_weeklies --nocapture
//
// After collecting output, replace this file with the golden tests in Task 4.

#[tokio::test]
async fn discover_hannover_weeklies() {
    let key = std::env::var("STARTGG_API_KEY")
        .expect("STARTGG_API_KEY must be set to run live tests");

    // Replace with your start.gg slug. Find it at start.gg — the URL when
    // viewing a profile reads: start.gg/user/<slug>
    let player_slug = "user/YOUR_SLUG_HERE";
    let melee_game_id: i64 = 1;

    let client = StartggClient::new(key);

    let user = client
        .user_by_slug(player_slug)
        .await
        .expect("API call failed")
        .expect("player slug not found — check it is correct");
    eprintln!("User ID: {}  gamerTag: {:?}", user.id, user.gamer_tag());

    // Fetch page 1 of Melee tournaments (50 per page — enough for a local player)
    let page = client
        .tournaments_by_user(user.id, melee_game_id, 1, 50)
        .await
        .expect("tournaments_by_user failed");
    eprintln!(
        "\nTotal pages: {:?}  Tournaments on page 1: {}",
        page.page_info.as_ref().map(|p| p.total_pages),
        page.nodes.len()
    );

    for t in &page.nodes {
        eprintln!("\n=== {} ===", t.name);
        eprintln!("  slug:    {}", t.slug);
        eprintln!("  state:   {:?}", t.state);
        eprintln!("  startAt: {:?}", t.start_at);
        if let Some(events) = &t.events {
            for e in events {
                eprintln!("  event: {} (id: {})", e.name, e.id);
                eprintln!("    numEntrants: {:?}  state: {:?}", e.num_entrants, e.state);
            }
        }
    }
}
```

- [ ] **Step 3: Fill in your player slug**

In `import_live.rs`, replace `"user/YOUR_SLUG_HERE"` with a real start.gg user slug for a known Smash Hannover scene player. The slug is the path component after `start.gg/` when viewing a profile.

- [ ] **Step 4: Start a Postgres container**

```bash
docker run -d --name rf-live-discover \
  -e POSTGRES_PASSWORD=postgres \
  -p 15432:5432 postgres:18
until docker exec rf-live-discover pg_isready -U postgres -q 2>/dev/null; do sleep 0.1; done
```

- [ ] **Step 5: Run the discovery test**

```bash
cd backend
DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
STARTGG_API_KEY=your-key-here \
SQLX_OFFLINE=true \
cargo test -p e2e --features live-tests -- discover_hannover_weeklies --nocapture
```

- [ ] **Step 6: Stop the Postgres container**

```bash
docker rm -f rf-live-discover
```

---

**⚠️ CHECKPOINT — User action required before continuing to Task 4**

Inspect the printed output. Identify **2–3 past completed Smash Hannover Weekly tournaments** you can verify results for. For each one, record:

| Field | Example |
|---|---|
| Tournament name | `"Smash Hannover Weekly #42"` |
| Tournament slug | `"tournament/smash-hannover-weekly-42"` |
| Melee Singles event state | should be `"COMPLETED"` |
| A second Hannover player slug | `"user/their-slug"` |

Do not proceed to Task 4 until you have confirmed the output shows real Smash Hannover data and have these values in hand.

---

### Task 4: Write the golden-dataset live import tests

**Files:**
- Modify: `backend/crates/e2e/tests/import_live.rs` (replace discovery helper entirely)

The test structure is identical to the existing `full_flow.rs`: register → create project → add player → link start.gg account → `worker::import::run()` → assert via API.

- [ ] **Step 1: Replace import_live.rs with the full golden-dataset implementation**

Replace the entire contents of `backend/crates/e2e/tests/import_live.rs` with the following, then fill in every `FILL_IN` constant from your Task 3 discovery output:

```rust
#![cfg(feature = "live-tests")]

use api::{routes, state::AppState};
use axum::{Router, body::Body, http::Request, http::StatusCode};
use common::{jobs::ImportParams, startgg::StartggClient};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// ── Golden dataset: Smash Hannover Weekly ─────────────────────────────────────
// Verified by running discover_hannover_weeklies (Task 3) and confirming output.
// These are completed past tournaments — data is immutable.

const HANNOVER_PLAYER_1_SLUG: &str = "user/FILL_IN"; // your slug from Task 3
const HANNOVER_PLAYER_2_SLUG: &str = "user/FILL_IN"; // a second Hannover player

const WEEKLY_1_NAME: &str = "FILL_IN"; // e.g. "Smash Hannover Weekly #42"
const WEEKLY_1_SLUG: &str = "tournament/FILL_IN"; // e.g. "tournament/smash-hannover-weekly-42"

const WEEKLY_2_NAME: &str = "FILL_IN";
const WEEKLY_2_SLUG: &str = "tournament/FILL_IN";

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_app(pool: PgPool) -> Router {
    let state = AppState {
        db: pool,
        cors_origin: "http://localhost".to_string(),
        startgg_base_url: "https://api.start.gg/gql/alpha".to_string(),
    };
    routes::router().with_state(state)
}

async fn read_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn register(app: &Router, email: &str, password: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({
                "email": email,
                "display_name": "live-test-user",
                "password": password
            }))
            .unwrap(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    format!("session_id={}", body["session_id"].as_str().unwrap())
}

async fn post_json(
    app: &Router,
    uri: &str,
    cookie: &str,
    body: Value,
) -> axum::response::Response {
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

async fn set_startgg_api_key(pool: &PgPool, cookie: &str, api_key: &str) {
    let session_id: Uuid = cookie
        .split('=')
        .nth(1)
        .unwrap()
        .parse()
        .expect("invalid session UUID in cookie");
    sqlx::query!(
        "UPDATE users SET startgg_api_key = $1
         WHERE id = (SELECT user_id FROM sessions WHERE id = $2)",
        api_key,
        session_id,
    )
    .execute(pool)
    .await
    .expect("failed to set startgg_api_key");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Live import: Smash Hannover Weekly #1.
/// Verifies the tournament imports and appears in the project tournament list.
#[sqlx::test(migrations = "../../migrations")]
async fn import_hannover_weekly_1(pool: PgPool) {
    let key = std::env::var("STARTGG_API_KEY")
        .expect("STARTGG_API_KEY must be set to run live tests");

    let app = make_app(pool.clone());
    let cookie = register(&app, "live1@test.com", "password123").await;
    set_startgg_api_key(&pool, &cookie, &key).await;

    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({
            "name": "Live Test Weekly 1",
            "game_id": 1,
            "game_name": "Super Smash Bros. Melee"
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Player1"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let player_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{player_id}/accounts"),
        &cookie,
        json!({"handle": HANNOVER_PLAYER_1_SLUG}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let client = StartggClient::new(key);
    worker::import::run(
        &pool,
        &client,
        Uuid::parse_str(&project_id).unwrap(),
        Uuid::nil(),
        ImportParams::default(),
    )
    .await
    .expect("import failed");

    let resp = get_req(&app, &format!("/projects/{project_id}/tournaments"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tournaments = read_json(resp).await;
    let tournaments = tournaments.as_array().unwrap();

    let weekly = tournaments
        .iter()
        .find(|t| t["name"].as_str().unwrap_or("") == WEEKLY_1_NAME)
        .unwrap_or_else(|| {
            panic!(
                "'{}' not found in imported tournaments. Got: {:?}",
                WEEKLY_1_NAME,
                tournaments.iter().map(|t| &t["name"]).collect::<Vec<_>>()
            )
        });

    // At least one Melee Singles event was imported with entrants
    let events = weekly["events"].as_array().unwrap();
    assert!(!events.is_empty(), "expected at least one event in {}", WEEKLY_1_NAME);
    assert!(
        events[0]["num_entrants"].as_i64().unwrap_or(0) > 0,
        "expected non-zero entrant count in first event"
    );
}

/// Live import: Smash Hannover Weekly #2 with two players.
/// Verifies the tournament imports and at least one set result is recorded.
#[sqlx::test(migrations = "../../migrations")]
async fn import_hannover_weekly_2(pool: PgPool) {
    let key = std::env::var("STARTGG_API_KEY")
        .expect("STARTGG_API_KEY must be set to run live tests");

    let app = make_app(pool.clone());
    let cookie = register(&app, "live2@test.com", "password123").await;
    set_startgg_api_key(&pool, &cookie, &key).await;

    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({
            "name": "Live Test Weekly 2",
            "game_id": 1,
            "game_name": "Super Smash Bros. Melee"
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    for (name, slug) in [
        ("Player1", HANNOVER_PLAYER_1_SLUG),
        ("Player2", HANNOVER_PLAYER_2_SLUG),
    ] {
        let resp = post_json(
            &app,
            &format!("/projects/{project_id}/players"),
            &cookie,
            json!({"name": name}),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        let player_id = read_json(resp).await["id"].as_str().unwrap().to_string();

        let resp = post_json(
            &app,
            &format!("/projects/{project_id}/players/{player_id}/accounts"),
            &cookie,
            json!({"handle": slug}),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    let client = StartggClient::new(key);
    worker::import::run(
        &pool,
        &client,
        Uuid::parse_str(&project_id).unwrap(),
        Uuid::nil(),
        ImportParams::default(),
    )
    .await
    .expect("import failed");

    // Weekly 2 appears in the tournament list
    let resp = get_req(&app, &format!("/projects/{project_id}/tournaments"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tournaments = read_json(resp).await;
    assert!(
        tournaments
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"].as_str().unwrap_or("") == WEEKLY_2_NAME),
        "'{}' not found in imported tournaments",
        WEEKLY_2_NAME
    );

    // At least one set was recorded across both players
    let resp = get_req(&app, &format!("/projects/{project_id}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    let total_sets: usize = stats
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            s["wins"].as_array().map(|a| a.len()).unwrap_or(0)
                + s["losses"].as_array().map(|a| a.len()).unwrap_or(0)
        })
        .sum();
    assert!(total_sets > 0, "expected at least one set recorded after import");
}
```

- [ ] **Step 2: Fill in every FILL_IN constant**

Open `import_live.rs` and replace each `"FILL_IN"` with real values from Task 3:
- `HANNOVER_PLAYER_1_SLUG` — your slug
- `HANNOVER_PLAYER_2_SLUG` — a second Hannover player's slug
- `WEEKLY_1_NAME` and `WEEKLY_1_SLUG` — first target tournament
- `WEEKLY_2_NAME` and `WEEKLY_2_SLUG` — second target tournament

- [ ] **Step 3: Update the sqlx offline query cache**

The `set_startgg_api_key` helper uses a `sqlx::query!` macro. Run `prepare-sqlx.sh` to add it to the offline cache (even if the identical query exists in `full_flow.rs`, running this is safe and ensures consistency):

```bash
cd backend && bash prepare-sqlx.sh
```

This spins up a temporary Postgres container, runs migrations, and updates `.sqlx/`. Takes 1–2 minutes.

- [ ] **Step 4: Start a Postgres container and run the live tests**

```bash
docker run -d --name rf-live-test \
  -e POSTGRES_PASSWORD=postgres \
  -p 15432:5432 postgres:18
until docker exec rf-live-test pg_isready -U postgres -q 2>/dev/null; do sleep 0.1; done

cd backend
DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
STARTGG_API_KEY=your-key-here \
SQLX_OFFLINE=true \
cargo test -p e2e --features live-tests -- --nocapture

docker rm -f rf-live-test
```

Expected: both tests pass. Total run time: 2–5 minutes (real API calls + full import pipeline per test).

- [ ] **Step 5: Run the normal test suite to verify no regressions**

```bash
cd backend && bash test.sh
```

Expected: all existing tests pass unchanged — live tests are invisible without `--features live-tests`.

- [ ] **Step 6: Commit**

```bash
git add backend/crates/e2e/Cargo.toml \
        backend/crates/e2e/tests/import_live.rs \
        backend/.sqlx/
git commit -m "test(e2e): add live golden-dataset import tests for Smash Hannover Weeklies"
```

---

### Task 5: Add the live-tests CI job

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Append the live-tests job to ci.yml**

In `.github/workflows/ci.yml`, append after the existing `test` job (at the same indentation level as `test:` and `build-backend:`):

```yaml
  live-tests:
    needs: test
    # Only run on pushes to main/tags where the STARTGG_API_KEY secret is available.
    # Pull requests from forks do not have access to secrets and are excluded.
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: backend/

      - name: Start Postgres
        run: |
          docker run -d --name rf-live-test \
            -e POSTGRES_PASSWORD=postgres \
            -p 15432:5432 postgres:18
          until docker exec rf-live-test pg_isready -U postgres -q 2>/dev/null; do
            sleep 0.1
          done

      - name: Run live import tests
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost:15432/postgres
          STARTGG_API_KEY: ${{ secrets.STARTGG_API_KEY }}
          SQLX_OFFLINE: "true"
        working-directory: backend
        run: cargo test -p e2e --features live-tests
```

- [ ] **Step 2: Add STARTGG_API_KEY as a GitHub repository secret**

In the GitHub repository UI: Settings → Secrets and variables → Actions → New repository secret.
- Name: `STARTGG_API_KEY`
- Value: your start.gg API key

This step is done in the browser, not via the CLI.

- [ ] **Step 3: Verify the live-tests binary compiles**

```bash
cd backend && SQLX_OFFLINE=true cargo test -p e2e --features live-tests --no-run
```

Expected: compiles successfully with no errors.

- [ ] **Step 4: Commit and push**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add live-tests job for start.gg API integration tests"
```
