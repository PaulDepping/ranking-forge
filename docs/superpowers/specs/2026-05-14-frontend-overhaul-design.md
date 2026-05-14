# Frontend Overhaul Design

**Date:** 2026-05-14  
**Status:** Approved

## Context

The Stats and H2H pages need a significant UX improvement. Accumulated Upset Factor is being removed (it is not a meaningful statistic). The Stats page currently shows a flat table with expandable rows, which doesn't scale well and provides no context about individual sets. The H2H matrix shows only win/loss counts with no way to drill into individual matches. Account linking requires a manual page refresh after adding an account.

## Changes in Scope

1. Remove Accumulated Upset Factor entirely (display and sort logic)
2. Display Upset Factor as a whole integer everywhere
3. Redesign Stats page as a grid of player cards with scrollable wins/losses lists
4. Add a shared set detail modal with full context (tournament, event, round, date, seeds, UF, optional VOD/start.gg links)
5. Add a H2H side panel showing all sets between two players, with set detail modal on click
6. Fix account linking to auto-refresh without a manual page reload

---

## Backend

### 1. Enrich `SetRecord`

**File:** `backend/crates/api/src/routes/tournaments.rs` — `SetRecord` (line 49) and `PlayerStatsResponse` (line 58) are both defined inline here

Add the following fields, all available via JOIN through `sets → events → tournaments` and `sets → entrants`:

| Field | Source | Notes |
|---|---|---|
| `tournament_name` | `tournaments.name` | |
| `tournament_slug` | `tournaments.slug` | Used to build start.gg link |
| `event_name` | `events.name` | |
| `round_name` | `sets.round_name` | e.g. "Winners Finals" |
| `completed_at` | `sets.completed_at` | `Option<DateTime<Utc>>` |
| `is_dq` | `sets.is_dq` | |
| `vod_url` | `sets.vod_url` | `Option<String>` |
| `startgg_set_id` | `sets.startgg_set_id` | `i64`, used to build start.gg link |
| `winner_seed` | `entrants.seed` (winner side) | `Option<i32>` |
| `loser_seed` | `entrants.seed` (loser side) | `Option<i32>` |

After changes, run `bash backend/prepare-sqlx.sh` to regenerate `.sqlx/`.

### 2. Replace sort order in stats handler

**File:** `backend/crates/api/src/routes/tournaments.rs` lines ~374–378

Replace the accumulated UF sort:
```rust
let a_uf: i64 = a.wins.iter().map(|s| s.upset_factor).sum();
let b_uf: i64 = b.wins.iter().map(|s| s.upset_factor).sum();
b_uf.cmp(&a_uf).then(b.wins.len().cmp(&a.wins.len()))
```

With win rate, win count tiebreaker, zero-set players sort last:
```rust
let a_total = a.wins.len() + a.losses.len();
let b_total = b.wins.len() + b.losses.len();
let a_rate = if a_total == 0 { -1.0 } else { a.wins.len() as f64 / a_total as f64 };
let b_rate = if b_total == 0 { -1.0 } else { b.wins.len() as f64 / b_total as f64 };
b_rate.partial_cmp(&a_rate).unwrap_or(Equal).then(b.wins.len().cmp(&a.wins.len()))
```

### 3. New H2H sets endpoint

**Route:** `GET /projects/{id}/head-to-head/{pid_a}/{pid_b}/sets`  
**Response:** `Vec<H2HSet>` — each set includes `is_win: bool` (true if pid_a won), sorted chronologically by `completed_at` desc. `opponent_id` and `opponent_name` are always from pid_a's perspective (opponent = pid_b).  
**Auth:** `AuthUser` extractor (same as other project routes)  
**Error:** 404 if project doesn't belong to user

Query: join `sets` through `entrants` where `(winner_entrant.player_id = pid_a AND loser_entrant.player_id = pid_b) OR (winner_entrant.player_id = pid_b AND loser_entrant.player_id = pid_a)`, filtered to events included in the project.

> **Future:** User-configurable sort order for the stats page is explicitly out of scope here. When added, the sort field should be a query param on the stats endpoint so the backend owns ordering.

---

## Frontend

### Types (`web/src/lib/types.ts`)

Extend `SetRecord` with all new backend fields:

```typescript
export interface SetRecord {
  opponent_id: string;
  opponent_name: string;
  upset_factor: number;
  winner_score: number | null;
  loser_score: number | null;
  // new
  tournament_name: string;
  tournament_slug: string;
  event_name: string;
  round_name: string | null;
  completed_at: string | null;
  is_dq: boolean;
  vod_url: string | null;
  startgg_set_id: number;
  winner_seed: number | null;
  loser_seed: number | null;
}
```

