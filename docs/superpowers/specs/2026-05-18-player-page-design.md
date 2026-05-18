# Player Detail Page

**Date:** 2026-05-18  
**Status:** Approved

## Overview

A dedicated per-player page at `/projects/[id]/players/[player_id]` that shows a player's complete wins/losses and full tournament attendance history within a project. Accessible from every place a player name currently appears in the UI.

## Route

```
/projects/[id]/players/[player_id]
```

Sits inside the existing `projects/[id]` layout, so the tab bar and project header remain visible. SvelteKit's file-system routing handles this cleanly: `routes/projects/[id]/players/[player_id]/` is a child of `routes/projects/[id]/players/` with no conflict — the existing players management page stays at `+page.svelte` and the new detail page is a nested dynamic segment. The page is readable by anyone with project read access (same auth rules as Stats and H2H).

## Backend — two new API endpoints

### `GET /projects/{project_id}/stats/{player_id}`

Returns wins and losses for a single player. Uses the same SQL as the existing `/stats` handler but adds a filter so only sets involving that player are returned:

```sql
AND (we.player_id = $2 OR le.player_id = $2)
```

Response shape — a single object (not an array):

```json
{
  "player_id": "uuid",
  "name": "mang0",
  "wins": [ /* SetRecord[] */ ],
  "losses": [ /* SetRecord[] */ ]
}
```

Wins and losses include sets against untracked opponents (same semantics as the existing `/stats` endpoint — at least one side must be a tracked project player). Sets are sorted by upset factor descending.

Returns 404 if the project or player does not exist.

### `GET /projects/{project_id}/players/{player_id}/tournaments`

Returns all tournament events the player has ever entered, derived from the `entrants` table via the player's linked start.gg accounts. Not restricted to included events or events with tracked opponents.

Response shape — an array sorted by `start_at` descending:

```json
[
  {
    "tournament_name": "Genesis 9",
    "tournament_slug": "genesis-9",
    "event_name": "Melee Singles",
    "placement": 1,
    "num_entrants": 486,
    "start_at": "2024-01-12T00:00:00Z",
    "location": "San Jose, CA"
  }
]
```

`placement` is `entrants.final_placement` (nullable). `location` is derived from `tournaments.online`, `city`, `addr_state`, `country_code` using the same `compute_location` helper already used in the stats handler.

Returns 404 if the project or player does not exist.

## Frontend — new page

### `web/src/routes/projects/[id]/players/[player_id]/+page.server.ts`

Loads both endpoints in parallel:

```ts
const [statsRes, tournamentsRes] = await Promise.all([
  api.get(`/projects/${params.id}/stats/${params.player_id}`),
  api.get(`/projects/${params.id}/players/${params.player_id}/tournaments`),
]);
```

### `web/src/routes/projects/[id]/players/[player_id]/+page.svelte`

Vertical scroll layout:

1. **Back link** — `← Back` button that calls `history.back()`. The player page is reachable from Stats, Players, and H2H, so a fixed link destination would be wrong depending on the entry point.
2. **Header** — player name, start.gg handles (from accounts on the player record), total W · L · win rate · tournament count
3. **Wins / losses** — two side-by-side `ScrollArea` panels (green / red), each row shows opponent name + UF + tournament name, clickable to open `SetDetailModal`
4. **Tournament history** — shadcn `Table` with columns: Tournament · Event, Placement, Entrants, Date. Placement cell is green for top 3, red otherwise, grey if null.

Uses `SetDetailModal` (existing component) for set drill-down.

### New TypeScript type — `web/src/lib/types.ts`

```ts
export interface TournamentAttendance {
  tournament_name: string;
  tournament_slug: string;
  event_name: string;
  placement: number | null;
  num_entrants: number | null;
  start_at: string | null;
  location: string | null;
}
```

## Navigation entry points — player names become links

Every place a tracked player's name renders should link to their detail page. Changes required:

| Location | Component / file | Change |
|---|---|---|
| Stats page cards | `routes/projects/[id]/stats/+page.svelte` | Wrap player name `<span>` in `<a href="/projects/{id}/players/{player.player_id}">` |
| Players page rows | `routes/projects/[id]/players/+page.svelte` | Wrap player name in link |
| H2H matrix row/column labels | `routes/projects/[id]/h2h/+page.svelte` | Wrap row label `<span>` and column label `<span>` in links |
| SetDetailModal opponent name | `lib/components/SetDetailModal.svelte` | Add optional `projectId` and `opponentPlayerId` props; render as link only when `opponentPlayerId` is a real player UUID (not a fallback entrant UUID) |

For `SetDetailModal`, the caller already has `opponent_id` from `SetRecord`. It should pass the project id and opponent id; the modal renders a link only when the opponent id is a known tracked player. The stats and H2H pages already have the player list available, so they can check membership before passing the prop.

## OpenAPI updates

Add both new endpoints to `backend/openapi.yaml`:

- `GET /projects/{project_id}/stats/{player_id}` → `PlayerStats` (single object)
- `GET /projects/{project_id}/players/{player_id}/tournaments` → `TournamentAttendance[]` (new schema)

## Out of scope

- Pagination on wins/losses or tournament history (ScrollArea handles long lists)
- Filtering sets by tournament or date range on the player page
- Player comparison view
