# Design: Mirror-Backed Rankings

**Date:** 2026-06-25
**Status:** Approved
**Supersedes:** `2026-06-24-mirror-backed-import-design.md`

---

## Overview

Rankings become fully dependent on `global_*` tables populated by the crawler. All project-scoped copies of tournament data are eliminated. No per-user start.gg API key is needed anywhere in the user-facing application.

The crawler (using a platform-level API key) continuously mirrors start.gg data into `global_*` tables. Import jobs and ranking computation become pure Postgres-to-Postgres operations. The `startgg_accounts` table bridges global player identity to project-local players.

```
start.gg API
    ↓  (crawler, platform API key)
global_* tables
    ↓  (import job, pure Postgres)
project_events  ←→  rankings
    ↓  (compute_ranking job)
ranking_set_results / ranking_player_scores
```

All changes are in-place to `backend/migrations/001_initial.sql`. No production database exists.

---

## Schema Changes

### Tables dropped

Project-scoped tournament mirror tables, now redundant:

- `sets`
- `entrants`
- `phase_groups`
- `phases`
- `events`
- `tournaments`

### Columns dropped

- `users.startgg_api_key`
- `ranking_set_results.set_id` and `ranking_set_results.event_id` (replaced with global FKs below)
- `ranking_events.event_id` (replaced with global FK below)

### New table: `project_events`

Records which global events a project has imported — the result of running an import job.

```sql
CREATE TABLE project_events (
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    global_event_id UUID NOT NULL REFERENCES global_events(id),
    PRIMARY KEY (project_id, global_event_id)
);
CREATE INDEX project_events_project_id_idx ON project_events(project_id);
```

### Modified: `ranking_events`

`event_id UUID REFERENCES events(id)` → `global_event_id UUID REFERENCES global_events(id)`.
The `included` boolean and all other columns are unchanged.

### Modified: `ranking_set_results`

- `set_id UUID REFERENCES sets(id)` → `global_set_id UUID REFERENCES global_sets(id)`
- `event_id UUID REFERENCES events(id)` → `global_event_id UUID REFERENCES global_events(id)`

`winner_player_id` and `loser_player_id` remain local player UUIDs — identity is resolved to project players at compute time.

### Enriched: `global_tournaments`

Add columns currently missing from the global mirror:

```sql
profile_image_url  TEXT,
banner_url         TEXT,
venue_name         TEXT,
venue_address      TEXT,
hashtag            TEXT,
short_slug         TEXT
```

### Enriched: `global_players`

Add `banner_url TEXT`. (`profile_image_url` already exists.) The start.gg User type returns an `images` array with a `type` field — `banner_url` is populated from the first non-`profile` image if present, and left NULL otherwise. The COALESCE upsert pattern ensures it is never overwritten by NULL on a re-crawl.

---

## Crawler Changes (`crates/crawler/`)

### `api.rs` — `TOURNAMENT_QUERY`

Add to the tournament node fields: `images { url type }`, `venueName`, `venueAddress`, `hashtag`, `shortSlug`.

### `api_types.rs`

- `TournamentNode` gains: `images: Vec<TournamentImage>`, `venue_name`, `venue_address`, `hashtag`, `short_slug`
- New struct: `TournamentImage { url: Option<String>, image_type: Option<String> }` (mirrors existing `UserImage`)

### `db.rs` — `upsert_tournament`

Pick `profile_image_url` and `banner_url` from the images list by `type` field (same pattern as player images). Upsert all new columns using `COALESCE(EXCLUDED.x, global_tournaments.x)` so richer data is never overwritten by nulls on a re-crawl.

No other crawler changes required — player `profile_image_url` is already handled; `banner_url` is an additive column using the same upsert pattern.

---

## Worker Changes

### Import rewrite (`worker/src/import.rs`)

The `import_tournaments` job becomes a pure Postgres-to-Postgres operation. `StartggClient` and all network calls are removed from the worker.

**Flow:**

