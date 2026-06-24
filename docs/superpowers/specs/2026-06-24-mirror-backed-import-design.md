# Design: Mirror-Backed Import

**Date:** 2026-06-24
**Status:** Approved — awaiting implementation plan

---

## Overview

Replace the existing import flow — which calls the start.gg API live using per-user API keys — with one that queries the `global_*` tables populated by the crawler. The local project-scoped mirror tables (`tournaments`, `events`, `phases`, `phase_groups`, `entrants`, `sets`) are eliminated entirely. Projects reference global tournament data directly rather than copying it.

This also removes the per-user start.gg API key requirement from all user-facing flows.

### Strategic context

The crawler (implemented 2026-06-23) continuously mirrors all start.gg tournament data into `global_*` tables. Once the mirror has sufficient coverage, the per-user API key is no longer needed for imports. This spec implements that transition. Data gaps are accepted while the crawler builds its initial history; there is no live-API fallback.

---

## Schema changes

All changes are made in-place to `backend/migrations/001_initial.sql` (no production database exists).

### Tables dropped

- `tournaments`
- `events`
- `phases`
- `phase_groups`
- `entrants`
- `sets`

### Table added: `project_events`

Records which global events belong to a project — the result of running an import.

```sql
CREATE TABLE project_events (
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    global_event_id UUID NOT NULL REFERENCES global_events(id),
    PRIMARY KEY (project_id, global_event_id)
);
CREATE INDEX project_events_project_id_idx ON project_events(project_id);
```

### Table modified: `ranking_events`

`event_id UUID REFERENCES events(id)` → `global_event_id UUID REFERENCES global_events(id)`. The `included` boolean and all other columns are unchanged.

### Table modified: `ranking_set_results`

`set_id UUID REFERENCES sets(id)` → `global_set_id UUID REFERENCES global_sets(id)`
`event_id UUID REFERENCES events(id)` → `global_event_id UUID REFERENCES global_events(id)`

`winner_player_id` and `loser_player_id` remain as local player UUIDs — identity is already resolved to project players at compute time.

### Column dropped: `users.startgg_api_key`

The per-user start.gg API key field is removed. Any API key gate on project creation is also removed.

---

## Import rewrite (`worker/src/import.rs`)

The import becomes a pure Postgres-to-Postgres operation. No `StartggClient`, no network calls.

### Flow

1. Load the project's linked players: query `startgg_accounts JOIN global_players` to resolve each account's `global_players.id`. If no accounts are linked, exit early.
2. Discover events: query `global_event_entries JOIN global_events JOIN global_tournaments` for all events any linked player attended, filtered by the job's `after_date` / `before_date` window. If the project has a `game_id` (start.gg bigint), join `global_games WHERE startgg_id = projects.game_id` to filter by game.
3. Upsert each discovered event into `project_events(project_id, global_event_id)` — `ON CONFLICT DO NOTHING`.
4. For each existing ranking, upsert `ranking_events(ranking_id, global_event_id, included=true)` — `ON CONFLICT DO NOTHING` so existing user toggles are preserved.
5. Enqueue `compute_ranking` for all project rankings.

`seed_ranking_by_winrate` (initial ranking seed) stays but queries `global_sets` and `global_event_entries` instead of local tables.

`StartggClient` is removed from the worker's dependencies entirely.

### Retrigger

A new endpoint `POST /projects/:id/import/:job_id/retrigger` reads the specified job's stored `params` JSONB from the `jobs` table and enqueues a new import job with those same params. The frontend renders a "Re-run" button on each import job card (any status). No global "last params" concept is needed.

---

## Compute rewrite (`worker/src/compute.rs`)

Both phases rewrite their queries to join through global tables. `startgg_accounts` is the bridge between global player identity and local project players.

### Phase 1: set results → `ranking_set_results`

