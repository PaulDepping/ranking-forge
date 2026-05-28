---
title: Documentation Improvement
date: 2026-05-28
status: approved
---

# Documentation Improvement

## Goal

Bring project documentation up to date and fill gaps so that future AI agents and human contributors can understand the current architecture and coding decisions without prior context.

## Audience

Both AI agents (dropped into the repo cold) and human contributors (new or returning). Documentation should be precise enough for an agent to make correct decisions and readable enough for a developer to orient quickly.

## What exists today

| File | Purpose | Status |
|---|---|---|
| `DESIGN.md` | High-level architecture, data model, API overview, upset factor | Stale — describes POC, marks shipped features as out-of-scope |
| `CLAUDE.md` | Developer onboarding, commands, patterns, conventions | Accurate |
| `web/CLAUDE.md` | Frontend-specific guidance | Accurate |
| `docs/startgg/project-notes.md` | start.gg API quirks and rate limits | Accurate |
| `docs/superpowers/specs/` | Per-feature design docs | Historical, not architectural |
| `docs/superpowers/plans/` | Per-feature implementation plans | Historical, not architectural |

## What this delivers

### 1. Move and update `DESIGN.md` → `docs/DESIGN.md`

Move `DESIGN.md` to `docs/DESIGN.md` to consolidate all architecture reference material in one place. Update `CLAUDE.md` to reflect the new path.

Fix three stale areas:

- **Scope section** — remove "initial POC" framing and the "out of scope" list. Reframe to describe what the platform is today: a multi-user platform with project ownership, collaboration (members/invite links), public project sharing (guest access), per-player start.gg API keys, and upset-factor statistics.
- **Use Cases** — extend to cover the two new actors: *collaborators* (members who can manage a shared project) and *guests* (unauthenticated visitors viewing published projects).
- **API Overview table** — add missing endpoints: `GET /projects/:id/ranking`, members endpoints, invite link endpoints, account management endpoints, and note the `published` flag on projects.

No structural changes to DESIGN.md — accuracy fixes only.

### 2. `docs/routes.md` — Frontend route map

Documents every SvelteKit route: path, access control, server-side data loaded, and purpose. Opens with a short explanation of the two guard layers:

1. `hooks.server.ts` — calls `GET /auth/me` on every request; redirects unauthenticated users to `/login` for non-public routes.
2. `(editor)` group layout — checks project membership; blocks non-members from editor routes.

Route table:

| Path | Access | Purpose |
|---|---|---|
| `/` | Public | Landing page |
| `/login` | Public (redirects if authed) | Login form |
| `/register` | Public (redirects if authed) | Registration form |
| `/logout` | Authenticated | Clears session cookie |
| `/account` | Authenticated | Manage username, password, start.gg API key |
| `/invite/[token]` | Public | Accept a collaboration invite |
| `/projects` | Authenticated | List owned and member projects |
| `/projects/new` | Authenticated | Create a new project |
| `/projects/[id]` | Owner/member (published: guest) | Project root — redirects to stats |
| `/projects/[id]/stats` | Owner/member (published: guest) | Per-player win/loss lists, upset factor |
| `/projects/[id]/h2h` | Owner/member (published: guest) | Head-to-head set record matrix |
| `/projects/[id]/ranking` | Owner/member (published: guest) | Ordered ranking view |
| `/projects/[id]/tournaments` | Owner/member (published: guest) | Tournament list with include/exclude toggles |
| `/projects/[id]/settings` | Owner/member | Project name, game, published flag, members |
| `/projects/[id]/(editor)/import` | Owner/member | Trigger start.gg import, view job status |
| `/projects/[id]/(editor)/players` | Owner/member | Add/remove/link players |
| `/projects/[id]/(editor)/players/[player_id]` | Owner/member | Edit one player's name and start.gg accounts |

### 3. `docs/modules.md` — Backend module map

Documents each Rust crate and its key source files. One section per crate with a short purpose statement and a file table.

**`common`** (shared library — no binary)

| File | Owns |
|---|---|
| `models/mod.rs` | Rust structs mirroring DB tables (`Project`, `Player`, `Set`, etc.) |
| `jobs.rs` | Job queue: `enqueue`, `claim`, `mark_running`, `mark_done`, `mark_failed` |
| `db.rs` | `PgPool` construction and migration runner |
| `upset.rs` | Pure upset-factor calculation logic |
| `startgg/mod.rs` | `StartggClient` — the only permitted way to call start.gg |
| `startgg/operations/` | Typed GraphQL operation structs and response parsing |
| `startgg/queries.rs` | Raw GraphQL query strings |
| `error.rs` | Shared error types |

**`api`** (Axum HTTP server binary)

| File | Owns |
|---|---|
| `state.rs` | `AppState` — holds `PgPool`, `StartggClient`, session secret, CORS origin |
| `extractors.rs` | `AuthUser` — reads session cookie, queries DB, returns user or 401 |
| `routes/` | One file per route group (`auth`, `projects`, `players`, `import`, `tournaments`, `members`, `invite_links`, `account`, `games`, `health`) |
| `error.rs` | `ApiError` — converts `common::Error` and DB errors to HTTP responses |
| `config.rs` | Reads env vars into a typed config struct at startup |

**`worker`** (background import binary)

| File | Owns |
|---|---|
| `import.rs` | Core import logic: fetch from start.gg, write tournaments/events/entrants/sets to DB |
| `lib.rs` | Worker loop: `PgListener` → claim job → call `import::run` → mark done/failed |
| `config.rs` | Worker env config |

**`e2e`** (test-only crate)

Full-pipeline integration tests through the real Axum router and `worker::import::run`. Requires `DATABASE_URL`. Tests cover the full register → create project → import → stats pipeline.

**`topology`** (test-only crate)

Smoke tests against a running deployment (api + db containers). Run in CI after deploy to verify the live stack is healthy.

### 4. `docs/adr/` — Architecture Decision Records

Six ADR files plus a `README.md` explaining the format. Each ADR has four sections: **Context**, **Decision**, **Rationale**, **Consequences**.

| File | Decision captured |
|---|---|
| `README.md` | Format explanation and index |
| `001-postgres-job-queue.md` | Why Postgres NOTIFY/LISTEN for the job queue instead of Redis or a dedicated queue |
| `002-no-db-mocks.md` | Why tests use real isolated schemas (`#[sqlx::test]`) instead of mocking sqlx |
| `003-startgg-client-injection.md` | Why all start.gg calls must go through `StartggClient`, never inline reqwest |
| `004-split-api-url.md` | Why `PUBLIC_API_URL` and `INTERNAL_API_URL` are separate env vars |
| `005-samesite-strict-cookie.md` | Why `SameSite=Strict` is safe for a cross-subdomain frontend/API setup |
| `006-vitest-svelte-plugin.md` | Why Vitest uses `svelte()` + `conditions: ['browser']` instead of `sveltekit()` |

### 5. Update `CLAUDE.md`

Add a "Further reading" section pointing to the new docs:

```
## Further reading

- `docs/DESIGN.md` — architecture, data model, API overview, upset factor algorithm
- `docs/routes.md` — SvelteKit route map with access control
- `docs/modules.md` — backend crate and module map
- `docs/adr/` — architecture decision records (the *why* behind key decisions)
```

Update the existing `DESIGN.md` reference to point to `docs/DESIGN.md`.

## Out of scope

- Updating per-feature spec or plan documents in `docs/superpowers/`
- Adding inline code comments to source files
- API reference documentation beyond what exists in `openapi.yaml`
- Frontend component documentation
