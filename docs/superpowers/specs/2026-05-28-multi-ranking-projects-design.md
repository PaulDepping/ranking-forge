# Multi-Ranking Projects Design

**Date:** 2026-05-28
**Status:** Approved

## Overview

Transform `ranking_projects` from a single-ranking entity into a project container that holds
multiple independent rankings. Each ranking selects a subset of the project's shared player pool
and event pool, enabling multiple views of the same scene (e.g. different seasons, different
criteria) without duplicating players or re-importing tournaments.

Modeled after braacket.com's organization → ranking hierarchy.

---

## Data Model

### Schema consolidation

The two existing migrations (`001_initial.sql`, `002_job_progress.sql`) are collapsed into a
single `001_initial.sql`. No prod database exists.

### Table rename

`ranking_projects` → `projects`

All columns are preserved. The `published` flag is **removed** from `projects` (it moves to
`rankings`).

### New table: `rankings`

```sql
CREATE TABLE rankings (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    description TEXT,
    published   BOOLEAN     NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX rankings_project_id_idx ON rankings(project_id);
```

Each project contains one or more rankings. The `published` flag gates guest access at the
ranking level — unpublished rankings are invisible to non-members even if sibling rankings are
public.

### New table: `ranking_players`

Replaces `players.rank_position`. Scopes a subset of the project's player pool to a ranking and
holds the per-ranking ordering and notes.

```sql
CREATE TABLE ranking_players (
    ranking_id    UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    player_id     UUID    NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    rank_position INTEGER NOT NULL DEFAULT 0,
    notes         TEXT,
    PRIMARY KEY (ranking_id, player_id)
);

CREATE INDEX ranking_players_player_id_idx ON ranking_players(player_id);
```

### Replaced table: `project_events` → `ranking_events`

Event inclusion/exclusion moves from project scope to ranking scope.

```sql
CREATE TABLE ranking_events (
    ranking_id UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    event_id   UUID    NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    included   BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (ranking_id, event_id)
);
```

A missing row is treated as `included = true` (default-include). New events from an import
become visible in all rankings until manually excluded.

### Unchanged tables

`players`, `project_members`, `project_invite_links`, `jobs`, `startgg_accounts`,
`tournaments`, `events`, `phases`, `phase_groups`, `entrants`, `sets` are all unchanged.
Players, members, invite links, and import jobs remain project-scoped.
Tournaments and events remain global (no project ownership).

### `players` table change

`rank_position` column removed. Ordering is now fully managed via `ranking_players.rank_position`.

---

## API Changes

### Project-scoped endpoints (unchanged paths)

| Method | Path | Notes |
|---|---|---|
| GET/POST | `/projects` | unchanged |
| GET/PATCH/DELETE | `/projects/:id` | unchanged; `published` field removed from response |
| GET/POST/DELETE | `/projects/:id/players` | shared player pool management |
| PATCH | `/projects/:id/players/:pid` | unchanged |
| POST/DELETE | `/projects/:id/players/:pid/accounts` | unchanged |
| GET | `/projects/:id/tournament-entrants` | unchanged |
| POST/GET | `/projects/:id/import` | project-level import, unchanged |
| GET/POST/PATCH/DELETE | `/projects/:id/members` + sub-routes | unchanged |
| GET/POST/DELETE | `/projects/:id/invite-links` + sub-routes | unchanged |
| POST | `/invite/:token/accept` | unchanged |
| GET | `/games` | unchanged |

### Tournament deletion (new, project-scoped)

`DELETE /projects/:id/tournaments/:tournament_id`

Removes all `ranking_events` rows for this tournament's events across every ranking in the
project. The global tournament data is preserved (other projects may reference it). On the next
import the tournament will reappear unless excluded again.

### Ranking management (new)

| Method | Path | Description |
|---|---|---|
| GET | `/projects/:id/rankings` | List all rankings in the project |
| POST | `/projects/:id/rankings` | Create a ranking (name, description) |
| GET | `/projects/:id/rankings/:rid` | Get ranking details |
| PATCH | `/projects/:id/rankings/:rid` | Update name, description, published |
| DELETE | `/projects/:id/rankings/:rid` | Delete ranking and all its ranking_players/ranking_events |

### Ranking player membership (new)

| Method | Path | Description |
|---|---|---|
| GET | `/projects/:id/rankings/:rid/players` | List ranking's players with rank_position and notes |
| POST | `/projects/:id/rankings/:rid/players` | Add a project player to this ranking |
| DELETE | `/projects/:id/rankings/:rid/players/:pid` | Remove player from ranking |
| PATCH | `/projects/:id/rankings/:rid/players/:pid` | Update notes |

### Ranking-scoped endpoints (moved from project scope)

