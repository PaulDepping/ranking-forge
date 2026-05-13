# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

RankingForge is a platform for creating smash-scene power rankings. It collects tournament data from start.gg and computes upset-factor statistics.

Three services: `db` (PostgreSQL), `api` (Axum HTTP server), `worker` (background import worker). Communication between `api` and `worker` is via a Postgres job queue using `NOTIFY`/`LISTEN`.

## Repository layout

```
backend/          Rust workspace
  crates/
    common/       Shared library: DB models, jobs queue, StartggClient, upset-factor logic
    api/          Axum HTTP server binary
    worker/       Background import worker binary
    e2e/          End-to-end tests (full pipeline, real router + worker)
  migrations/     SQL migrations (run by sqlx at startup)
  .sqlx/          Offline query cache (committed; required for SQLX_OFFLINE=true builds)
  openapi.yaml    Full REST API contract
web/              SvelteKit frontend (Phase 6 — not yet implemented)
DESIGN.md         Architecture reference with full data model and API overview
ROADMAP.md        Phase breakdown and implementation decisions
```

## Backend commands

All commands run from `backend/`:

```bash
# Build
cargo build

# Run API server (requires DATABASE_URL in .env)
cargo run --bin api

# Run worker
cargo run --bin worker

# Tests — no DB needed
cargo test -p common

# Tests — requires DATABASE_URL
DATABASE_URL=postgres://... cargo test -p api
DATABASE_URL=postgres://... cargo test -p e2e

# Run a single test
cargo test -p api -- test_name
cargo test -p common -- test_name

# After adding any sqlx::query! macro — update the offline cache
cargo sqlx prepare
# If tests have query macros too:
cargo sqlx prepare --workspace -- --all-targets
```

## Environment variables

| Variable | Used by | Notes |
|---|---|---|
| `DATABASE_URL` | api, worker | Postgres connection string |
| `STARTGG_API_KEY` | api, worker | Shared start.gg API key |
| `CORS_ORIGIN` | api | Default: `https://rankingforge.example.com`; set to `http://localhost:5173` for local dev |

## Key architectural patterns

### sqlx compile-time query checking

All DB queries use `sqlx::query!` macros. The schema is the source of truth for Rust types. After adding any new `sqlx::query!` block, run `cargo sqlx prepare` from `backend/` to update `.sqlx/`. The `.sqlx/` directory is committed.

### StartggClient injection

`StartggClient` lives in `common` and is the only way to call start.gg — never use raw `reqwest` in routes or worker code. `StartggClient::new_with_base_url` is the test constructor: it accepts a wiremock URL so tests never hit the real network.

### No DB mocks

Tests use `#[sqlx::test(migrations = "../../migrations")]` which spins up a real isolated schema per test. Do not mock sqlx.

### AppState

`AppState` (`crates/api/src/state.rs`) holds `PgPool`, `StartggClient`, `session_secret`, and `cors_origin`. It is passed to the Axum router and accessed via `State<AppState>` in handlers.

### AuthUser extractor

Protected routes receive an `AuthUser` via `impl FromRequestParts`. It reads the `session_id` cookie, queries the `sessions` table, and returns the user or 401. Axum 0.8 uses native `async fn` — do not add `#[async_trait]`.

### Job queue

`common::jobs` provides `enqueue`, `claim`, `mark_running/done/failed`. The worker listens via `PgListener` and claims jobs with `SELECT ... FOR UPDATE SKIP LOCKED` — safe for concurrent workers.

## Dependency management

Use `cargo add` to add Rust dependencies. Never edit version numbers in `Cargo.toml` manually.

## Frontend (Phase 6 — not yet started)

SvelteKit + TypeScript in `web/`. Before writing any page, create `src/lib/api.ts` as a thin fetch wrapper that sets `credentials: 'include'` and prefixes `PUBLIC_API_URL` (client-side) or `INTERNAL_API_URL` (server-side). Install shadcn/ui for Svelte before starting UI work: `npx shadcn-svelte@latest init`.
