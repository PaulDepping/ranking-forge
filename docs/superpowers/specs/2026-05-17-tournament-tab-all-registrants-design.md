# Design: Tournament Tab — All Registrants + Per-Event Filtering

**Date:** 2026-05-17  
**Status:** Approved

## Summary

The "From tournament" tab in the Add Players dialog currently only fetches entrants from events matching the project's configured game. If the tournament has no matching event, it errors. This redesign makes it return all tournament registrants by default, with per-event tabs for filtering and a seed/placement toggle for ordering.

---

## Backend

### New StartggClient functions (`common/src/startgg/operations.rs`)

**`tournament_participants(handle: &str) -> Result<Vec<TournamentParticipant>>`**

Paginates through `tournament { participants }` — the start.gg endpoint for all registrants, including spectators. Each participant must have a linked user account (`user.id` present); those without are skipped. Returns name, handle (derived from `user.slug` by stripping the `user/` prefix), and `startgg_user_id`.

**`tournament_events_with_entrants(handle: &str) -> Result<Vec<TournamentEventWithEntrants>>`**

First fetches all events at the tournament with no game filter (`tournament { events { id name } }`). Then for each event, paginates through entrants fetching `initialSeedNum` and `standing { placement }`. Deduplication within an event is not needed (each entrant appears once per event). The existing `tournament_entrants(handle, game_id)` function is unchanged — it is used by the background import worker.

### New public types (`common/src/startgg/queries.rs`)

```rust
pub struct TournamentParticipant {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
}

pub struct TournamentEntrantOrdered {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
    pub seed: Option<i32>,
    pub placement: Option<i32>,
}

pub struct TournamentEventWithEntrants {
    pub id: i64,
    pub name: String,
    pub entrants: Vec<TournamentEntrantOrdered>,
}
```

### New GraphQL queries

- `TOURNAMENT_PARTICIPANTS_QUERY` — pages `tournament { participants { nodes { gamerTag user { id slug } } } }`
- `TOURNAMENT_ALL_EVENTS_QUERY` — fetches `tournament { events { id name } }` with no game filter
- Modify `TOURNAMENT_ENTRANT_LIST_QUERY` to also fetch `initialSeedNum` and `standing { placement }` per entrant node

### Modified API endpoint (`api/src/routes/players.rs`)

`GET /projects/{id}/tournament-entrants?tournament=...`

The `game_id` requirement is removed — the endpoint no longer errors when the project has no configured game. It calls both new StartggClient functions and returns:

```json
{
  "all_participants": [
    { "startgg_user_id": 1234, "handle": "mang0", "name": "Mang0" }
  ],
  "events": [
    {
      "id": 123456,
      "name": "Melee Singles",
      "entrants": [
        { "startgg_user_id": 1234, "handle": "mang0", "name": "Mang0", "seed": 1, "placement": 1 }
      ]
    }
  ]
}
```

New response types in `players.rs`:

```rust
pub struct TournamentDataResponse {
    pub all_participants: Vec<TournamentParticipantResponse>,
    pub events: Vec<TournamentEventResponse>,
}

pub struct TournamentParticipantResponse {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
}

pub struct TournamentEventResponse {
    pub id: i64,
    pub name: String,
    pub entrants: Vec<TournamentEntrantOrderedResponse>,
}

pub struct TournamentEntrantOrderedResponse {
    pub startgg_user_id: i64,
    pub handle: String,
    pub name: String,
    pub seed: Option<i32>,
    pub placement: Option<i32>,
}
```

---

## Frontend

### New types (`web/src/lib/types.ts`)

```typescript
export type TournamentParticipant = {
  startgg_user_id: number;
  handle: string;
  name: string;
};

export type TournamentEntrantOrdered = {
  startgg_user_id: number;
  handle: string;
  name: string;
  seed: number | null;
  placement: number | null;
};

export type TournamentEventData = {
  id: number;
  name: string;
  entrants: TournamentEntrantOrdered[];
};

export type TournamentData = {
  all_participants: TournamentParticipant[];
  events: TournamentEventData[];
};
```

The existing `TournamentEntrant` type is replaced by `TournamentParticipant` (same shape). Update `AddPlayersDialog.svelte` and any other references.

### Reworked `TournamentTab.svelte`

**State changes:**
- `entrants: TournamentEntrant[]` → `tournamentData: TournamentData | null`
- New: `activeTab: 'all' | string` — `'all'` or an event id (as string). Defaults to `'all'` after fetch.
- New: `sortMode: 'placement' | 'seed'` — defaults to `'placement'`
- `selected: Set<number>` — unchanged; shared across all tabs, never reset on tab switch
- `search: string` — unchanged; applies to whichever tab is active

**Derived state:**
- `visibleEntrants` — derived from `activeTab` and `sortMode`:
  - `activeTab === 'all'`: `tournamentData.all_participants` sorted alphabetically by name
  - otherwise: matching event's `entrants` sorted by `placement` (nulls last) or `seed` (nulls last)
- `filteredEntrants` — `visibleEntrants` filtered by `search` (existing name/handle match logic)
- `selectableFiltered`, `allSelected`, `alreadyAddedCount`, `selectedCount` — unchanged logic, applied to `filteredEntrants`

**UI layout:**

```
[ tournament input ]  [ Fetch ]

[ All | Event 1 | Event 2 | ... ]        ← shadcn Tabs, scrollable if many events

                    [ Placement | Seed ]  ← toggle hidden on "All" tab

[ Search entrants… ]
[ ☐ Select all ]
┌──────────────────────────────────────┐
│ ☐  [rank]  Name       handle        │
│ ☐  [rank]  Name       handle  [badge]│
└──────────────────────────────────────┘

N selected · M already added   [ Add N players ]
```

- The `[rank]` column is hidden on the "All" tab (no meaningful rank for cross-event participants)
- On event tabs, rank shows ordinal placement ("1st", "2nd") or seed ("#1", "#2") depending on `sortMode`
- `null` placement/seed renders as "—"
- "Select all" selects only the currently visible filtered entrants on the active tab
- Selections persist when switching tabs
- The "Add N players" button and `addSelected()` function are unchanged — they POST to `/players/bulk` with name, handle, startgg_user_id

---

## What is NOT changing

- `POST /projects/{id}/players/bulk` — unchanged
- `tournament_entrants(handle, game_id)` in StartggClient — unchanged (used by import worker)
- Selection model, "already added" detection, bulk-add logic — unchanged
- `HandleTab` and `NameTab` — unchanged

## Test changes

- `tests/mock-api.js`: `MOCK_ENTRANTS` (currently a flat array) must be updated to the new response shape — `{ all_participants: [...], events: [{ id, name, entrants: [...with seed/placement] }] }`
- The existing Playwright test "Add players dialog opens with three tabs" continues to pass unchanged (it doesn't interact with the fetch flow)
- A new e2e test should cover: fetching a tournament → all-participants tab shows results → switching to an event tab shows ordered entrants → selecting across tabs → adding players

---

## Open questions / edge cases

- **Large tournaments**: `tournament.participants` can be in the thousands for majors. The existing pagination loop handles this but the fetch will be slow. No spinner change needed — loading state already exists.
- **Participants without user accounts**: skipped (no `startgg_user_id` to key on).
- **Events with no entrant data** (e.g. side bracket not yet run): entrants list will be empty; the tab still shows.
- **Seed vs placement on incomplete events**: `placement` will be `null` for events in progress; sort falls back to seed order in that case.