Also add a thin extension type for the H2H sets endpoint, which returns a flat chronological list with an explicit win/loss flag (unlike stats which uses separate `wins[]`/`losses[]` arrays):
```typescript
export interface H2HSet extends SetRecord {
  is_win: boolean;  // true if pid_a (the row player) won this set
}
```

### API layer (`web/src/lib/api.ts`)

Add one new fetch function for H2H sets:
```typescript
getH2HSets(projectId: string, pidA: string, pidB: string): Promise<H2HSet[]>
```

This is called client-side (from the H2H page `<script>`) when a cell is clicked, not in a server load.

### Stats page (`web/src/routes/projects/[id]/stats/+page.svelte`)

Replace the current `<table>` with a responsive CSS grid of player cards:

- Grid: `grid-template-columns: repeat(auto-fill, minmax(320px, 1fr))`
- Each card:
  - Header row: player name (left) · `W {n} · L {n} · {win%}` (right)
  - Body: two equal flex columns — WINS and LOSSES — each a fixed-height (`~90px`) scrollable list
  - Each row in the list: `{opponent_name} · UF {upset_factor}` (integer, no decimal: `Math.round()` or cast)
  - Row is clickable — opens `SetDetailModal` with the selected `SetRecord` and a boolean indicating whether it's a win (needed to colour the header correctly)
- Remove all `totalUF()` logic and the Agg. UF column
- No rank badge

### Shared modal (`web/src/lib/components/SetDetailModal.svelte`)

New component. Props:
```typescript
export let set: SetRecord | null = null;   // null = closed
export let isWin: boolean = false;
export let currentPlayerName: string = '';
export let onClose: () => void;
```

Rendered as a shadcn `<Dialog>` (already installed). Content:

**Header:**  
- Title: `{currentPlayerName} vs {set.opponent_name}`  
- Subtitle: coloured result + score — `Win · {winner_score}–{loser_score}` (green) or `Loss · {loser_score}–{winner_score}` (red). If scores are null, show just "Win" / "Loss".

**Details grid (2 columns):**  
Tournament · Event · Round · Date · Seeds · Upset Factor  
- Seeds: winner seed in green, loser seed in red, `–` if null  
- Date: formatted as `MMM D, YYYY` from `completed_at`; "Unknown" if null  
- Round: `round_name` or "Unknown" if null  

**Footer links (only when non-null):**  
- `↗ View on start.gg` → `https://www.start.gg/tournament/{tournament_slug}`  
- `▶ Watch VOD` → `vod_url`

### H2H page (`web/src/routes/projects/[id]/h2h/+page.svelte`)

Extend the existing matrix:

- Clicking a non-diagonal cell calls `getH2HSets(projectId, rowPlayerId, colPlayerId)` and stores the result in a `selectedPair` reactive variable
- While loading, show a subtle spinner inside the panel area
- The side panel renders to the right of the matrix when `selectedPair` is set:
  - Header: `{playerA} vs {playerB}` · `{wins} wins · {losses} losses` · × close button
  - Scrollable list of sets, each row: W/L badge · score · tournament name · abbreviated round
  - Each row is clickable → opens `SetDetailModal` with `currentPlayerName` set to the row player's name and `isWin` taken directly from `H2HSet.is_win`
- The active cell gets a blue highlight border (`border-2 border-blue-500` / equivalent)
- On narrow screens the panel stacks below the matrix

### Players page (`web/src/routes/projects/[id]/players/+page.svelte`)

In the `use:enhance` callback for both `linkAccount` and the unlink delete form, call `invalidateAll()` after a successful response. This triggers SvelteKit to re-run the server load and update the displayed accounts without a manual refresh.

---

## Verification

1. `bash backend/test.sh` — all existing backend tests pass
2. `bash backend/prepare-sqlx.sh` — `.sqlx/` cache regenerated cleanly
3. Start dev stack, open Stats page — cards render in a grid, no Agg. UF column, UF values are integers, clicking a set row opens the detail modal with all fields populated
4. Open H2H page — clicking a cell loads and displays the side panel, clicking a set row in the panel opens the modal on top
5. Link a start.gg account on the Players page — the account appears under the player name immediately without refreshing
6. `cd web && npm run test:unit` — frontend unit tests pass
7. `cd web && npm run test:e2e` — Playwright e2e tests pass
