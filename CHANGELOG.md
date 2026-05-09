# Changelog

Tracks structural decisions and work completed in each Claude session.
Intended to give future agents fast context without reading every file.

---

## Session 1 — 2026-05-08/09

### What was done

**Design review and architecture decisions**

- Reviewed and substantially expanded `DESIGN.md` with: architecture section
  (three-process design, Postgres job queue, REST API), data model entity list,
  API overview table, auth mechanism, upset factor seed clarification, Docker
  Compose layout, and out-of-scope list.

**Key decisions made (with rationale)**

| Decision | Rationale |
|---|---|
| Separate `api` and `worker` binaries | Independent scaling in Docker Compose / Kubernetes |
| Postgres job queue (SKIP LOCKED + LISTEN/NOTIFY) | Zero extra infra; scales to N workers without coordination layer |
| Cookie-based sessions (not JWT) | Simpler for POC; session data stays server-side in `sessions` table |
| `argon2` for password hashing | More modern than bcrypt; winner of the Password Hashing Competition |
| `reqwest` with `rustls` (no OpenSSL) | Cleaner Docker images; no system SSL dep |
| Seeds from start.gg entry seed | No manual override; fetch from API |
| REST API (not GraphQL) | Simpler with Axum; straightforward OpenAPI docs |

**Artifacts created**

- `backend/migrations/001_initial.sql` — full schema (all tables in one migration)
- `backend/openapi.yaml` — full OpenAPI 3.1 spec for all 18 endpoints
- `docker-compose.yml` — four services: `db`, `api`, `worker`, `web`
- `Dockerfile` — multi-stage build; `builder → api` and `builder → worker` targets
- `.env` — updated to match docker-compose credentials (`rankingforge`), added `SQLX_OFFLINE=true`
- `.gitignore` — `backend/target/`, `web/node_modules/`

**Workspace scaffolded**

```
ranking_forge/
├── backend/                  Rust workspace (Cargo.toml)
│   ├── crates/
│   │   ├── common/           lib: DB pool, error types, model stubs, (future) start.gg client
│   │   ├── api/              bin: Axum HTTP server stub
│   │   └── worker/           bin: background worker stub
│   ├── migrations/           SQL (moved here from db/)
│   └── openapi.yaml          API contract (moved here from api/)
└── web/                      SvelteKit + TypeScript (minimal template)
```

`cargo build` passes for the full workspace with `SQLX_OFFLINE=true`.

**start.gg API research**

Checked the start.gg GraphQL schema. Fields stored beyond the basics:
- Tournaments: `end_at`, `venue_address`, `addr_state`, `timezone`, `num_attendees`, `online`
- Events: `start_at` (on the event, not just the tournament — for time-range filtering)
- Entrants: `is_disqualified`, `final_placement`
- Sets: `round_name`, `best_of`, `winner_score`, `loser_score`, `is_dq`, `vod_url`, `completed_at`

Note: `is_dq` on sets and `is_disqualified` on entrants are distinct — a set
can be a DQ without the entrant being globally DQ'd from the event.

**Crate note**: reqwest v0.13 renamed the TLS feature from `rustls-tls` (v0.12)
to `rustls`. Use `--features rustls` not `--features rustls-tls`.

### What comes next

See `DESIGN.md` for the full roadmap. Implementation order:

1. **Phase 1** — AppState, auth routes, session extractor
2. **Phase 2** — Projects/Players CRUD
3. **Phase 3** — start.gg GraphQL client in `crates/common`
4. **Phase 4** — Import worker (job queue, tournament/entrant/set ingestion)
5. **Phase 5** — Tournament deselection + upset factor stats
6. **Phase 6** — Frontend (SvelteKit + shadcn/ui)
