# Per-Ranking Player Detail Page

**Date:** 2026-06-14

## Problem

The player detail page at `/projects/[id]/players/[player_id]` is broken. Its
server load calls `GET /projects/{id}/stats/{player_id}`, which no longer
exists — stats are now ranking-scoped at
`GET /projects/{id}/rankings/{rid}/stats/{player_id}`. Additionally, the page
sits under the `(editor)` route group, making it inaccessible to viewers and
guests even though it only shows read-only stats content.

## Solution

Move the player detail page into the rankings subtree so it is scoped to a
specific ranking, matching the backend's data model and access control.

## New Route

```
/projects/[id]/rankings/[rid]/players/[player_id]
```

- Replaces `/projects/[id]/players/[player_id]` (deleted)
- No route group guard — access is inherited from the ranking layout
  (`require_ranking_read_access`), so owners, members, and guests on published
  rankings can all view it
- The ranking layout wraps it automatically, providing breadcrumb and tab
  navigation

## New Backend Endpoint

```
GET /projects/{id}/rankings/{rid}/players/{pid}/tournaments
```

Returns tournament history filtered to events that are included in the
specified ranking (`ranking_events.included = true AND ranking_events.ranking_id = rid`).

- Returns the existing `TournamentAttendance` shape
- Access guard: `require_ranking_read_access` (same as `get_stats`)
- Registered at `/{rid}/players/{pid}/tournaments` in the rankings router
  (`rankings.rs`)
- The path struct reuses or mirrors `RankingPlayerStatPath` (`id`, `rid`,
  `player_id`)

The existing `GET /projects/{id}/rankings/{rid}/stats/{player_id}` endpoint
already works correctly and needs no changes.

## Page Server Load

Three parallel requests:

| Request | Purpose |
|---|---|
| `GET /projects/{id}/rankings/{rid}/stats/{player_id}` | Player name, wins, losses |
| `GET /projects/{id}/rankings/{rid}/players/{pid}/tournaments` | Ranking-scoped tournament history (new endpoint) |
| `GET /projects/{id}/rankings/{rid}/players` | Build `trackedPlayerIds` for SetDetailModal opponent links |

The third request replaces the old project-wide `GET /projects/{id}/players`
so that only opponents who are in the same ranking are rendered as links.

## Page Content

Same structure as the current page — wins/losses cards, tournament history
table, SetDetailModal — with two changes:

1. Back button links to `/projects/{id}/rankings/{rid}/stats`
2. `SetDetailModal` receives a `rankingId` prop so opponent links resolve to
   the correct ranking-scoped URL

## Link Updates

Every link to the old project-level player URL must be updated. All affected
files are inside the `rankings/[rid]/` subtree and already have `data.ranking.id`
available.

| File | Change |
|---|---|
| `rankings/[rid]/stats/+page.svelte` | Add `rankings/{rid}` segment to player name `href` |
| `rankings/[rid]/ranking/+page.svelte` (×2) | Add `rankings/{rid}` segment to player name `href` in both layout variants |
| `rankings/[rid]/h2h/+page.svelte` (×2) | Add `rankings/{rid}` segment to row and column header `href` |
| `lib/components/SetDetailModal.svelte` | Add `rankingId?: string` prop; update opponent `href` to include `rankings/{rankingId}` |
| `lib/components/PlayerCard.svelte` | Remove link; render player name as plain text |

`PlayerCard` is used only on the project-level players management page
(`(hub)/(editor)/players/`), which has no `rid` in scope. That page is for
admin actions (rename, delete, link accounts) and does not need a stats
shortcut.

## Files to Delete

- `web/src/routes/projects/[id]/(hub)/(editor)/players/[player_id]/+page.svelte`
- `web/src/routes/projects/[id]/(hub)/(editor)/players/[player_id]/+page.server.ts`

## Files to Create

- `web/src/routes/projects/[id]/rankings/[rid]/players/[player_id]/+page.svelte`
- `web/src/routes/projects/[id]/rankings/[rid]/players/[player_id]/+page.server.ts`

## Backend Files to Change

- `backend/crates/api/src/routes/tournaments.rs` — add `get_ranking_player_tournaments` handler
- `backend/crates/api/src/routes/rankings.rs` — register the new route at `/{rid}/players/{pid}/tournaments`

## Documentation to Update

- `docs/routes.md` — replace old player route row with new ranking-scoped row; update access column to Owner/member (published: guest)
- `backend/openapi.yaml` — add the new `GET /projects/{id}/rankings/{rid}/players/{pid}/tournaments` endpoint