1. Resolve linked players: `startgg_accounts JOIN global_players` → get each account's `global_players.id`. If no accounts are linked, exit early.
2. Discover events: `global_event_entries JOIN global_events JOIN global_tournaments`, filtered by the job's `after_date`/`before_date` window. If the project has a `game_id`, join `global_games WHERE startgg_id = projects.game_id` to filter by game.
3. Upsert each discovered event into `project_events(project_id, global_event_id)` — `ON CONFLICT DO NOTHING`.
4. For each existing ranking, upsert `ranking_events(ranking_id, global_event_id, included=true)` — `ON CONFLICT DO NOTHING` so existing user toggles are preserved.
5. Enqueue `compute_ranking` for all project rankings.

`seed_ranking_by_winrate` stays but queries `global_sets` and `global_event_entries` instead of local tables.

### Compute rewrite (`worker/src/compute.rs`)

Both phases rewrite their queries to join through global tables. `startgg_accounts` bridges `global_players` to local project players.

**Phase 1 — set results → `ranking_set_results`:**

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

Upset factor computation is unchanged — inputs are `(winner_seed, loser_seed)`, now sourced from `global_event_entries`.

**Phase 2 — algorithm scores:** Same join pattern as Phase 1, narrowed to `winner_player_id`, `loser_player_id`, `completed_at`.

### New endpoint: `POST /projects/:id/import/:job_id/retrigger`

Reads the specified job's stored `params` JSONB from the `jobs` table and enqueues a new import job with those same params. Available on any job status.

---

## API Changes (`crates/api/src/routes/`)

The API contract (response shapes, route paths) is unchanged. Only backing queries change.

### `tournaments.rs`

- **`list_tournaments`** — joins `ranking_events → global_events → global_tournaments → global_phases`. Bracket types subquery: `SELECT bracket_type FROM global_phases WHERE event_id = ge.id`.
- **`get_stats` / `get_player_stats`** — joins `global_sets → global_players → startgg_accounts → players`. Display names for non-project opponents come from `global_players.handle`. Seeds and placements come from `global_event_entries`.
- **`get_h2h_sets`** — joins `ranking_set_results → global_sets → global_event_entries` for seeds/placements, `global_events → global_tournaments` for metadata.
- **`get_player_tournaments` / `get_ranking_player_tournaments`** — queries `global_event_entries → global_events → global_tournaments`, filtered by `global_players.id` (resolved via `startgg_accounts`) and membership in `project_events` or `ranking_events`.
- **`delete_tournament`** — deletes from `project_events` and `ranking_events` where `global_event_id IN (SELECT id FROM global_events WHERE tournament_id = $1)`.

### `players.rs`

**Account linking** — `link_account` queries `SELECT * FROM global_players WHERE handle ILIKE $1` instead of calling the start.gg API. Returns 404 with a "player not yet indexed" message if not found. `StartggClient` is removed from `AppState`; `startgg_base_url` is removed from `AppState`.

### `import.rs`

Adds `POST /projects/:id/import/:job_id/retrigger` as described above.

### `account.rs`

Remove `PUT /account/api-key` and `DELETE /account/api-key` entirely — the `startgg_api_key` column no longer exists.

### `games.rs`

`GET /games/search` currently uses `user.startgg_api_key` to call `StartggClient.search_games()`. Rewrite to query `SELECT * FROM global_games WHERE name ILIKE $1 LIMIT 20` — the crawler already populates this table. No API key or `StartggClient` needed.

### `projects.rs`

Removes the API key gate from project creation.

---

## Frontend Changes (`web/`)

- **API key UI removed** — account settings, project creation flow, any API-key error messages or gates
- **Re-run button** — added to each import job card (any status), calls `POST /projects/:id/import/:job_id/retrigger`
- **Image display** — `profile_image_url` and `banner_url` are now available on tournament and player data via API responses; actual use in UI components is separate scope — this spec ensures the data is surfaced

---

## Out of Scope

- Live fallback to start.gg API for data not yet in the mirror — accepted gap, no fallback
- Automatic import triggering on player link or crawler window completion — deferred
- Global rating computation using `global_player_ratings` — separate phase
- Rendering images in the existing UI — data availability is in scope, UI use is not
