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

## Test scripts

```bash
# Run all tests (backend + frontend unit + frontend e2e)
# Quiet by default: prints PASS/FAIL per section, dumps output only on failure
bash test.sh

# Stream all output (useful when debugging a failure interactively)
bash test.sh --verbose   # or -v

# Backend only — spins up an ephemeral Postgres container via Docker, then runs cargo test --workspace
# Also quiet by default; supports --verbose / -v
bash backend/test.sh

# Frontend unit tests only
cd web && npm run test:unit

# Frontend e2e tests only (Playwright auto-starts mock API + SvelteKit dev server)
cd web && npm run test:e2e

# Update the sqlx offline query cache after adding/modifying any sqlx::query! macro
bash backend/prepare-sqlx.sh
```

`backend/test.sh` handles the full backend suite (common, api, e2e) without a pre-existing database.
`backend/prepare-sqlx.sh` runs migrations then `cargo sqlx prepare --workspace -- --all-targets` against a fresh container.
Playwright e2e tests are self-contained: the config auto-starts a mock API on port 9999 and the SvelteKit dev server on port 5174.

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
```

## Environment variables

| Variable | Used by | Notes |
|---|---|---|
| `DATABASE_URL` | api, worker | Postgres connection string |
| `STARTGG_API_KEY` | api, worker | Shared start.gg API key |
| `CORS_ORIGIN` | api | Default: `https://rankingforge.example.com`; set to `http://localhost:5173` for local dev |

## start.gg API reference

Local documentation lives in `docs/startgg/`:

- `schema.graphql` — full SDL schema from GraphQL introspection; the authoritative type and field reference for the start.gg API
- `project-notes.md` — the 5 GraphQL operations this project uses, rate limits, auth, and known API quirks (including the `ActivityState` string/int inconsistency)
- `fetch-schema.sh` — run this to refresh `schema.graphql` before extending the query set; requires `STARTGG_API_KEY` in environment or root `.env`

## Key architectural patterns

### sqlx compile-time query checking

All DB queries use `sqlx::query!` macros. The schema is the source of truth for Rust types. After adding any new `sqlx::query!` block, run `bash backend/prepare-sqlx.sh` to update `.sqlx/`. The `.sqlx/` directory is committed.

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

## Frontend

SvelteKit + TypeScript in `web/`. See `web/CLAUDE.md` for frontend-specific guidance.

**Component policy:** Always use shadcn-svelte components in preference to raw HTML elements or hand-rolled Tailwind. Never write a raw `<input>`, `<select>`, `<button>`, `<table>`, or `<dialog>` when a shadcn equivalent exists. Add new shadcn components with `npx shadcn-svelte@latest add --yes --overwrite <name>` — do not write component files by hand.

Installed components: `alert`, `badge`, `button`, `calendar`, `card`, `checkbox`, `collapsible`, `command`, `dialog`, `empty`, `input`, `label`, `popover`, `scroll-area`, `select`, `separator`, `skeleton`, `table`, `tabs`, `tooltip`. Install others as needed with `npx shadcn-svelte@latest add --yes --overwrite <name>`.

**shadcn-svelte reference docs** are cached locally in `docs/shadcn-svelte/`. Index: `docs/shadcn-svelte.md`. Individual component docs: `docs/shadcn-svelte/docs/components/<name>.md`. Read the relevant file instead of fetching from the web.
