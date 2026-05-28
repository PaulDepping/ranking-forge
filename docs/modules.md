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
| `src/state.rs` | `AppState` — holds `PgPool`, `cors_origin`, `startgg_base_url` |
| `src/extractors.rs` | `ClientIpExtractor` — reads `X-Forwarded-For` for rate-limiting via `tower_governor` |
| `src/routes/mod.rs` | Top-level router wiring all route groups together |
| `src/routes/auth.rs` | Register, login, logout, `/auth/me`; also defines `AuthUser` and `OptionalAuthUser` extractors |
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
| `src/import.rs` | Core import logic: fetch from start.gg, write tournaments/events/entrants/sets to DB |
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
