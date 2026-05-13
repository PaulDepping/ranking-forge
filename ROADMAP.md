# Implementation Roadmap

## Current state

**Phase 1 complete** (2026-05-09). Next: Phase 2 (Core CRUD API).

| Phase | Status |
|---|---|
| 1a — AppState, router, CORS | ✅ Done |
| 1b — Auth endpoints + AuthUser extractor | ✅ Done |
| 2 — Core CRUD API | ✅ Done |
| 3 — start.gg GraphQL client | ✅ Done |
| 4 — Import worker | ✅ Done |
| 5 — Tournament deselection + stats | ✅ Done |
| 6 — Frontend | ✅ Done |

**Phase 6 complete** (2026-05-13). All phases done.

What exists and compiles:
- `backend/` — Rust workspace with working `api` and `worker` binaries
- `web/` — SvelteKit 5 + TypeScript frontend, fully implemented
- `backend/migrations/001_initial.sql` — full schema (run via `sqlx::migrate!` at startup)
- `backend/openapi.yaml` — full REST API contract
- `backend/.sqlx/` — offline query cache (committed; required for `SQLX_OFFLINE=true` builds)
- `backend/crates/common/src/startgg/` — `StartggClient` with all 5 GraphQL operations; `AppState` uses it instead of raw reqwest fields
- `backend/crates/e2e/` — end-to-end regression test crate; `tests/full_flow.rs` runs the full register→import→stats pipeline against a wiremock start.gg
- `docker-compose.yml` — all services defined; `initdb.d` mount removed (sqlx is the migration runner)

Implementation notes from Phase 1:
- `CORS_ORIGIN` is a config field (env var, default prod URL) — set to `http://localhost:5173` in `.env` for local dev
- Axum 0.8 `FromRequestParts` uses native `async fn` — `#[async_trait]` must not be used
- `cookie` must be a direct dep to access `cookie::time::Duration` for `max_age`

---

## Design Decisions

### start.gg API Key

Use a single shared API key stored server-side in `.env` (`STARTGG_API_KEY`). Both `api` and `worker` read it at startup. Users never supply their own key.

**Rationale:** This is a private tool for a small, known user base. A shared key simplifies onboarding — users never touch API credentials. start.gg's GraphQL API has a rate limit of ~80 requests/minute per token; for a small user base this is not a bottleneck. If the user base grows, per-user keys can be added later (a `startgg_api_key` column on `users`, set via a settings endpoint).

**In practice:** The worker must still be courteous — add a small delay (e.g. `tokio::time::sleep(Duration::from_millis(200))`) between paginated requests to avoid bursting the rate limit during large imports.

### URLs

| Role | URL |
|---|---|
| Frontend | `https://rankingforge.example.com` |
| API | `https://api.rankingforge.example.com` |

These are different origins (different subdomains), so the API must configure CORS and client-side fetches must opt in to sending credentials.

**API CORS (`CorsLayer` from `tower-http`)** — configured in Phase 1a alongside the router:
```rust
CorsLayer::new()
    .allow_origin("https://rankingforge.example.com".parse::<HeaderValue>().unwrap())
    .allow_credentials(true)
    .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
    .allow_headers([header::CONTENT_TYPE])
```
`allow_credentials(true)` is required for the session cookie to be sent cross-origin. Wildcard `allow_origin("*")` is incompatible with credentials and must not be used.

**Frontend env vars:**
- `PUBLIC_API_URL=https://api.rankingforge.example.com` — used by client-side fetches (exposed to the browser).
- `INTERNAL_API_URL=http://api:8080` — used by server-side `load` functions to hit the API directly over the Docker network, bypassing the public internet.

**Client-side fetch** — all browser-side calls to the API must include `credentials: 'include'`; otherwise the browser will not send the session cookie cross-origin. Centralise this in a thin `src/lib/api.ts` wrapper so it is never forgotten.

`SameSite=Strict` on the session cookie is fine — both hosts share the same registrable domain (`example.com`), so they are considered same-site by the browser.

### User Authentication

**Server side (Axum):**

- Passwords are hashed with `argon2` (via the `argon2` crate) using a random per-password salt.
- On login/register, a new `sessions` row is created and its UUID is set as a cookie:
  ```
  Set-Cookie: session_id=<uuid>; HttpOnly; SameSite=Strict; Path=/; Max-Age=2592000
  ```
  Add `Secure` in production (HTTPS); omit it locally.
