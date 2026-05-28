# RankingForge

A platform for creating and managing rankings of players.

This platform serves a common use case in the smash scene: helping TOs and other figureheads create Power Rankings of their players.

## Scope

RankingForge is a multi-user platform for smash-scene power rankings. Features:

- Project ownership and collaboration (members, invite links)
- Public project sharing — guests can view published projects without an account
- Per-user start.gg API keys
- Tournament import from start.gg with per-event filtering
- Upset-factor statistics, head-to-head set records, and ranking views

Out of scope:
- Email verification or password reset
- OAuth / social login
- Multi-player doubles events

## Use Cases

1. As a user I go on the website and create an account with a username and password.
2. I create a ranking project.
3. I insert/select a list of relevant, eligible players, together with their start.gg accounts (a player may have zero or more start.gg accounts).
4. I select a video game that start.gg supports.
5. The server queries all start.gg tournaments that any of those players have entered. I am shown a list of those tournaments and their relevant events, and can manually deselect any I do not want to count.
6. I get an overview of that list of players: each player's individual wins and losses as separate lists (each sorted by upset factor), with the player list ordered by aggregate upset factor. I also get a head-to-head table of set records between each player.

7. As a project owner I can invite collaborators by email. Collaborators can manage
   players, trigger imports, and adjust settings, but cannot delete the project or
   transfer ownership.

8. I can mark the project as published so that anyone with the link can view the
   stats, head-to-head, ranking, and tournament pages without creating an account.

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

### Backend

| Suite | Command | What it covers |
|---|---|---|
| `cargo test -p common` | No DB needed | Unit tests for pure logic; wiremock-based tests for `StartggClient` operations |
| `cargo test -p api` | Needs `DATABASE_URL` | Integration tests: real isolated DB per test (`#[sqlx::test]`), wiremock for start.gg calls |
| `cargo test -p e2e` | Needs `DATABASE_URL` | End-to-end: full register→import→stats pipeline through the real Axum router and `worker::import::run` |

Key design decisions:

- **`StartggClient::new_with_base_url`** — any code that calls start.gg goes through `StartggClient`, never inline `reqwest`. Tests construct the client with a wiremock URL, so no real network calls are made.
- **No DB mocks** — `#[sqlx::test]` spins up a fresh schema per test. Mocking sqlx would add complexity and miss schema mismatches that compile-time query checking doesn't catch (e.g. constraint violations, NULL handling).
- **Self-contained tests** — each test registers its own users, creates its own data. No shared fixtures, no ordering dependencies.

### Frontend (run from `web/`)

| Suite | Command | What it covers |
|---|---|---|
| Vitest unit + component | `npm run test:unit` | `makeApi` factory logic; H2H grid rendering; Stats table rendering and expand/collapse |
| Playwright E2E | `npm run test:e2e` | Auth pages, unauthenticated redirect, project/H2H/stats pages against a mock API |

Key design decisions:

- **Vitest uses `svelte()` (not `sveltekit()`) with `conditions: ['browser']`** — the `sveltekit()` Vite plugin resolves Svelte to its SSR build; the plain `svelte()` plugin with browser conditions resolves to the client build, so `mount()` is available in jsdom.
- **Playwright starts two `webServer` processes** — a lightweight Node.js mock API (`tests/mock-api.js`) on port 9999, and the SvelteKit dev server on port 5174 with `INTERNAL_API_URL=http://localhost:9999`. Both are started automatically; no manual setup needed.
- **Mock API uses a test cookie** — `tests/mock-api.js` returns 200 from `GET /auth/me` only when `session_id=test-session` is present, enabling the unauthenticated redirect test to work without a real backend. Authenticated E2E tests pre-set this cookie via `page.context().addCookies()`.
- **Known limitation: login cookie persistence** — SvelteKit's `event.fetch` does not forward `Set-Cookie` headers from cross-origin API responses to the browser. The login→session→redirect flow therefore cannot be fully E2E tested without modifying the SvelteKit login action to re-set the cookie via `event.cookies.set()`. Workaround: the test verifies that correct credentials don't produce an error, and authenticated page access is covered by the pre-set cookie fixture.

## Frontend

Written in Svelte 5 (runes mode) with shadcn-svelte components and Tailwind CSS v4. Talks to the `api` over REST via a thin `src/lib/api.ts` wrapper that sets `credentials: 'include'` on every request. Built with adapter-node for Docker deployment.

Route protection lives in `src/hooks.server.ts`: calls `GET /auth/me` on every request, redirects to `/login` on 401. Server-side `load` functions use `INTERNAL_API_URL` (Docker-internal); client-side fetches use `PUBLIC_API_URL`.

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

See `openapi.yaml` for the full contract.

| Group             | Endpoints |
|-------------------|-----------|
| Auth              | POST /auth/register, /auth/login, /auth/logout; GET /auth/me |
| Account           | PATCH /account/profile; PATCH /account/password; PUT/DELETE /account/startgg-key; DELETE /account |
| Projects          | GET/POST /projects; GET/PATCH/DELETE /projects/:id (`published` flag toggles guest access) |
| Players           | CRUD on /projects/:id/players |
| Ranking           | PUT /projects/:id/ranking (reorder players) |
| start.gg accounts | POST/DELETE /projects/:id/players/:pid/accounts |
| Import            | POST/GET /projects/:id/import |
| Tournament entrants | GET /projects/:id/tournament-entrants |
| Tournaments       | GET /projects/:id/tournaments |
| Events            | PATCH /projects/:id/events/:eid (toggle included) |
| Stats             | GET /projects/:id/stats; GET /projects/:id/stats/:player_id; GET /projects/:id/head-to-head; GET /projects/:id/head-to-head/:a/:b/sets |
| Members           | GET/POST /projects/:id/members; PATCH/DELETE /projects/:id/members/:uid; POST /projects/:id/members/transfer-ownership |
| Invite links      | GET/POST /projects/:id/invite-links; DELETE /projects/:id/invite-links/:lid; POST /invite/:token/accept |
| Games             | GET /games?q= (proxies start.gg game search) |

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

A positive result means the lower-seeded player performed better than expected (an upset).

`GET /projects/:id/stats` returns each player's wins and losses as separate lists of individual set records, each record carrying its own upset factor. Within each list the sets are sorted by upset factor descending (biggest upsets first). The outer player list is ordered by aggregate upset factor (sum of all the player's wins' upset factors) descending.

## Infrastructure

### URLs

| Role | Public URL |
|---|---|
| Frontend | `https://rankingforge.com` |
| API | `https://api.rankingforge.com` |

The frontend and API are on different subdomains (different origins). The API must configure CORS to allow `https://rankingforge.com` with `allow_credentials: true`. Client-side fetches from the browser must use `credentials: 'include'`. `SameSite=Strict` on the session cookie is safe — both hosts are under the same registrable domain (`rankingforge.com`).

The SvelteKit server has two API base URL env vars: `PUBLIC_API_URL` (sent to the browser, used for client-side fetches) and `INTERNAL_API_URL` (Docker-internal address, used by server-side `load` functions to avoid the public network).

### Docker Compose

```yaml
services:
  db:      # postgres
  api:     # ranking_forge_api binary
  worker:  # ranking_forge_worker binary
```

All three share a Docker network. `api` and `worker` connect to `db` via the `DATABASE_URL` environment variable.
