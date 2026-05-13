# Stats: All Opponents + Game Scores

**Date:** 2026-05-13  
**Status:** Approved

## Problem

The Stats tab currently shows wins and losses only between project players. Sets against anyone outside the project's player list are silently dropped. Users want to see results against every opponent encountered in imported tournaments, and also want the game score (e.g. 3-1) shown for each set.

## Design

### Backend — `GET /projects/{id}/stats`

**SQL change in `get_stats` (`crates/api/src/routes/tournaments.rs`):**

- Change both `JOIN players wp` and `JOIN players lp` from INNER to LEFT JOIN.
- Add filter: `AND (wp.id IS NOT NULL OR lp.id IS NOT NULL)` — at least one side must be a project player.
- Use `COALESCE(wp.name, we.display_name)` / `COALESCE(lp.name, le.display_name)` for names.
- Add `s.winner_score`, `s.loser_score` to the SELECT.
- Add `we.id AS winner_entrant_id`, `le.id AS loser_entrant_id` to the SELECT (used as the opponent identifier when the opponent has no project player UUID).

**`SetRecord` struct — add two fields:**

```rust
pub winner_score: Option<i16>,
pub loser_score:  Option<i16>,
```

`opponent_id` stays `Uuid`. For project players it remains their `players.id`; for non-project opponents it is the `entrants.id` UUID. The type is unchanged.

**Rust accumulator logic:**

- `winner_player_id` is now `Option<Uuid>` (nullable because of the LEFT JOIN).
- `loser_player_id` is now `Option<Uuid>`.
- If `winner_player_id` is `Some(id)` → push a `SetRecord` into that player's wins; `opponent_id` is `loser_player_id.unwrap_or(loser_entrant_id)`.
- If `loser_player_id` is `Some(id)` → push a `SetRecord` into that player's losses; `opponent_id` is `winner_player_id.unwrap_or(winner_entrant_id)`.
- A set between two project players gets recorded in both (unchanged from today).

### Frontend — TypeScript types (`web/src/lib/types.ts`)

Add to `SetRecord`:

```ts
winner_score: number | null;
loser_score:  number | null;
```

### Frontend — Stats UI (`web/src/routes/projects/[id]/stats/+page.svelte`)

In the expanded set-detail rows, display the score from the project player's perspective:

- **Wins:** `{winner_score}–{loser_score}` (e.g. `3-1`)
- **Losses:** `{loser_score}–{winner_score}` (e.g. `1-3`)
- When either score is `null`, show `–` instead of the score pair.

The score is shown alongside the existing UF display, e.g.:

```
PlayerName        3-1   UF 2.0
```

## Scope

- No new endpoints, no new DB migrations, no new tables.
- Only `get_stats`, `SetRecord`, the TS type, and the Svelte component change.
- `.sqlx/` offline cache must be regenerated after the SQL change (`cargo sqlx prepare --workspace -- --all-targets`).

## Out of Scope

- Grouping non-project opponents by `startgg_user_id` (each set shown individually).
- Preserving a project-only view alongside the new all-opponents view.