| Old path | New path |
|---|---|
| `PUT /projects/:id/ranking` | `PUT /projects/:id/rankings/:rid/ranking` |
| `GET /projects/:id/tournaments` | `GET /projects/:id/rankings/:rid/tournaments` |
| `PATCH /projects/:id/events/:eid` | `PATCH /projects/:id/rankings/:rid/events/:eid` |
| `GET /projects/:id/stats` | `GET /projects/:id/rankings/:rid/stats` |
| `GET /projects/:id/stats/:pid` | `GET /projects/:id/rankings/:rid/stats/:pid` |
| `GET /projects/:id/head-to-head` | `GET /projects/:id/rankings/:rid/head-to-head` |
| `GET /projects/:id/head-to-head/:a/:b/sets` | `GET /projects/:id/rankings/:rid/head-to-head/:a/:b/sets` |

### Access control

Unchanged in structure. Project owner/editors manage all rankings within a project. Viewers can
read all rankings. Guest access (no account) is granted per-ranking via `rankings.published`.

---

## Frontend Routes

### Project-level routes (unchanged)

| Path | Access | Purpose |
|---|---|---|
| `/projects/[id]/settings` | Owner only | Project name, game, members, invite links |
| `/projects/[id]/(editor)/import` | Owner/editor | Trigger import, view job status |
| `/projects/[id]/(editor)/players` | Owner/editor | Manage shared player pool |
| `/projects/[id]/(editor)/players/[player_id]` | Owner/editor | Edit player, link start.gg accounts |

### Project overview (changed)

`/projects/[id]` — Lists rankings as cards (name, description, player count, published badge).
"Create ranking" button for editors/owners. If the project has exactly one ranking, redirects
directly to that ranking.

### Ranking routes (new)

| Path | Access | Purpose |
|---|---|---|
| `/projects/[id]/rankings/[rid]` | Owner/member (published: guest) | Ranking overview; same role-based redirect as current project overview |
| `/projects/[id]/rankings/[rid]/ranking` | Owner/member (published: guest) | Players ordered by aggregate upset factor |
| `/projects/[id]/rankings/[rid]/stats` | Owner/member (published: guest) | Per-player win/loss lists |
| `/projects/[id]/rankings/[rid]/h2h` | Owner/member (published: guest) | Head-to-head matrix |
| `/projects/[id]/rankings/[rid]/tournaments` | Owner/member (published: guest) | Tournament list with per-ranking include/exclude toggles and project-level delete |
| `/projects/[id]/rankings/[rid]/(editor)/players` | Owner/editor | Manage which project players are in this ranking; reorder; add notes |

### `(editor)` group

The existing `(editor)` group layout check (owner/editor role guard) extends to cover the new
ranking-level editor routes.

### Tournament delete UX

The delete button appears on the ranking-level tournament page. A confirmation dialog makes
clear that deletion removes the tournament from all rankings in the project (not just the
current one), and that re-importing may bring it back.

---

## Backend Rust Changes

### `common` crate

- Add `Ranking`, `RankingPlayer` structs to `src/models/mod.rs`
- Remove `rank_position` from `Player` struct
- Remove `published` from `Project` struct

### `api` crate

- Rename all `sqlx::query!` table references: `ranking_projects` → `projects`,
  `project_events` → `ranking_events`
- Remove `rank_position` from player queries; order via `ranking_players`
- Add `src/routes/rankings.rs` — ranking CRUD + ranking player management
- Move stats, H2H, ranking-order, tournament listing, event toggle into ranking-scoped handlers
- Stats and H2H queries join through `ranking_players` (player membership) and `ranking_events`
  (event inclusion) instead of `players.project_id` / `project_events`
- Guest access check switches from `projects.published` → `rankings.published`
- Add `DELETE /projects/:id/tournaments/:tournament_id` handler (removes `ranking_events` rows
  across all project rankings for that tournament's events)

### `worker` crate

- Import job remains project-scoped
- Queries all players by `project_id` (the full pool) to collect start.gg accounts — not limited to `ranking_players`, so players not yet added to any ranking are still imported
- Table rename in queries: `ranking_projects` → `projects`

### sqlx offline cache

Run `bash backend/prepare-sqlx.sh` after all query changes to regenerate `.sqlx/`.

---

## Future Work (out of scope for this spec)

- **Ranking algorithms** — Elo, win %, or other metrics beyond upset factor
- **Ranking snapshots/history** — freeze a ranking state for archival ("end of season")
- **Player profile pages** — per-player view across all rankings they appear in
- **Human-readable URL slugs** — `/projects/my-region/rankings/2025-season`
- **Criteria-based event filters** — date range, minimum entrant count per ranking
