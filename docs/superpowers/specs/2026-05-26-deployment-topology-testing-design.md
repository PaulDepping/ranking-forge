# Design: Deployment Topology Testing

**Date:** 2026-05-26
**Status:** Approved

## Problem

All existing tests — including the Layer 2 live import tests — call `worker::import::run()` as a library function. No test exercises the actual job queue path: the `api` inserting a job row and sending `NOTIFY jobs`, the `worker` waking via `PgListener`, claiming the job with `SELECT ... FOR UPDATE SKIP LOCKED`, and updating status through `pending → running → done`. A bug in this coordination layer would only surface in production.

## Solution

A new `crates/topology` package containing a single smoke test. The test makes real HTTP calls against a running `api` process, triggers an import, and polls until the `worker` completes it. CI starts the actual compiled `api` and `worker` binaries as background processes alongside a Postgres container — no Docker images required.

## Health Endpoint

A new `GET /health` route added to the Axum router returns `200 OK` with no authentication required and no DB ping. If the process is running and accepting connections, that is sufficient to signal readiness. This also serves as a Docker health check for production.

## Crate Structure

```
backend/crates/topology/
  Cargo.toml      — reqwest (json feature), tokio (full), serde_json; no workspace crate deps
  tests/
    smoke.rs      — topology smoke tests
```

No `src/lib.rs` is needed — the package has only integration tests.

Gated by a `topology-tests` Cargo feature:

```toml
[features]
topology-tests = []
```

```rust
// tests/smoke.rs
#![cfg(feature = "topology-tests")]
```

Without `--features topology-tests` the file does not compile and `cargo test --workspace` never touches it.

**No workspace crate dependencies.** The topology crate is a pure HTTP client. Importing `common` or `api` types would silently accept internal type changes that break real HTTP clients. Independence is the point.

## Test Structure

One test: `smoke_import_roundtrip`.

### Golden dataset

Two Hannover player slugs and two tournament names — the same values used in `import_live.rs`, duplicated here intentionally (the topology crate is independent of the `e2e` crate):

```rust
const HANNOVER_PLAYER_1_SLUG: &str = "user/...";
const HANNOVER_PLAYER_2_SLUG: &str = "user/...";
const WEEKLY_1_NAME: &str = "Smash Hannover Weekly #...";
const WEEKLY_2_NAME: &str = "Smash Hannover Weekly #...";
```

### Environment variables

| Variable | Default | Notes |
|---|---|---|
| `API_URL` | `http://localhost:8080` | Where the running API listens |
| `STARTGG_API_KEY` | — | Required; test panics if absent |

### Test flow

1. **Wait for API readiness** — poll `GET /health` every 500ms up to 30s. Panic if the API does not answer in time.

2. **Register** — `POST /auth/register` to create a test user. Extract the session cookie from the response.

3. **Set API key** — `PUT /account/startgg-key` with the real key. This endpoint validates the key against start.gg before storing it. One live API call; no direct DB access from the test.

4. **Create project** — `POST /projects` with `game_id = 1` (Melee) and `game_name = "Super Smash Bros. Melee"`.

5. **Add players** — for each of the two Hannover player slugs: `POST /projects/{id}/players` then `POST /projects/{id}/players/{pid}/accounts`.

6. **Trigger import** — `POST /projects/{id}/import`. The API inserts a job row and sends `NOTIFY jobs`. This is the path no other test exercises.

7. **Poll for completion** — `GET /projects/{id}/import` every 2s up to 120s. Accepted terminal states:
   - `"done"` → proceed to assertions
   - `"failed"` → panic with the error message from the response
   - Timeout → panic with the last observed status

8. **Assert tournament** — `GET /projects/{id}/tournaments`. The response array must contain at least one entry whose `name` matches `WEEKLY_1_NAME` or `WEEKLY_2_NAME`.

9. **Assert stats** — `GET /projects/{id}/stats`. At least one player in the response must have a non-empty `wins` or `losses` array.

Assertions are intentionally shallow. Data correctness is covered by `import_live.rs`. The goal is confirming the full job-queue round-trip (`NOTIFY → PgListener → claim → run → mark done`) works end-to-end.

## CI Integration

New `topology` job in `.github/workflows/ci.yml`:

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
          -p 15432:5432 postgres:18
        until docker exec rf-topology pg_isready -U postgres -q 2>/dev/null; do
          sleep 0.1
        done

    - name: Build api and worker
      working-directory: backend/
      run: cargo build -p api -p worker
      env:
        SQLX_OFFLINE: "true"

    - name: Start api
      shell: bash
      working-directory: backend/
      run: ./target/debug/api &
      env:
        DATABASE_URL: postgres://postgres:postgres@localhost:15432/postgres
        STARTGG_API_KEY: ${{ secrets.STARTGG_API_KEY }}
        CORS_ORIGIN: http://localhost

    - name: Start worker
      shell: bash
      working-directory: backend/
      run: ./target/debug/worker &
      env:
        DATABASE_URL: postgres://postgres:postgres@localhost:15432/postgres
        STARTGG_API_KEY: ${{ secrets.STARTGG_API_KEY }}

    - name: Run topology tests
      working-directory: backend/
      run: cargo test -p topology --features topology-tests
      env:
        API_URL: http://localhost:8080
        STARTGG_API_KEY: ${{ secrets.STARTGG_API_KEY }}
        SQLX_OFFLINE: "true"
```

`needs: test` ensures topology only runs when the main test suite passes.

The `api` binary runs migrations at startup, so no separate migration step is needed. The `GET /health` wait loop in the test absorbs binary startup time.

Processes terminate automatically when the CI job ends.

## Out of Scope

- Testing the `docker compose` networking layer specifically — the goal is job-queue correctness, not container networking.
- Asserting every mapped field — data depth is covered by `import_live.rs`.
- Multi-worker concurrency (`SELECT ... FOR UPDATE SKIP LOCKED`) — a single worker instance is sufficient to exercise the claim path.
