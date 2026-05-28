# Documentation Improvement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring project documentation up to date and fill gaps — stale DESIGN.md, missing route/module maps, missing ADRs, missing inline docs for the upset-factor algorithm, and a standing maintenance rule in CLAUDE.md.

**Architecture:** Pure documentation changes. No code logic changes, no new dependencies. Seven independent tasks that each produce a committed, self-contained doc change.

**Tech Stack:** Markdown, Rust `//!` module doc syntax

---

## File map

| Action | Path |
|---|---|
| Move + edit | `DESIGN.md` → `docs/DESIGN.md` |
| Create | `docs/routes.md` |
| Create | `docs/modules.md` |
| Create | `docs/adr/README.md` |
| Create | `docs/adr/001-postgres-job-queue.md` |
| Create | `docs/adr/002-no-db-mocks.md` |
| Create | `docs/adr/003-startgg-client-injection.md` |
| Create | `docs/adr/004-split-api-url.md` |
| Create | `docs/adr/005-samesite-strict-cookie.md` |
| Create | `docs/adr/006-vitest-svelte-plugin.md` |
| Edit | `backend/crates/common/src/upset.rs` |
| Edit | `CLAUDE.md` |

---

### Task 1: Move DESIGN.md → docs/DESIGN.md and fix stale content

**Files:**
- Move + edit: `DESIGN.md` → `docs/DESIGN.md`

- [ ] **Step 1: Move the file**

```bash
git mv DESIGN.md docs/DESIGN.md
```

- [ ] **Step 2: Replace the Scope section**

The current `## Scope` section (lines 6–16) describes an "initial POC" and lists features as "out of scope" that are now shipped. Replace it entirely with:

```markdown
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
```

- [ ] **Step 3: Extend the Use Cases section**

Append two new use cases after the existing six (after "I also get a head-to-head table..."):

```markdown
7. As a project owner I can invite collaborators by email. Collaborators can manage
   players, trigger imports, and adjust settings, but cannot delete the project or
   transfer ownership.

8. I can mark the project as published so that anyone with the link can view the
   stats, head-to-head, ranking, and tournament pages without creating an account.
```

- [ ] **Step 4: Update the API Overview table**

Replace the existing `## API Overview` table with:

```markdown
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
```

- [ ] **Step 5: Commit**

```bash
git add docs/DESIGN.md
git commit -m "docs: move DESIGN.md to docs/ and update stale scope, use cases, and API table"
```

---

### Task 2: Create docs/routes.md

**Files:**
- Create: `docs/routes.md`

- [ ] **Step 1: Write the file**

Create `docs/routes.md` with the following content:

```markdown
# SvelteKit Route Map

## Access control layers

Two guards enforce access control across all routes:

1. **`src/hooks.server.ts`** — runs on every server-side request. Calls `GET /auth/me`
   and attaches the result to `event.locals.user`. For all routes except those listed
   as Public below, an unauthenticated response redirects to `/login`.

2. **`(editor)` group layout** (`src/routes/projects/[id]/(editor)/+layout.server.ts`) —
   checks that the current user is a member of the project. Returns 403 for non-members.

"Owner/member" means the route is accessible to the project owner and all project
members (collaborators). "Published: guest" means unauthenticated users can also
access it when the project's `published` flag is `true`.

## Routes

| Path | Access | Purpose |
|---|---|---|
| `/` | Public | Landing page |
| `/login` | Public (redirects if authed) | Login form |
| `/register` | Public (redirects if authed) | Registration form |
| `/logout` | Authenticated | Clears session cookie and redirects to `/login` |
| `/account` | Authenticated | Manage username, password, start.gg API key, delete account |
| `/invite/[token]` | Public | Accept a collaboration invite link |
| `/projects` | Authenticated | List projects owned by or shared with the current user |
| `/projects/new` | Authenticated | Create a new project |
| `/projects/[id]` | Owner/member (published: guest) | Project root — redirects to `/stats` |
| `/projects/[id]/stats` | Owner/member (published: guest) | Per-player win/loss lists sorted by upset factor |
| `/projects/[id]/h2h` | Owner/member (published: guest) | Head-to-head set record matrix |
| `/projects/[id]/ranking` | Owner/member (published: guest) | Players ordered by aggregate upset factor |
| `/projects/[id]/tournaments` | Owner/member (published: guest) | Tournament list with include/exclude toggles |
| `/projects/[id]/settings` | Owner/member | Project name, game, published flag, member management |
| `/projects/[id]/(editor)/import` | Owner/member | Trigger a start.gg import; view current job status |
| `/projects/[id]/(editor)/players` | Owner/member | Add, remove, and link players |
| `/projects/[id]/(editor)/players/[player_id]` | Owner/member | Edit one player's display name and start.gg accounts |
```