- Every protected Axum handler uses an `AuthUser` extractor (`impl FromRequestParts`). It reads the `session_id` cookie, queries the `sessions` table (joining `users`), rejects expired rows, and returns the `User` or `401 Unauthorized`.
- Add `GET /auth/me` → returns `{id, username}` for the active session, or 401. The frontend uses this to bootstrap auth state.
- Logout (`POST /auth/logout`) deletes the session row and sends `Set-Cookie: session_id=; Max-Age=0` to clear the browser cookie.

**Frontend side (SvelteKit):**

- All server-side `load` functions use `event.fetch` (not `globalThis.fetch`). SvelteKit's `event.fetch` automatically forwards the incoming browser cookie to the API, so SSR requests are authenticated without any extra work.
- Client-side `fetch` must use `credentials: 'include'` — the API is a different origin, so cookies are not sent automatically. Use the `src/lib/api.ts` wrapper for all API calls.
- Route protection lives in `src/hooks.server.ts`: on every navigation it calls `GET /auth/me` via `event.fetch` and redirects to `/login` on 401. Exempt `/login` and `/register`.
- Login flow: POST to `/auth/login` → API sets cookie → redirect to `/`.
- Logout flow: POST to `/auth/logout` → redirect to `/login`.
- Server-side `load` functions use `INTERNAL_API_URL` (Docker-internal address); client-side fetches use `PUBLIC_API_URL`. See the URLs section above.

---

## Phase 1 — Backend foundation

Everything else depends on this.

### 1a. AppState + error handling (`crates/api`)

- `AppState`: `PgPool`, `reqwest::Client` (for start.gg calls), `String` (session cookie secret)
- `AppError` enum that implements `IntoResponse` — maps variants to HTTP status codes
- Wire Axum router with `AppState` as shared state
- Apply `CorsLayer` to the router (see URLs section): allow origin `https://rankingforge.example.com`, allow credentials, explicit methods and headers

### 1b. Auth (`crates/api`)

Implement the three auth endpoints using the `sessions` and `users` tables:

- `POST /auth/register` — hash password with `argon2`, insert user, create session, set `HttpOnly` cookie
- `POST /auth/login` — verify password, create session, set cookie
- `POST /auth/logout` — delete session row, clear cookie

Write a session extractor (`FromRequestParts`) that reads the cookie, looks up the session, and returns the authenticated `User`. All protected routes use this extractor.

### Verify

`docker compose up db -d`, then `cargo run --bin api` — register and login with `curl`, confirm session cookie is set.

---

## Phase 2 — Core CRUD API (`crates/api`)

With auth working, implement the remaining synchronous endpoints.

| Endpoint group | Notes |
|---|---|
| `GET/POST /projects`, `GET/DELETE /projects/:id` | Standard CRUD against `ranking_projects` |
| `GET/POST /projects/:id/players`, `DELETE .../players/:pid` | Scoped to project; enforce ownership |
| `POST/DELETE .../players/:pid/accounts` | Needs start.gg slug → user ID resolution (see Phase 3) |
| `GET /games?q=` | Proxy to start.gg game search |

Add sqlx model structs to `crates/common/src/models/` alongside each query. Run `cargo sqlx prepare` after adding each `sqlx::query!` block to update the offline cache (`.sqlx/`).

### Verify

Full CRUD round-trip via `curl` or a REST client against the running API.

---

## Phase 3 — start.gg GraphQL client (`crates/common`)

Both `api` and `worker` call start.gg, so the client lives in `common`.

Structure:
```
crates/common/src/startgg/
├── mod.rs          — client constructor (reqwest + bearer token)
├── queries.rs      — typed query/response structs (serde)
└── operations.rs   — one function per query
```

Queries to implement (in order of need):

1. **Game search** — `videogames(query: {name: $q})` → needed by `GET /games`
2. **User by slug** — `user(slug: $slug)` → needed by account linking
3. **Tournaments by user** — paginated, filtered by game ID → core of the import job
4. **Event entrants** — seed + user ID per entrant in an event
5. **Event sets** — winner, loser, round, scores, completedAt, vodUrl, isDQ

Look up the start.gg GraphQL schema / explorer before writing each query to confirm field names and pagination patterns (cursor-based).

### Verify

Write a short `#[tokio::test]` or throwaway `main` that calls game search and prints results.

---

## Phase 4 — Import worker (`crates/worker`)

The heaviest piece. Implement the full import pipeline.

### 4a. Job queue

In `crates/common/src/jobs.rs`:
- `enqueue(pool, project_id)` — inserts a pending job, calls `pg_notify`
- `claim(pool)` — `SELECT ... FOR UPDATE SKIP LOCKED LIMIT 1` on pending jobs
- `mark_running / mark_done / mark_failed` — status updates

In `crates/api` import handler:
- `POST /projects/:id/import` — calls `enqueue`, returns 202
- `GET /projects/:id/import` — returns latest job status for the project

