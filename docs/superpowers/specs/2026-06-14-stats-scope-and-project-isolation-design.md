# Design: Stats Scope Fix & Tournament Project Isolation

**Date:** 2026-06-14
**Status:** Approved — awaiting implementation plan

---

## Problem

Two related bugs introduced during the ranking-algorithms implementation (Task 8):

**Bug 1 — Stats scope too narrow.** `GET /rankings/:rid/stats` and `GET /rankings/:rid/stats/:pid` read from `ranking_set_results`, which only contains sets where both players are ranking members. Wins and losses against non-ranking opponents are invisible. Example: a player who goes 12-0 at an included tournament but faces only one ranking member shows 1 win, not 12.

**Bug 2 — Cross-project entrant contamination.** `tournaments`, `events`, `phases`, and `phase_groups` are globally deduplicated by `startgg_id`. Two projects importing the same tournament share the same `entrants` rows. The import worker writes a project-scoped `player_id` into `entrant.player_id` on conflict, so whichever project imports last overwrites the previous project's player resolution. The `compute_ranking` worker joins `entrant.player_id` against `ranking_players` to identify set participants — corrupted values cause sets to be silently dropped from `ranking_set_results`, producing wrong scores and ratings for the affected ranking.

---

## Design

### 1. Schema — project-scope the tournament hierarchy

Edit `001_initial.sql` directly (no production database exists).

Add `project_id` to `tournaments` and replace global `UNIQUE(startgg_id)` constraints with composite keys that scope each entity to its parent:

```sql
-- tournaments: add project_id, scope unique constraint
ALTER TABLE tournaments
  ADD COLUMN project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE;
CREATE UNIQUE INDEX tournaments_project_startgg_idx ON tournaments(project_id, startgg_id);
-- drop the old UNIQUE(startgg_id) constraint

-- events: already scoped by tournament_id FK, relax global unique
-- DROP UNIQUE(startgg_id), add:
CREATE UNIQUE INDEX events_tournament_startgg_idx ON events(tournament_id, startgg_id);

-- phases
-- DROP UNIQUE(startgg_id), add:
CREATE UNIQUE INDEX phases_event_startgg_idx ON phases(event_id, startgg_id);

-- phase_groups
-- DROP UNIQUE(startgg_id), add:
CREATE UNIQUE INDEX phase_groups_phase_startgg_idx ON phase_groups(phase_id, startgg_id);

-- sets: UNIQUE(event_id, startgg_set_id) — unchanged
-- entrants: UNIQUE(event_id, startgg_entrant_id) — unchanged
```

`entrants` and `sets` already use composite keys scoped by `event_id`. Once `event_id` is per-project (through `tournament_id → project_id`), they are automatically isolated without any constraint changes.

One-to-many relationships within a project are unaffected: different start.gg entities have different `startgg_id` values, so `UNIQUE(parent_id, startgg_id)` allows multiple children per parent row.

**Data migration:** no automatic upgrade path — a shared tournament row cannot be safely split between projects without knowing original ownership. Existing data must be cleared and imports re-run. Acceptable for pre-production.

### 2. Worker — updated ON CONFLICT clauses

`project_id` is already carried in job params (`jobs.project_id`). Thread it as a parameter to the tournament import function and use it in the upsert:

```sql
-- tournament insert
INSERT INTO tournaments (project_id, startgg_id, name, handle, ...)
VALUES ($1, $2, ...)
ON CONFLICT (project_id, startgg_id) DO UPDATE SET name = EXCLUDED.name, ...

-- events (tournament_id already in scope)
ON CONFLICT (tournament_id, startgg_id) DO UPDATE SET ...

-- phases
ON CONFLICT (event_id, startgg_id) DO UPDATE SET ...

-- phase_groups
ON CONFLICT (phase_id, startgg_id) DO UPDATE SET ...
```

`entrants` and `sets` ON CONFLICT clauses are unchanged.

### 3. Stats endpoints — restore sets-based query

