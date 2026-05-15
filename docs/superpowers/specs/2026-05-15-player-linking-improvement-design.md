---
title: Player linking improvement
date: 2026-05-15
status: approved
---

# Player linking improvement

## Problem

The current player import/linking flow requires two manual steps per player:
1. Type the player's name to create a player record.
2. Navigate to the player's start.gg profile, copy the URL slug, and paste it into a dialog.

Step 2 is the primary time sink. For an initial roster of ~20 players this means ~20 separate profile lookups.

## Goals

- Let users add a batch of players from a single tournament URL with no individual slug lookups.
- Let users add individual players by pasting bare start.gg handles (e.g. `mang0`), full slugs, or full profile URLs.
- Let users rename a player after creation.
- Normalize slug/handle storage across the DB to store only the bare identifier in each table.

## DB changes (edit `001_initial.sql` in place — no prod data exists)

| Table | Column change | Stored value |
|---|---|---|
| `startgg_accounts` | `slug` → `handle` (NOT NULL) | `mang0` |
| `tournaments` | `slug` → `handle` (NOT NULL) | `some-weekly` |
| `events` | `slug` → `handle` (NOT NULL) | `melee-singles` |

All three follow the same principle: store the locally meaningful segment only. Full start.gg URLs are reconstructed from context:
- User profile: `start.gg/user/{handle}`
- Tournament: `start.gg/tournament/{handle}`
- Event: `start.gg/tournament/{tournament.handle}/event/{event.handle}`

All three `handle` columns are NOT NULL.

All Rust `sqlx::query!` macros, model structs, and API response types updated to use `handle`. The `.sqlx/` offline cache regenerated via `prepare-sqlx.sh`.

## New start.gg operation: `tournament_entrants`

A new `StartggClient` method taking a tournament handle. Fetches all events for the project's `game_id` at that tournament, then all entrants across those events. Returns `Vec<TournamentEntrant>`:

```rust
pub struct TournamentEntrant {
    pub startgg_user_id: i64,
    pub handle: String,    // bare handle, e.g. "mang0"
    pub name: String,      // gamer tag
}
```

Uses the existing adaptive `perPage` halving pattern on `ComplexityTooHigh`. Entrants without a linked start.gg user (guests, anonymous) are omitted.

## API changes

### Existing: `POST /projects/:id/players/:pid/accounts`

Field renamed from `slug` to `handle` in the request body. The backend normalizes any of three input formats before storing and before calling start.gg:
- Bare handle: `mang0` → stored as `mang0`
- Full slug: `user/mang0` → stored as `mang0`
- Full URL: `https://www.start.gg/user/mang0` → stored as `mang0`

### New: `GET /projects/:id/tournament-entrants?tournament=<handle>`

Read-only. Accepts a tournament handle or any start.gg URL containing the tournament segment. Normalization: strip `https://www.start.gg/`, strip leading `tournament/`, keep only the first path segment (e.g. `https://www.start.gg/tournament/some-weekly/event/melee-singles` → `some-weekly`). Calls `tournament_entrants` on the start.gg client and returns the entrant list for the frontend checkbox picker. Nothing is written to the DB.

Response: `[{ startgg_user_id, handle, name }]`

### New: `POST /projects/:id/players/bulk`

Creates players from a pre-resolved list. Both the tournament picker and the by-handle tab converge here.

Request:
```json
{
  "players": [
    { "name": "Mang0", "startgg_user_id": 12345, "handle": "mang0" },
    { "name": "Armada", "startgg_user_id": 67890, "handle": "armada" }
  ]
}
```

For each entry: inserts a `players` row then a `startgg_accounts` row. Skips any `startgg_user_id` already linked in this project without error.

Response: `[{ name, handle, status: "created" | "skipped" }]`

### New: `PATCH /projects/:id/players/:pid`

Renames a player.

Request: `{ "name": "New Name" }`

Response: updated `PlayerResponse`. Returns 422 if name is empty.

## Frontend changes (high-level — full spec deferred)

The Players page gains a single **"Add players"** button that opens a dialog with two tabs.

**Tab: From tournament**
- Text input for tournament URL or handle.
- "Fetch" button calls `GET /projects/:id/tournament-entrants`.
- Displays a scrollable, searchable checklist of entrants (gamer tag + bare handle).
- "Add N players" button calls `POST /projects/:id/players/bulk` with selected entries.
- Already-present players shown as disabled/greyed in the list.

**Tab: By handle**
- Textarea accepting one entry per line: bare handle, full slug, or full URL.
- On submit, calls `POST /projects/:id/players/bulk` (backend resolves and creates in one step).
- Results shown inline after submission: created / skipped / not found per entry.

**Edit player name**
- Each player row gains an Edit button.
- Clicking opens an inline input or small dialog pre-filled with the current name.
- On confirm, calls `PATCH /projects/:id/players/:pid`.

**Handle normalization**
- The existing per-player "add account" dialog updated to accept bare handle, full slug, or full URL (label updated to "start.gg handle").

## Out of scope

- Player search/autocomplete by gamer tag (start.gg has no global user search endpoint).
- Merging duplicate players.
- Frontend implementation detail (deferred to a follow-up spec).