- [ ] **Step 2: Commit**

```bash
git add docs/routes.md
git commit -m "docs: add frontend route map with access control"
```

---

### Task 3: Create docs/modules.md

**Files:**
- Create: `docs/modules.md`

- [ ] **Step 1: Write the file**

Create `docs/modules.md` with the following content (the inner code block uses indented style to avoid nesting fences):

```markdown
# Backend Module Map

The backend is a Rust workspace at `backend/`. It contains five crates:

    backend/crates/
      common/    Shared library (no binary) — models, job queue, StartggClient, upset logic
      api/       Axum HTTP server binary
      worker/    Background import worker binary
      e2e/       End-to-end test suite (test-only, requires DATABASE_URL)
      topology/  Deployment smoke tests (test-only, runs against live stack)

---

## `common`

Shared library imported by both `api` and `worker`. Contains everything that is not
specific to either binary.

| File | Owns |
|---|---|
| `src/models/mod.rs` | Rust structs mirroring every DB table (`Project`, `Player`, `Set`, `Job`, etc.) |
| `src/jobs.rs` | Job queue helpers: `enqueue`, `claim`, `mark_running`, `mark_done`, `mark_failed` |
| `src/db.rs` | `PgPool` construction and migration runner (called at binary startup) |
| `src/upset.rs` | Pure upset-factor calculation: `seed_to_projected_round`, `set_upset_factor` |
| `src/startgg/mod.rs` | `StartggClient` — the only permitted way to call start.gg |
| `src/startgg/operations/` | Typed GraphQL operation structs and response deserialization |
| `src/startgg/queries.rs` | Raw GraphQL query strings |
| `src/error.rs` | Shared `Error` and `Result` types |

---

## `api`

Axum HTTP server. Listens for browser requests, manages sessions, enqueues import jobs.

| File | Owns |
|---|---|
| `src/state.rs` | `AppState` — holds `PgPool`, `StartggClient`, `session_secret`, `cors_origin`, `startgg_base_url` |
| `src/extractors.rs` | `AuthUser` extractor — reads `session_id` cookie, queries `sessions` table, returns user or 401 |
| `src/routes/mod.rs` | Top-level router wiring all route groups together |
| `src/routes/auth.rs` | Register, login, logout, `/auth/me` |
| `src/routes/account.rs` | Profile update, password change, start.gg API key, account deletion |
| `src/routes/projects.rs` | Project CRUD; also mounts players, import, tournaments, members, invite-links sub-routers |
| `src/routes/players.rs` | Player CRUD, start.gg account linking, tournament entrant listing |
| `src/routes/import.rs` | Import trigger (rate-limited POST) and status GET |
| `src/routes/tournaments.rs` | Tournament/event listing, event include/exclude toggle, stats, H2H, ranking |
| `src/routes/members.rs` | Collaborator management (add, change role, remove, transfer ownership) |
| `src/routes/invite_links.rs` | Invite link create/list/revoke, invite accept |
| `src/routes/games.rs` | Proxies start.gg game search |
| `src/routes/health.rs` | `GET /health` for Docker health checks |
| `src/error.rs` | `ApiError` / `AppError` — converts internal errors to HTTP responses |
| `src/config.rs` | Reads env vars into a typed config struct at startup |

---

## `worker`

Background process. Listens on the Postgres job queue and runs imports.

| File | Owns |
|---|---|
| `src/import.rs` | Core import logic: paginate start.gg GraphQL, write tournaments/events/entrants/sets to DB |
| `src/lib.rs` | Worker loop: `PgListener` wakeup → `jobs::claim` → `import::run` → mark done/failed |
| `src/config.rs` | Worker env config (DATABASE_URL, STARTGG_API_KEY) |

---

## `e2e`

Test-only crate. Full-pipeline integration tests through the real Axum router and
`worker::import::run`. Requires `DATABASE_URL` (provided by `backend/test.sh`).

| File | Covers |
|---|---|
| `tests/full_flow.rs` | Complete register → create project → import → stats pipeline |
| `tests/import_live.rs` | Live start.gg import tests (skipped unless `STARTGG_API_KEY` is set) |

---

## `topology`

Test-only crate. Smoke tests run against a fully deployed stack (api + db containers).
Used in CI after deploy to verify the live environment is healthy.

| File | Covers |
|---|---|
| `tests/smoke.rs` | Health check, auth round-trip, project creation against live API |
```

- [ ] **Step 2: Commit**

