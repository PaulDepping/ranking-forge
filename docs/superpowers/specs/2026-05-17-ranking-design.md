# Design: Player Ranking Tab

**Date:** 2026-05-17
**Status:** Approved

## Overview

Add a **Ranking** tab to the project detail view. The ranking is the user's manually curated ordering of players. Once set, this order propagates to the Stats and H2H tabs. Players can be reordered via drag-and-drop or by editing rank numbers directly; changes are local until the user clicks Save.

---

## Backend

### Schema change (`001_initial.sql`)

Add `rank_position INTEGER NOT NULL DEFAULT 0` to the `players` table definition. Add an index on `(project_id, rank_position)`.

```sql
CREATE TABLE players (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id   UUID        NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    rank_position INTEGER    NOT NULL DEFAULT 0,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX players_project_id_rank_idx ON players(project_id, rank_position);
```

No migration needed — the existing migration file is overwritten because there is no production database.

### Player insert (`add_player`, `bulk_add_players`, `add_players_by_handles`)

When inserting a new player, assign:

```sql
rank_position = (SELECT COALESCE(MAX(rank_position), 0) + 1 FROM players WHERE project_id = $1)
```

New players always append to the bottom of the ranking.

### `list_players` query

Change `ORDER BY created_at ASC` → `ORDER BY rank_position ASC`.

H2H inherits the correct order automatically — no other H2H changes needed.

### `get_stats` handler

- Change the player fetch query from `ORDER BY created_at ASC` → `ORDER BY rank_position ASC`.
- Remove the win-rate `sort_by` at the end of the handler. Stats now renders in ranking order.

### New endpoint: `PUT /projects/{id}/ranking`

Reorders all players in a project.

**Request body:**
```json
{ "player_ids": ["uuid1", "uuid2", "uuid3"] }
```

**Validation:**
- All provided player IDs must belong to the project.
- The list must be complete — every player in the project must be present (no partial reorders).

**Behaviour:** Updates all `rank_position` values in a single transaction, assigning position = array index + 1.

**Responses:** `200 {}` on success · `400` for validation errors · `401` unauthenticated · `404` project not found.

### OpenAPI

Add `PUT /projects/{id}/ranking` to `openapi.yaml`.

---

## Frontend

### New route: `web/src/routes/projects/[id]/ranking/`

**`+page.server.ts`** — loads two endpoints in parallel:
- `GET /projects/:id/players` — ordered list of players (rank_position order)
- `GET /projects/:id/stats` — for inline win/loss record and win rate per row

**`+page.svelte`** — the ranking page component (see layout below).

### Layout tab

Add **"Ranking"** to the tab list in `+layout.svelte`, between H2H and Settings:

```ts
const tabs = [
  { label: 'Players',     href: 'players' },
  { label: 'Import',      href: 'import' },
  { label: 'Tournaments', href: 'tournaments' },
  { label: 'Stats',       href: 'stats' },
  { label: 'H2H',         href: 'h2h' },
  { label: 'Ranking',     href: 'ranking' },
  { label: 'Settings',    href: 'settings' },
];
```

### Ranking page layout

```
[ Ranking                                    [Unsaved changes] [Save] ]

⠿  1  mang0       18W · 3L   86%
⠿  2  Plup        12W · 6L   67%   ← subtle highlight on moved rows
⠿  3  Cody        15W · 5L   75%
⠿  4  Westballz   10W · 7L   59%
⠿  5  n0ne         8W · 9L   47%
```

Each row contains:
- **Drag handle** (⠿) — triggers drag-and-drop reorder
- **Rank number** — editable; click to change, confirm on Enter or blur; list re-sorts immediately
- **Player name**
- **W/L record** — from stats data (e.g. `18W · 3L`)
- **Win rate** — right-aligned (e.g. `86%`)

### Interaction

**Drag-and-drop:** implemented with `svelte-dnd-action` (uses the `use:` action directive, stable in Svelte 5). On drop, local state updates immediately.

**Editable rank number:** clicking a rank number makes it an editable input. On Enter or blur, the list re-sorts to match the new position.

**Save button:**
- Disabled when there are no local changes.
- Active (with "Unsaved changes" label) when the local order differs from the persisted order.
- On click: calls `PUT /projects/:id/ranking` with the current ordered player IDs.
- On success: button briefly shows "Saved ✓" then returns to disabled.

**Empty state:** if the project has no players, show the standard `<Empty>` component prompting the user to add players first.

**No stats yet:** if `GET /stats` returns an empty array (no imports done), rows still render with rank number and player name — the W/L and win rate columns are simply omitted for that player. The ranking is fully usable before any tournament data is imported.

### API layer (`src/lib/api.ts`)

Add:
```ts
putRanking(projectId: string, playerIds: string[]): Promise<Response>
```

### Stats and H2H tabs

No frontend changes required. Both tabs derive their player order from the API (`GET /players` and `GET /stats`), which now return players in `rank_position` order.

---

## What Does Not Change

- The Players, Import, Tournaments, and Settings tabs are unaffected.
- Worker, common crate, and all other backend crates are unaffected.
- No commit/finalize concept — the ranking is always the current live order. A snapshot/publish model is deferred to the future publishing feature.
- No partial reorder support — the `PUT /ranking` endpoint always replaces the full order.
