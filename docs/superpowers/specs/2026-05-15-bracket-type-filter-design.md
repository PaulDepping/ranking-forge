# Bracket Type Filter Design

**Date:** 2026-05-15
**Scope:** Frontend only — `web/src/routes/projects/[id]/tournaments/+page.svelte` and its filter test file.

## Overview

Replace the "Exclude ladder / matchmaking" checkbox in the tournament filter panel with a richer bracket-type filter. Each of the 10 start.gg bracket types can be independently set to one of three states: **neutral** (don't care), **required** (event must include this type), or **excluded** (event must not include this type).

## Filter Semantics

An event passes the bracket type filter iff:

1. Its `bracket_types` array contains **every** required type (AND logic across required types).
2. Its `bracket_types` array contains **none** of the excluded types (OR logic — any excluded type present causes rejection).
3. Events with an empty `bracket_types` array pass unconditionally (no bracket data = no filter applied), matching existing behaviour.

If no types are required and no types are excluded, the filter is a no-op.

## State Representation

```ts
type BracketTypeState = 'neutral' | 'required' | 'excluded';

const BRACKET_TYPES = [
  'DOUBLE_ELIMINATION',
  'SINGLE_ELIMINATION',
  'ROUND_ROBIN',
  'MATCHMAKING',
  'SWISS',
  // divider in UI
  'EXHIBITION',
  'RACE',
  'CIRCUIT',
  'CUSTOM_SCHEDULE',
  'ELIMINATION_ROUNDS',
] as const;

// State: one entry per type; default is 'neutral' for all
let bracketFilter = $state<Record<string, BracketTypeState>>(
  Object.fromEntries(BRACKET_TYPES.map(t => [t, 'neutral']))
);
```

The old `excludeLadder: boolean` state variable is removed entirely.

## UI — Trigger Button

Placed in row 3 of the filter panel, immediately after the "Event type" select, replacing the old checkbox label.

- **Label text:**
  - All neutral → `Brackets ▾`
  - Active → `{n} req · {m} excl ▾` (omit the zero side if zero, e.g. `2 req ▾` or `1 excl ▾`)
- **Border/colour:** default `border-input` when all neutral; `border-primary text-primary` (or indigo) when any type is active.
- Clicking the trigger toggles the popover open/closed.
- Clicking outside the popover (document `click` listener) closes it.

## UI — Popover

An absolutely-positioned panel that appears below the trigger button.

### Layout

```
[ Bracket Types ]                   [ Reset ]
        –    ✓    ✕
Double Elim  [ ]  [✓]  [ ]
Single Elim  [–]  [ ]  [ ]
Round Robin  [–]  [ ]  [ ]
Matchmaking  [ ]  [ ]  [✕]
Swiss        [–]  [ ]  [ ]
──────────────────────────
Exhibition   [–]  [ ]  [ ]
Race         [–]  [ ]  [ ]
Circuit      [–]  [ ]  [ ]
Custom Sched [–]  [ ]  [ ]
Elim. Rounds [–]  [ ]  [ ]
──────────────────────────
– don't care   ✓ required   ✕ excluded
```

- Column header row labels: `–`, `✓`, `✕`
- A visual divider separates the top 5 common types (DE, SE, RR, Matchmaking, Swiss) from the bottom 5 rarer types.
- Each cell is a clickable button (28×24 px). Clicking the already-active button resets that type back to neutral.
- Active button colours:
  - Neutral (–): indigo tint (`bg-indigo-950 border-indigo-500 text-indigo-400`)
  - Required (✓): green tint (`bg-green-950 border-green-500 text-green-400`)
  - Excluded (✕): red tint (`bg-red-950 border-red-500 text-red-400`)
  - Inactive cell: muted (`bg-muted/30 border-border`)
- **Reset** link (top-right of panel) sets all types back to `neutral`.
- A short legend line at the bottom of the panel explains the three states.

### Popover placement and dismissal

- Positioned `absolute; top: calc(100% + 4px); left: 0; z-index: 50`.
- Implemented with a Svelte `$state` boolean (`popoverOpen`) — no library component needed.
- A `use:clickOutside` action (or equivalent `document.addEventListener('click', …)` in an `$effect`) closes the popover when the user clicks outside.

## Filter Logic (eventVisible)

Remove the `excludeLadder` parameter. Add a `bracketFilter` parameter:

```ts
function eventVisible(
  e: TournamentEvent,
  t: Tournament,
  search: string,
  minEntrants: number | null,
  maxEntrants: number | null,
  eventType: 'all' | 'singles' | 'teams',
  bracketFilter: Record<string, BracketTypeState>,
): boolean {
  // ... existing checks unchanged ...

  // Bracket type filter
  const required = Object.entries(bracketFilter)
    .filter(([, s]) => s === 'required')
    .map(([t]) => t);
  const excluded = Object.entries(bracketFilter)
    .filter(([, s]) => s === 'excluded')
    .map(([t]) => t);

  if (required.length > 0 || excluded.length > 0) {
    if (e.bracket_types.length === 0) return true; // no data → pass

    for (const r of required) {
      if (!e.bracket_types.includes(r)) return false;
    }
    for (const x of excluded) {
      if (e.bracket_types.includes(x)) return false;
    }
  }

  return true;
}
```

## Testing

Update `filter.test.ts`:

- Remove the `excludeLadder: boolean` parameter from the standalone `eventVisible` function and all call sites.
- Add `bracketFilter: Record<string, BracketTypeState>` parameter.
- Add test cases:
  - Required type present → passes
  - Required type absent → filtered out
  - Excluded type present → filtered out
  - Excluded type absent → passes
  - Event with empty `bracket_types` passes regardless of filter state
  - Multiple required types: event must have all of them
  - Required + excluded on different types: event with both required and excluded type is rejected
  - All neutral → no filtering (existing events pass)

## Files Changed

| File | Change |
|---|---|
| `web/src/routes/projects/[id]/tournaments/+page.svelte` | Replace `excludeLadder` state + checkbox with `bracketFilter` state + popover UI |
| `web/src/routes/projects/[id]/tournaments/filter.test.ts` | Update `eventVisible` signature and add bracket filter test cases |