### 4b. Worker main loop

```
PgListener::connect → listen("jobs")
loop:
  recv notification OR poll every 30s
  while let Some(job) = claim(pool):
    tokio::spawn(process_job(pool, startgg_client, job))
```

### 4c. Import job

For a given `project_id`:

1. Load all players + their `startgg_accounts` for the project
2. For each start.gg user ID, query all tournaments they've entered (filtered by project's `game_id`)
3. Upsert tournaments and events into DB; insert rows into `project_events` (included = true) for any new events
4. For each event, upsert all entrants; match `startgg_user_id` against `startgg_accounts` to set `player_id`
5. Fetch all sets for the event; upsert into `sets`
6. Back-fill `final_placement` on entrants from standing data

Handle pagination carefully — start.gg returns 25 items per page by default.

### Verify

Run a real import against a known player's start.gg account (use the key from `.env`). Inspect the DB with `psql` to confirm rows landed correctly.

---

## Phase 5 — Tournament deselection + stats (`crates/api`)

### 5a. Tournament/event listing

`GET /projects/:id/tournaments` — join tournaments → events → project_events, return nested JSON with `included` flag.

`PATCH /projects/:id/events/:eid` — upsert into `project_events`.

### 5b. Upset factor calculation

Implement the smash-community seed → losers-round conversion tables in `crates/common/src/upset.rs`.

The pipeline:

