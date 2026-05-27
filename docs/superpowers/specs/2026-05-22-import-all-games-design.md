# Design: Import All Games When No Game Is Set

**Date:** 2026-05-22

## Problem

When a ranking project has no game selected, the import worker skips the entire import with a warning. The desired behavior is: a project with no game set imports all tournaments and events for each linked player, regardless of game, and stores per-event game metadata from start.gg.

## Approach

Two separate GraphQL queries — one filtered by game (existing), one unfiltered (new). The worker branches on `project.game_id` to choose the right path. This keeps each query self-contained and avoids optional-variable serialization complexity.

## GraphQL Layer (`common/src/startgg/operations.rs`)

### Existing query (unchanged)

`TOURNAMENTS_BY_USER_QUERY` keeps `videogameId: [$gameId]` on both the tournament filter and the events sub-selection. Game info on events is not requested (it comes from the project).

### New query

`TOURNAMENTS_BY_USER_ALL_GAMES_QUERY` omits `videogameId` everywhere and adds `videogame { id name }` to the events sub-selection so the API returns game metadata per event.

New vars struct: `TournamentsByUserAllGamesVars { user_id, page, per_page }` — no `game_id`.

New `StartggClient` method: `tournaments_by_user_all_games(user_id, page, per_page)` — same pagination and complexity-retry logic as `tournaments_by_user`.

### `EventNode` additions

Two new optional fields deserialized from the `videogame` sub-object returned by the all-games query:

```rust
pub videogame_id: Option<i64>,    // from videogame.id
pub videogame_name: Option<String>, // from videogame.name
```

These are `None` when the filtered query is used (the field is not requested).

## Import Worker (`worker/src/import.rs`)

### `run` function

Remove the early-return guard on `project.game_id`. Branch instead:

- **`Some(game_id)`** — existing path: `collect_user_tournaments` (filtered) → `import_tournament` with project-level `game_id`/`game_name`.
- **`None`** — new path: `collect_user_tournaments_all_games` (unfiltered) → `import_tournament` with `game_id = None` / `game_name = None` at the project level, resolved per-event from `EventNode`.

### `collect_user_tournaments_all_games`

New private function mirroring `collect_user_tournaments` — same pagination loop, date filtering, and deduplication into the `seen` map. Calls `startgg.tournaments_by_user_all_games(...)`.

### `import_tournament` and `import_event`

Change `game_id: i64` → `Option<i64>` and `game_name: Option<&str>` remains `Option<&str>`.

In `import_event`, when the caller passes `game_id = None`, use `event.videogame_id` and `event.videogame_name` as the values stored in the DB. When the caller passes `Some(game_id)`, use those (existing behavior).

## Data Model

No schema changes. The `events` table already has nullable `game_id` and `game_name` columns. Per-event game info populates them when the project has no game set.

## Error Handling

No new error cases. start.gg and DB errors continue to bubble up as `anyhow::Error` and fail the job.

## Testing

- Existing filtered-query tests are untouched.
- New integration test (wiremock-based, in `crates/api/tests/` or `crates/e2e/`):
  1. Seed a project with `game_id = NULL`.
  2. Stub `TOURNAMENTS_BY_USER_ALL_GAMES_QUERY` response with `videogame { id name }` on events.
  3. Run the import and assert events are created with the correct per-event `game_id`/`game_name` in the DB.