```bash
git add docs/modules.md
git commit -m "docs: add backend module map"
```

---

### Task 4: Create docs/adr/ README and ADRs 001–003

**Files:**
- Create: `docs/adr/README.md`
- Create: `docs/adr/001-postgres-job-queue.md`
- Create: `docs/adr/002-no-db-mocks.md`
- Create: `docs/adr/003-startgg-client-injection.md`

- [ ] **Step 1: Write docs/adr/README.md**

```markdown
# Architecture Decision Records

This directory records non-obvious architectural decisions: what was chosen, why, and
what it means in practice.

Each ADR has four sections:
- **Context** — the situation or problem that prompted the decision
- **Decision** — what was chosen
- **Rationale** — why this option over the alternatives
- **Consequences** — what becomes easier, harder, or constrained as a result

## Index

| # | Decision |
|---|---|
| [001](001-postgres-job-queue.md) | Postgres NOTIFY/LISTEN for the job queue |
| [002](002-no-db-mocks.md) | Real isolated schemas in tests, no DB mocks |
| [003](003-startgg-client-injection.md) | All start.gg calls through `StartggClient` |
| [004](004-split-api-url.md) | `PUBLIC_API_URL` and `INTERNAL_API_URL` as separate env vars |
| [005](005-samesite-strict-cookie.md) | `SameSite=Strict` on the cross-subdomain session cookie |
| [006](006-vitest-svelte-plugin.md) | Vitest uses `svelte()` plugin, not `sveltekit()` |
```

- [ ] **Step 2: Write docs/adr/001-postgres-job-queue.md**

```markdown
# ADR 001: Postgres NOTIFY/LISTEN for the Job Queue

## Context

The API needs to hand off import work to the worker process asynchronously. Options
considered: a dedicated queue service (Redis/Sidekiq, RabbitMQ, SQS) or leveraging
the existing Postgres database.

## Decision

Use a `jobs` table in Postgres. The API inserts a row and sends `NOTIFY jobs`. The
worker listens with `LISTEN jobs` (via sqlx `PgListener`) and claims work with
`SELECT ... FOR UPDATE SKIP LOCKED`.

## Rationale

- **No new infrastructure.** Postgres is already a hard dependency. A separate queue
  service adds a third container, credentials, and an operational surface to monitor.
- **Correct delivery semantics.** `SELECT ... FOR UPDATE SKIP LOCKED` gives at-most-once
  job delivery without a distributed lock.
- **Instant wakeup.** `NOTIFY/LISTEN` wakes the worker immediately — no polling.
- **Horizontal scaling.** Multiple worker containers each independently claim jobs;
  `SKIP LOCKED` prevents double-processing without any coordination layer.
- **Transactional enqueue.** A job can be enqueued inside the same database transaction
  as the change that triggers it, so a rolled-back request never enqueues a phantom job.

## Consequences

- All job state is in Postgres — inspectable and queryable with standard SQL.
- Adding a new job type is a code change only; no queue configuration needed.
- If a worker crashes mid-job, the job stays in `running` state. Worker startup rescans
  for stale running jobs and retries them.
- Throughput is bounded by Postgres NOTIFY rate, which is not a concern at this scale.
```

- [ ] **Step 3: Write docs/adr/002-no-db-mocks.md**

```markdown
# ADR 002: Real Isolated Schemas in Tests, No DB Mocks

## Context

Tests that exercise database code need a strategy for the DB layer. Options: mock sqlx
query traits, use an in-memory SQLite, or use real Postgres via `#[sqlx::test]`.

## Decision

All tests that touch the database use `#[sqlx::test(migrations = "../../migrations")]`,
which spins up a real Postgres schema per test and tears it down on completion.

## Rationale

- **Compile-time query checking is not enough.** sqlx validates column names and types
  at compile time but does not catch constraint violations, NULL edge cases, or
  transaction behaviour. These bugs only surface against a real schema.
- **Mocks hide the failure modes that matter.** A mock that returns what you tell it to
  return can pass every assertion while the real query silently fails on INSERT conflicts
  or unexpected NULLs.
- **`#[sqlx::test]` is low friction.** The macro handles schema setup and teardown per
  test. Tests are fully isolated — no shared state, no ordering dependencies.

## Consequences

- Tests require a Postgres connection. `backend/test.sh` provides one via Docker.
- Running `cargo test -p api` or `cargo test -p e2e` directly requires `DATABASE_URL`.
- Schema changes are immediately visible in tests; no mock layer to synchronise.
- Do not use in-memory SQLite or mock the `PgPool` — it defeats the purpose.
```

- [ ] **Step 4: Write docs/adr/003-startgg-client-injection.md**

```markdown
# ADR 003: All start.gg Calls Through `StartggClient`