1. Select all sets from included events where both entrants have a non-null `player_id` and `is_dq = false`
2. For each set: look up `winner.seed` and `loser.seed` → compute each player's projected losers round → subtract to get per-set upset factor
3. Build per-player win and loss lists, each as individual `SetRecord` objects `{ opponent_id, opponent_name, upset_factor }`
4. Sort each player's win/loss lists by upset factor descending
5. Sort the outer player list by aggregate upset factor (sum of wins' upset factors) descending

`GET /projects/:id/stats` — returns the player list, each entry with `wins: [SetRecord]` and `losses: [SetRecord]`.

`GET /projects/:id/head-to-head` — for each (player_a, player_b) pair, count sets where player_a won vs. lost. A single aggregation query over the same filtered sets.

### Verify

After a real import, hit the stats and head-to-head endpoints and confirm the numbers match manually counted results for a small tournament.

---

## Phase 6 — Frontend (`web/`)

Install shadcn/ui for Svelte (`npx shadcn-svelte@latest init`) before starting UI work.

Build pages in this order (each depends on the previous API phase being done):

| Page | Route | Needs |
|---|---|---|
| Login / Register | `/login`, `/register` | Phase 1 |
| Projects list | `/` | Phase 2 |
| New project (with game search) | `/projects/new` | Phase 3 |
| Project detail / players | `/projects/[id]` | Phase 2 |
| Add player + link accounts | `/projects/[id]/players` | Phase 3 |
| Import + status poll | `/projects/[id]/import` | Phase 4 |
| Tournament deselection | `/projects/[id]/tournaments` | Phase 5a |
| Stats overview | `/projects/[id]/stats` | Phase 5b |
| Head-to-head table | `/projects/[id]/h2h` | Phase 5b |

Use SvelteKit `load` functions for server-side data fetching.

Before writing any page, create `src/lib/api.ts` — a thin wrapper around `fetch` that sets `credentials: 'include'` and prefixes the correct base URL (`PUBLIC_API_URL` client-side, `INTERNAL_API_URL` server-side). Every API call goes through this wrapper.

### Implementation notes from Phase 6

- shadcn-svelte v1.2.7 requires Tailwind CSS v4 (`tailwindcss` + `@tailwindcss/vite`) — install both before running `npx shadcn-svelte@latest init`
- Tailwind v4 uses the Vite plugin (`@tailwindcss/vite`) instead of a PostCSS plugin; add it to `vite.config.ts` before shadcn init
- Use `adapter-node` (not `adapter-auto`) for Docker — produces `build/index.js` run with `node`
- `makeApi(fetch, baseUrl)` factory in `src/lib/api.ts` — callers pass `event.fetch` + `INTERNAL_API_URL` server-side, or `globalThis.fetch` + `PUBLIC_API_URL` client-side
- Svelte 5 warns when `$state(data.foo)` captures a prop value — this is intentional for local mutable copies (tournaments optimistic updates, import job polling); the warning is benign
- Import polling: client-side `setInterval` at 3s, clears itself when status reaches `done` or `failed`
- Tournament toggle: optimistic UI update applied immediately, then synced from the PATCH response
- `src/hooks.server.ts` redirects all non-`/login`/`/register` routes to `/login` on 401 from `GET /auth/me`
- Dev env vars (`web/.env`): `PUBLIC_API_URL=http://localhost:8080`, `INTERNAL_API_URL=http://localhost:8080`

---

## Testing strategy

Write tests alongside each phase, before moving to the next. There are two distinct testing contexts:

### `crates/common` — no database required

Run with `cargo test -p common`.

- **Unit tests** for pure logic (helper methods, data transformations): inline `#[cfg(test)] mod tests` at the bottom of the relevant source file.
- **HTTP client tests** for `StartggClient` operations: use wiremock to stand up a local mock server, then construct the client with `StartggClient::new_with_base_url("test-key", &mock.uri())`. Assert on the returned Rust types, not on raw JSON. Cover happy path, empty/null responses (e.g. user not found), GraphQL-level errors, and HTTP errors.

### `crates/api` — requires a running PostgreSQL server

Run with `DATABASE_URL=postgres://... cargo test -p api`.

- **Integration tests** in `crates/api/tests/api.rs`: use `#[sqlx::test(migrations = "../../migrations")]` which spins up a fresh isolated DB per test.
- Construct the Axum app via `make_app(pool, startgg_base_url)` — the `startgg_base_url` parameter lets tests point at a wiremock mock server for any endpoint that calls start.gg.
- Use `tower::ServiceExt::oneshot` to fire requests and assert on HTTP status codes and JSON bodies.
- Every new route needs at least: happy path, auth-required (no cookie → 401), ownership enforcement (wrong user → 404), and invalid input (→ 422).

### `web/` — frontend

Run from `web/`.

**Unit + component tests** (`npm run test:unit`, no server needed):

- `src/lib/api.test.ts` — `makeApi` factory: verifies `credentials: 'include'` on every method, JSON body/header handling, URL construction, response passthrough.
- `src/routes/projects/[id]/h2h/h2h.test.ts` — H2H grid renders W–L records, empty state, diagonal dashes.
- `src/routes/projects/[id]/stats/stats.test.ts` — Stats table renders player order, aggregate UF, win/loss counts, expand/collapse.

Uses Vitest with the `svelte()` Vite plugin (not `sveltekit()`) and `resolve.conditions: ['browser']`. This is required because the `sveltekit()` plugin resolves Svelte to its SSR build; the plain `svelte()` plugin with browser conditions resolves to the client build where `mount()` is available.

**E2E tests** (`npm run test:e2e`, Playwright):

- `tests/auth.test.ts` — login form renders, error on bad credentials, valid credentials accepted, unauthenticated redirect to `/login`, register form renders.
- `tests/projects.test.ts` — projects list, tab navigation, H2H grid, stats page, import page, tournaments empty state; all using a pre-authenticated cookie fixture.

Playwright starts two servers automatically via `webServer`:
1. `tests/mock-api.js` — a Node.js HTTP mock server on port 9999 with all API routes stubbed. Returns 200 from `GET /auth/me` only when `session_id=test-session` is present (enabling the unauthenticated redirect test to work without a real backend).
2. SvelteKit dev server on port 5174, started with `INTERNAL_API_URL=http://localhost:9999` and `PUBLIC_API_URL=http://localhost:9999` so all server-side fetches hit the mock.

Authenticated E2E tests pre-set the `session_id=test-session` cookie via `page.context().addCookies()` — this is forwarded by SvelteKit's `event.fetch` to the mock API, so the auth guard passes.

**Known limitation:** SvelteKit's `event.fetch` does not forward `Set-Cookie` headers from cross-origin API responses to the browser. The full login→cookie→redirect flow therefore requires the SvelteKit login action to re-set the cookie explicitly via `event.cookies.set()` rather than relying on header forwarding. The test instead verifies correct credentials produce no error; authenticated page access is covered by the cookie fixture.

### General rules

- `new_with_base_url` on `StartggClient` exists specifically for testing — any code that calls start.gg must go through the client (never inline `reqwest` calls in routes) so the mock URL can be injected.
- Do not mock the database. `#[sqlx::test]` gives you a real isolated schema with no overhead; mocking sqlx adds complexity and hides schema mismatches.
- Keep each test self-contained: register its own users, create its own projects. Shared fixtures cause test ordering dependencies.

---

## Guiding principles throughout

- Add sqlx model structs only when writing the first query that uses them
- Run `cargo sqlx prepare` (from `backend/`) after every new `sqlx::query!` block
- Use `cargo add` (never edit versions manually) when adding new crates
- Look up start.gg GraphQL field names in the explorer before writing queries — the schema has quirks
- Test each phase against a real DB before moving to the next