```sql
SELECT
    gs.id         AS global_set_id,
    saw.player_id AS winner_player_id,
    sal.player_id AS loser_player_id,
    gs.event_id   AS global_event_id,
    wee.seed      AS winner_seed,
    lee.seed      AS loser_seed,
    gs.completed_at
FROM global_sets gs
JOIN global_players gwp ON gwp.id = gs.winner_player_id
JOIN global_players glp ON glp.id = gs.loser_player_id
JOIN startgg_accounts saw ON saw.startgg_user_id = gwp.startgg_user_id
JOIN startgg_accounts sal ON sal.startgg_user_id = glp.startgg_user_id
JOIN ranking_players rwp ON rwp.player_id = saw.player_id AND rwp.ranking_id = $1
JOIN ranking_players rlp ON rlp.player_id = sal.player_id AND rlp.ranking_id = $1
JOIN ranking_events re ON re.global_event_id = gs.event_id AND re.ranking_id = $1
LEFT JOIN global_event_entries wee
       ON wee.event_id = gs.event_id AND wee.player_id = gwp.id
LEFT JOIN global_event_entries lee
       ON lee.event_id = gs.event_id AND lee.player_id = glp.id
WHERE re.included = true
  AND gs.is_dq    = false
ORDER BY gs.completed_at ASC NULLS LAST
```

Upset factor computation is unchanged — inputs are still `(winner_seed, loser_seed)`, now sourced from `global_event_entries`.

### Phase 2: algorithm scores

Same join pattern as Phase 1, narrowed to `winner_player_id`, `loser_player_id`, `completed_at`. Logic is otherwise identical.

---

## API rewrites (`crates/api/src/routes/`)

The API contract (response shapes, route paths) is unchanged. Only the backing queries change.

### `tournaments.rs`

**`list_tournaments`** — joins `ranking_events → global_events → global_tournaments → global_phases`. `bracket_types` subquery: `SELECT bracket_type FROM global_phases WHERE event_id = ge.id`.

**`get_stats` / `get_player_stats`** — joins `global_sets → global_players → startgg_accounts → players` for both winner and loser sides. Display names for non-project opponents come from `global_players.handle` (previously `entrants.display_name`). Seeds and placements come from `global_event_entries`.

**`get_h2h_sets`** — joins `ranking_set_results → global_sets → global_event_entries` for seeds/placements, `global_events → global_tournaments` for metadata. Player IDs already come from `ranking_set_results`.

**`get_player_tournaments` / `get_ranking_player_tournaments`** — queries `global_event_entries → global_events → global_tournaments` filtering by `global_players.id` (resolved via `startgg_accounts`) and membership in `project_events` or `ranking_events`.

**`delete_tournament`** — deletes from `project_events` and `ranking_events` where `global_event_id IN (SELECT id FROM global_events WHERE tournament_id = $1)`.

### `players.rs`

**Account linking** — the `link_account` handler currently calls `StartggClient` to resolve a handle to a user ID. This changes to query `SELECT * FROM global_players WHERE handle ILIKE $1`. If the player is not yet in the global mirror, the API returns 404 with a message indicating the player may not be indexed yet. The platform-level API key is no longer needed in `AppState`; `startgg_base_url` is removed from `AppState`.

### `import.rs`

Adds `POST /projects/:id/import/:job_id/retrigger` as described above.

### `projects.rs`

Removes the API key gate from project creation.

---

## Frontend changes (`web/`)

**Retrigger button** — the import history UI already shows each job with its status. A "Re-run" button is added to each job card, calling `POST /projects/:id/import/:job_id/retrigger`. Available on any job status.

**API key removal** — any UI that prompts for or displays a start.gg API key is removed: account settings, project creation flow, any API-key-required error messages or gates.

No changes to the tournament/event list, stats views, H2H views, or ranking pages — the API contract is unchanged.

---

## Out of scope

- Live fallback to start.gg API for data not yet in the mirror — accepted gap, no fallback
- Automatic import triggering (on player link or crawler window completion) — deferred
- Global rating computation using `global_player_ratings` — separate phase
- Handling players absent from the global mirror in account linking (beyond the 404 response)
