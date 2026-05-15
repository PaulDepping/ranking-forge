# SetDetailModal Enrichment — Design Spec

**Date:** 2026-05-15

## Summary

Enrich the SetDetailModal with data that is already stored in the DB but not currently surfaced. Fix the start.gg link to point to the specific set instead of the tournament. Restructure the layout into three labelled sections: Match, Tournament, and Final Placements.

---

## Layout

Three sections separated by labelled dividers. All new fields are optional — any null field omits its row silently.

### Header

```
{currentPlayerName} vs {opponent_name}
Win  /  Loss                           ← colored green or red, no score here
```

Score moves out of the subtitle into the Match section.

### Section: Match

| Left | Right |
|------|-------|
| `{currentPlayerName} score` (green if win, red if loss) | `{opponent_name} score` (opposite color) |
| `{currentPlayerName} seed` (green if win) | `{opponent_name} seed` (red if win) |
| Upset Factor | — |

- Score row omitted if both scores are null.
- Seed row omitted if both seeds are null. If only one seed is null, show `?` for the missing value (consistent with current behavior).
- Seeds are prefixed with `#` (e.g. `#1`).
- Colors follow win/loss: winner stats green, loser stats red, consistent with existing behavior.

### Section: Tournament

| Left | Right |
|------|-------|
| Name (full-width, combined): `{tournament_name} · {event_name}` | — |
| Phase | Round |
| Location | Date |
| Entrants | — |

- Phase shows `{phase_name}` if available. If `pool_identifier` is also available, shown inline: `Top 8 · Pool A`.
- All Tournament section fields are independent cells in the CSS grid — null fields simply don't render a cell, and the grid auto-flows. Phase and Round appear as a natural pair when both are present; if only one is available it renders alone occupying one column.
- Location: `"Online"` if `tournament.online = true`, otherwise `"{city}, {state}"` (falling back through available fields: city + state → city + country_code → city → null → omit row).
- Entrants row omitted if `num_entrants` is null.
- Date format unchanged: `Jul 20, 2024`.

### Section: Final Placements

| Left | Right |
|------|-------|
| `{currentPlayerName}` | `{opponent_name}` |
| placement (green if win) | placement (red if win) |

- Entire section omitted if both placements are null.
- Placement displayed as ordinal: `1st`, `2nd`, `3rd`, `4th`, `5th`–`nth` using standard ordinal formatting.

### Footer

```
↗ View set on start.gg    ▶ Watch VOD
```

- **Set link fixed**: was `https://www.start.gg/{tournament_slug}` (tournament), now `https://www.start.gg/set/{startgg_set_id}` (the specific set). `startgg_set_id` is already in `SetRecord` — no backend change needed for this fix.
- VOD link unchanged.
- Footer only rendered if at least one link is available (unchanged behavior).

---

## Backend Changes

### New fields on `SetRecord`

```rust
pub phase_name: Option<String>,       // phases.name
pub pool_identifier: Option<String>,  // phase_groups.display_identifier
pub winner_placement: Option<i32>,    // winner entrant final_placement
pub loser_placement: Option<i32>,     // loser entrant final_placement
pub location: Option<String>,         // formatted in Rust from tournament fields
pub num_entrants: Option<i32>,        // events.num_entrants
```

Location is formatted server-side (not sent as raw lat/lng/city/state fields) to keep the frontend simple.

### SQL changes (both `get_stats` and `get_h2h_sets`)

Both queries already join `events e` and `tournaments t`. Two new LEFT JOINs needed:

```sql
LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
LEFT JOIN phases       ph ON ph.id = pg.phase_id
```

New SELECT columns:

```sql
pg.display_identifier  AS pool_identifier,
ph.name                AS phase_name,
we.final_placement     AS winner_placement,
le.final_placement     AS loser_placement,
e.num_entrants,
t.online,
t.city,
t.addr_state,
t.country_code
```

Location is computed in Rust when building the `SetRecord`:

```rust
let location = if row.online {
    Some("Online".into())
} else {
    match (row.city.as_deref(), row.addr_state.as_deref(), row.country_code.as_deref()) {
        (Some(c), Some(s), _) => Some(format!("{c}, {s}")),
        (Some(c), None, Some(cc)) => Some(format!("{c}, {cc}")),
        (Some(c), None, None) => Some(c.to_string()),
        _ => None,
    }
};
```

### `.sqlx/` offline cache

After modifying both queries, run `bash backend/prepare-sqlx.sh` to regenerate the offline query cache.

---

## Frontend Changes

### `types.ts` — extend `SetRecord`

```typescript
export interface SetRecord {
  // ... existing fields ...
  phase_name: string | null;
  pool_identifier: string | null;
  winner_placement: number | null;
  loser_placement: number | null;
  location: string | null;
  num_entrants: number | null;
}
```

### `SetDetailModal.svelte`

- Replace the flat 2-column grid with three labelled sections.
- Player-name labels for score, seed, and placement cells derived from `currentPlayerName` and `set.opponent_name`.
- `isWin` determines which player's stats are green vs red.
- Ordinal formatting helper: `toOrdinal(n: number): string` — handles `1st/2nd/3rd/nth`.
- Phase display: `set.pool_identifier ? \`${set.phase_name} · ${set.pool_identifier}\` : set.phase_name`.
- Fix set link: `https://www.start.gg/set/${set.startgg_set_id}`.
- "View set on start.gg" label updated from "View on start.gg".

---

## Null handling summary

| Field | When null | Behavior |
|-------|-----------|----------|
| `winner_score` / `loser_score` | both null | Score row omitted |
| `winner_seed` / `loser_seed` | both null | Seed row omitted |
| `phase_name` | null | Phase row omitted |
| `round_name` | null | Round row omitted |
| `location` | null | Location row omitted |
| `num_entrants` | null | Entrants row omitted |
| `winner_placement` / `loser_placement` | both null | Entire Placements section omitted |

---

## Out of scope

- `bracket_type` (from phases or phase_groups) — not surfaced, phase name already implies it
- `total_games` from sets — redundant with score display
- `num_attendees` from tournaments — not relevant at set level
- Lazy-loading or skeleton states — all data arrives with the existing stats payload
