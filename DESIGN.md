# RankingForge

A platform for creating and managing rankings of players.

This platform serves a common use case in the smash scene: helping TOs and other figureheads create Power Rankings of their players.

## Implementation Status

_Last updated: 2026-05-09_

| Component | Status |
|---|---|
| DB schema + migrations | ✅ Done |
| API: AppState, router, CORS | ✅ Done |
| API: Auth endpoints (`/auth/*`) + `AuthUser` extractor | ✅ Done |
| API: Projects/Players CRUD | ✅ Done |
| start.gg GraphQL client | ✅ Done |
| Import worker | ⬜ Phase 4 |
| API: Tournament deselection + stats | ⬜ Phase 5 |
| Frontend (SvelteKit) | ⬜ Phase 6 |

See `ROADMAP.md` for the detailed phase breakdown and implementation notes.

## Scope

The initial POC covers data collection and display only. Publishing rankings and allowing guests to view the underlying stats is planned but out of scope for now.

Out of scope for POC:
- Public / guest access to rankings
- Email verification or password reset
- OAuth / social login
- Multi-player doubles events

## Use Cases

1. As a user I go on the website and create an account with a username and password.
2. I create a ranking project.
3. I insert/select a list of relevant, eligible players, together with their start.gg accounts (a player may have zero or more start.gg accounts).
4. I select a video game that start.gg supports.
5. The server queries all start.gg tournaments that any of those players have entered. I am shown a list of those tournaments and their relevant events, and can manually deselect any I do not want to count.
6. I get an overview of that list of players: ordered by upset factor, the wins and losses of each player, and a head-to-head table of set records between each player.

## Architecture

### Processes

The system runs as three separate processes / Docker containers:

| Service  | Role                                                                 |
|----------|----------------------------------------------------------------------|
| `db`     | PostgreSQL — single source of truth                                  |
| `api`    | HTTP API server (Axum) — serves the browser, manages sessions        |
| `worker` | Background worker — queries start.gg, imports tournament data        |

### Worker Communication

The `api` and `worker` communicate via a Postgres job queue:

1. When a user triggers an import, the `api` inserts a row into the `jobs` table and sends a `NOTIFY jobs` signal.
2. The `worker` listens with `LISTEN jobs` (via sqlx `PgListener`) and wakes up immediately.
3. The worker claims a job with `SELECT ... FOR UPDATE SKIP LOCKED` — safe for multiple concurrent workers.
4. The worker updates job `status` (pending → running → done/failed) and writes results to the database.

This scales horizontally: running N worker containers automatically distributes jobs without any coordination layer beyond Postgres.

### API Style

REST over HTTP with JSON bodies. Session authentication via HttpOnly cookies (server-side sessions stored in Postgres).

## Backend

* Written in Rust using Axum, Tower, sqlx, etc.
* Workspace with two binaries: `api` and `worker`, sharing a common `db` library crate.
* PostgreSQL as the database.
* sqlx compile-time query checking (`sqlx::query!`) — the schema is the source of truth for Rust types.

## Testing

Two separate test suites, run independently:

| Suite | Command | What it covers |
|---|---|---|
| `cargo test -p common` | No DB needed | Unit tests for pure logic; wiremock-based tests for `StartggClient` operations |
| `cargo test -p api` | Needs `DATABASE_URL` | Integration tests: real isolated DB per test (`#[sqlx::test]`), wiremock for start.gg calls |

Key design decisions that make this work:

- **`StartggClient::new_with_base_url`** — any code that calls start.gg goes through `StartggClient`, never inline `reqwest`. Tests construct the client with a wiremock URL, so no real network calls are made.
- **No DB mocks** — `#[sqlx::test]` spins up a fresh schema per test. Mocking sqlx would add complexity and miss schema mismatches that compile-time query checking doesn't catch (e.g. constraint violations, NULL handling).
- **Self-contained tests** — each test registers its own users, creates its own data. No shared fixtures, no ordering dependencies.

## Frontend

Written in Svelte with shadcn/ui components. Talks to the `api` over REST.

## Data Model

### Entities

```
users
  └── ranking_projects
        ├── players
        │     └── startgg_accounts       (0..n per player)
        ├── jobs                          (import queue)
        └── project_events               (included/excluded per project)
              └── events
                    └── tournament
                    └── entrants          (player + seed per event)
                          └── sets        (winner / loser entrant pairs)
```

### Key Relationships

- A **player** belongs to exactly one **ranking_project**.
- A player has zero or more **startgg_accounts** (identified by start.gg user ID and slug).
- **tournaments** and **events** are imported from start.gg and shared across projects.
- **project_events** is a join table with an `included` flag (default `true`) for per-project event deselection.
- **entrants** represent one player's participation in one event. `player_id` is nullable — entrants whose start.gg user ID doesn't match any known startgg_account are stored but not linked.
- **sets** record the winner and loser entrant for each match. Scores are not stored (not needed for upset factor).

## API Overview

See `api/openapi.yaml` for the full contract.

| Group             | Endpoints                                           |
|-------------------|-----------------------------------------------------|
| Auth              | POST /auth/register, /auth/login, /auth/logout; GET /auth/me |
| Projects          | CRUD on /projects                                   |
| Players           | CRUD on /projects/:id/players                       |
| start.gg accounts | POST/DELETE /projects/:id/players/:pid/accounts     |
| Import            | POST/GET /projects/:id/import                       |
| Tournaments       | GET /projects/:id/tournaments                       |
| Events            | PATCH /projects/:id/events/:eid (toggle included)   |
| Stats             | GET /projects/:id/stats, /projects/:id/head-to-head |
| Games             | GET /games?q= (proxies start.gg game search)        |

## Upset Factor

Upset Factor is calculated by comparing the players' projected final losers round based on their seeds.

Seeds are taken from the start.gg entry seed assigned by the TO at tournament registration. No manual override.

The common smash-community algorithm is:

1. Convert each seed to expected placement.
   Example: seed 40 → projected ~33rd.
2. Convert that placement to a "Top X" finish.
   Example: 33rd → Top 48.
3. Convert that Top X finish to a projected losers-round number.
   Example: Top 48 → projected losers round 11.
4. Subtract the higher-seeded player's projected round from the lower-seeded winner's projected round.

A positive result means the lower-seeded player performed better than expected (an upset). The aggregate upset factor for a player is the sum of their upset factor across all counted sets.

## Infrastructure

### URLs

| Role | Public URL |
|---|---|
| Frontend | `https://rankingforge.example.com` |
| API | `https://api.rankingforge.example.com` |

The frontend and API are on different subdomains (different origins). The API must configure CORS to allow `https://rankingforge.example.com` with `allow_credentials: true`. Client-side fetches from the browser must use `credentials: 'include'`. `SameSite=Strict` on the session cookie is safe — both hosts are under the same registrable domain (`example.com`).

The SvelteKit server has two API base URL env vars: `PUBLIC_API_URL` (sent to the browser, used for client-side fetches) and `INTERNAL_API_URL` (Docker-internal address, used by server-side `load` functions to avoid the public network).

### Docker Compose

```yaml
services:
  db:      # postgres
  api:     # ranking_forge_api binary
  worker:  # ranking_forge_worker binary
```

All three share a Docker network. `api` and `worker` connect to `db` via the `DATABASE_URL` environment variable.