## Context

The worker and some API routes need to call the start.gg GraphQL API. The question is
whether route or worker code should use `reqwest` directly, or go through an
abstraction.

## Decision

All start.gg calls must go through `StartggClient` (`common::startgg`). Using `reqwest`
directly in route or worker code is not permitted.

## Rationale

- **Testability.** `StartggClient::new_with_base_url` accepts a URL at construction time.
  Tests pass a `wiremock::MockServer` URL, so no real network calls are made during
  the test suite. Direct `reqwest` usage bypasses this and causes tests to hit the
  real API or fail unpredictably.
- **Single point for auth and error handling.** API key headers, rate-limit retries, and
  GraphQL error mapping live in one place. Bypassing `StartggClient` would scatter this
  logic.

## Consequences

- Adding a new start.gg operation means adding a method or operation to `StartggClient`,
  not writing a one-off `reqwest` call.
- Tests for start.gg behaviour use `wiremock::MockServer` + `StartggClient::new_with_base_url`.
  See existing tests in `crates/common/src/startgg/operations/tests.rs` for examples.
```

- [ ] **Step 5: Commit**

```bash
git add docs/adr/
git commit -m "docs: add ADR directory with README and ADRs 001-003"
```

---

### Task 5: Create ADRs 004–006

**Files:**
- Create: `docs/adr/004-split-api-url.md`
- Create: `docs/adr/005-samesite-strict-cookie.md`
- Create: `docs/adr/006-vitest-svelte-plugin.md`

- [ ] **Step 1: Write docs/adr/004-split-api-url.md**

```markdown
# ADR 004: `PUBLIC_API_URL` and `INTERNAL_API_URL` as Separate Env Vars

## Context

The SvelteKit frontend makes API calls in two contexts: server-side `load` functions
(which run in the Node.js container) and client-side event handlers (which run in the
browser). In production the API is at `https://api.rankingforge.com`, but within the
Docker network the containers can reach each other at `http://api:8080`.

## Decision

Two env vars:

- `PUBLIC_API_URL` — the browser-facing URL, used for client-side fetches
- `INTERNAL_API_URL` — the Docker-internal URL, used for server-side `load` functions

`src/lib/api.ts` selects the correct URL based on whether the code is running in the
browser or on the server.

## Rationale

- **Performance.** Server-side `load` functions can reach the API via the Docker
  internal network, avoiding a round-trip over the public internet and TLS negotiation.
- **Reliability.** Server-side fetches do not depend on external DNS or the public CDN.
- Using a single public URL for both contexts adds unnecessary latency and an external
  dependency in the server-to-server path.

## Consequences

- Both vars must be set in production. `INTERNAL_API_URL` is typically
  `http://api:8080` (the Docker service name and port).
- In local development, both can point to `http://localhost:8080`.
- When adding new API calls in SvelteKit `load` functions, use `INTERNAL_API_URL`.
  In `+page.svelte` client-side code, the API client automatically uses `PUBLIC_API_URL`.
```

- [ ] **Step 2: Write docs/adr/005-samesite-strict-cookie.md**

```markdown
# ADR 005: `SameSite=Strict` on the Cross-Subdomain Session Cookie

## Context

The session cookie is set by `api.rankingforge.com` but must be sent when the browser
makes requests from `rankingforge.com`. This is a cross-origin request (different
hosts), which raises questions about cookie `SameSite` policy.

## Decision

Use `SameSite=Strict` on the session cookie.

## Rationale

`SameSite` is evaluated against the *registrable domain* (eTLD+1), not the full
hostname. Both `rankingforge.com` and `api.rankingforge.com` share the registrable
domain `rankingforge.com`, so they are classified as **same-site** despite being
different origins.

Therefore:

- `SameSite=Strict` allows the cookie to be sent on all requests from `rankingforge.com`
  to `api.rankingforge.com`.
- It provides the strongest CSRF protection: the cookie is never sent from a
  third-party context.
- `SameSite=Lax` or `SameSite=None` would also work but offer weaker protection with
  no benefit for our topology.

## Consequences

- `COOKIE_DOMAIN` must be set to the root domain (`rankingforge.com`) so the cookie is
  scoped to both subdomains, not locked to `api.rankingforge.com` alone.
- Third-party embed scenarios (e.g., an iframe on an unrelated domain) would not
  receive the session cookie — this is intentional and not a use case we support.
- This analysis only holds while both frontend and API share the same registrable
  domain. If they are ever moved to different domains, `SameSite` policy must be
  re-evaluated.