`get_stats` and `get_player_stats` revert to querying `sets` directly. The query joins `ranking_players` (not `entrant.player_id` directly) to identify ranking members:

```sql
SELECT
    we.player_id                        AS "winner_player_id?: Uuid",
    COALESCE(wp.name, we.display_name)  AS "winner_name!",
    we.seed                             AS winner_seed,
    le.player_id                        AS "loser_player_id?: Uuid",
    COALESCE(lp.name, le.display_name)  AS "loser_name!",
    le.seed                             AS loser_seed,
    -- set/event/tournament/phase fields unchanged
FROM sets s
JOIN entrants we ON we.id = s.winner_entrant_id
JOIN entrants le ON le.id = s.loser_entrant_id
JOIN ranking_events re ON re.event_id = s.event_id AND re.ranking_id = $1
JOIN events e ON e.id = s.event_id
JOIN tournaments t ON t.id = e.tournament_id
LEFT JOIN ranking_players rwp ON rwp.player_id = we.player_id AND rwp.ranking_id = $1
LEFT JOIN players wp ON wp.id = rwp.player_id
LEFT JOIN ranking_players rlp ON rlp.player_id = le.player_id AND rlp.ranking_id = $1
LEFT JOIN players lp ON lp.id = rlp.player_id
LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
LEFT JOIN phases ph ON ph.id = pg.phase_id
WHERE re.included = true
  AND s.is_dq = false
  AND s.has_placeholder = false
  AND (rwp.player_id IS NOT NULL OR rlp.player_id IS NOT NULL)
```

`get_player_stats` adds `AND (we.player_id = $2 OR le.player_id = $2)`.

**Upset factor:** computed in Rust from `(winner_seed, loser_seed)` using `set_upset_factor()`. Falls back to `0` if either seed is NULL. Identical to pre-task-8 logic.

**`opponent_id`:** `Some(player_uuid)` when the opponent has a `ranking_players` row in this ranking, `None` otherwise. Never an entrant UUID. Resolution:

```rust
let winner_opp_id = row.winner_player_id; // Option<Uuid>
let loser_opp_id  = row.loser_player_id;  // Option<Uuid>
```

**`opponent_name`:** `COALESCE(player.name, entrant.display_name)` — always non-null. For ranking members this is the player name; for non-members it is the entrant display name.

**H2H endpoints** (`get_head_to_head`, `get_h2h_sets`): untouched. They continue reading from `ranking_set_results` where both sides are always ranking members. `opponent_id` in H2H responses remains non-nullable (both players are always members).

### 4. Frontend

**`src/lib/types.ts`** — `SetRecord.opponent_id` becomes nullable:

```ts
opponent_id: string | null;
```

**`stats/+page.svelte` and `players/[player_id]/+page.svelte`** — null-guard added before `trackedPlayerIds` check:

```ts
selectedSet.opponent_id !== null && trackedPlayerIds.has(selectedSet.opponent_id)
  ? selectedSet.opponent_id
  : undefined
```

No other frontend changes. `opponent_name` is always present so rendering is unaffected.

**`openapi.yaml`** — `SetRecord.opponent_id` gains `nullable: true`.

### 5. sqlx offline cache

After all query changes, run `bash backend/prepare-sqlx.sh`. The seven `.sqlx/` files currently deleted in the working tree (from the previous prepare run) will be replaced by the correct set for the new queries.

---

## Data source summary

| Endpoint | Source | Both players must be members? |
|---|---|---|
| `GET /stats`, `GET /stats/:pid` | `sets` table directly | No — at least one |
| `GET /head-to-head`, `GET /head-to-head/:a/:b/sets` | `ranking_set_results` | Yes |
| Algorithm compute (Elo/Glicko-2) | `ranking_set_results` | Yes |

---

## Out of scope

- `result_sort = 'global_rating'` — deferred to Sub-project B (global mirror)
- Per-player or per-tournament pages
- Deduplication via global mirror — this project-scoped model is an interim solution; Sub-project B will maintain a separate `global_*` table family and the local tables can be simplified or deprecated at that point