```

- [ ] **Step 3: Write docs/adr/006-vitest-svelte-plugin.md**

```markdown
# ADR 006: Vitest Uses `svelte()` Plugin with `conditions: ['browser']`

## Context

Unit tests for Svelte components need to mount components in jsdom. The app build uses
the `sveltekit()` Vite plugin. When this same plugin is used in Vitest, tests fail
with "mount is not a function".

## Decision

The Vitest configuration uses the plain `svelte()` Vite plugin with
`resolve: { conditions: ['browser'] }` instead of `sveltekit()`.

## Rationale

`sveltekit()` resolves the `svelte` package to its SSR (server-side) entry point.
The SSR build does not export `mount()`, which Vitest tests use to render components
in jsdom. The plain `svelte()` plugin with `conditions: ['browser']` resolves to
the client-side build, making `mount()` available.

## Consequences

- SvelteKit-specific module aliases (`$app/navigation`, `$env/static/public`, etc.)
  are not automatically available in tests. They must be mocked in `src/__mocks__/`.
  Existing mocks live in `src/__mocks__/app-navigation.ts` and `src/__mocks__/env.ts`.
- When adding tests for a component that imports new `$app/` or `$env/` modules, add
  a corresponding mock file in `src/__mocks__/` before writing the test.
- The Vite config uses a conditional: `sveltekit()` when `command === 'build'` or the
  dev server is running; `svelte()` when Vitest is running. See `web/vite.config.ts`.
```

- [ ] **Step 4: Commit**

```bash
git add docs/adr/004-split-api-url.md docs/adr/005-samesite-strict-cookie.md docs/adr/006-vitest-svelte-plugin.md
git commit -m "docs: add ADRs 004-006 (split API URL, SameSite cookie, Vitest plugin)"
```

---

### Task 6: Add module-level doc comment to upset.rs

**Files:**
- Modify: `backend/crates/common/src/upset.rs`

- [ ] **Step 1: Add the module-level doc block**

Prepend the following `//!` block to the top of `backend/crates/common/src/upset.rs`,
before the existing `/// Maps a seed...` function doc:

```rust
//! Upset-factor calculation for double-elimination events.
//!
//! ## Algorithm
//!
//! 1. Convert each entrant's seed to a *projected losers-round number* using
//!    `seed_to_projected_round`. The table maps seed ranges to the losers round
//!    that seed is expected to reach based on standard DE bracket tiers
//!    (1st, 2nd, 3rd/4th, 5th, 7th, 9th, 13th, …).
//!
//! 2. Compute upset factor as:
//!
//!    ```text
//!    upset_factor = loser_projected_round - winner_projected_round
//!    ```
//!
//!    A **positive** value means the winner was seeded worse than expected — an upset.
//!    **Zero** means the seeds were equal. **Negative** means the favourite won.
//!
//! NULL seeds (entrants not linked to a known player) are stored as `0` in the
//! database and passed here as-is. `seed_to_projected_round(0)` returns `0`, so
//! sets involving unlinked entrants produce conservative upset factors.
```

- [ ] **Step 2: Verify the file compiles**

```bash
cd backend && cargo check -p common
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/common/src/upset.rs
git commit -m "docs(common): add module-level doc to upset.rs explaining the algorithm"
```

---

### Task 7: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the DESIGN.md reference in the repository layout block**

Find this line in the `## Repository layout` code block:

```
DESIGN.md         Architecture reference with full data model and API overview
```

Replace it with:

```
docs/             Architecture docs, route map, module map, ADRs
```

- [ ] **Step 2: Add a "Further reading" section**

Add the following section after the `## Frontend` section (the last section in the file):

```markdown
## Further reading

- `docs/DESIGN.md` — architecture, data model, API overview, upset factor algorithm
- `docs/routes.md` — SvelteKit route map with access control
- `docs/modules.md` — backend crate and module map
- `docs/adr/` — architecture decision records (the *why* behind key decisions)
```

- [ ] **Step 3: Add a "Documentation maintenance" section**

Add the following section immediately after "Further reading":

```markdown
## Documentation maintenance

When implementing any feature or making an architectural decision, update the relevant
documentation as part of the same change — do not wait to be asked:

- `docs/DESIGN.md` — if the data model, API surface, or overall architecture changes
- `docs/routes.md` — if a SvelteKit route is added, removed, or its access control changes
- `docs/modules.md` — if a new crate or significant module is added
- `docs/adr/` — if a non-obvious architectural decision is made; add a new numbered ADR
```

- [ ] **Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(claude): add further reading links and documentation maintenance rule"
```
