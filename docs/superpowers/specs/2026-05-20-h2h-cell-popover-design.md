# H2H Cell Popover Design

**Date:** 2026-05-20
**Status:** Approved

## Problem

The head-to-head matrix currently shows set details in a side panel Card that sits to the right of the table in a flex row. For large tables this panel is far from the clicked cell, or wraps below the table entirely due to `flex-wrap`, making it easy to miss.

## Solution

Replace the side panel Card with a shadcn `Popover` anchored directly to the clicked cell. The popover floats over the table near the click point, is dismissed by clicking elsewhere, and auto-flips to stay within the viewport.

## Architecture

### State (unchanged)

`selectedPair` and `loading` remain as-is — one piece of shared state controls which cell's popover is open. `isSelected(row.id, col.id)` returns true for at most one cell at a time.

### Per-cell Popover

Each non-diagonal cell that has a record gets a `Popover.Root` wrapping its trigger button and content:

```svelte
<Popover.Root
  open={isSelected(row.id, col.id)}
  onOpenChange={(v) => { if (!v) selectedPair = null; }}
>
  <Popover.Trigger>
    {#snippet child({ props })}
      <Button {...props} onclick={() => selectCell(row, col)} ...>
        {rec.wins}–{rec.losses}
      </Button>
    {/snippet}
  </Popover.Trigger>
  <Popover.Content side="right" align="start" class="w-64 p-0">
    <!-- detail content -->
  </Popover.Content>
</Popover.Root>
```

`side="right"` is the preferred direction; floating-ui flips to left/top/bottom automatically when the viewport edge is too close.

### Popover content

The popover content mirrors the existing side panel Card:
- Header: "{rowPlayer} vs {colPlayer}" + "N wins · N losses" + close button
- Set list: W/L badge, score, tournament name, round (same Button rows as now)
- Loading state: show `Skeleton` elements inside the popover while `loading && isSelected()`
- Empty state: "No sets found." message
- Footer: "Click a row for full details" hint

Clicking a set row still sets `selectedSet` and opens `SetDetailModal` — no change there.

### Closing

- Click outside the popover → `onOpenChange(false)` → clears `selectedPair`
- Click the same cell again → `selectCell` early-returns and sets `selectedPair = null` (existing toggle logic)
- Click the × button inside → `selectedPair = null`

### Layout change

The outer `<div class="flex gap-4 items-start flex-wrap">` wrapping table + side panel becomes just the table alone — no sibling Card. The popover overlays the table in screen space via `position: fixed` (floating-ui default), so it is not clipped by the table's overflow.

## Components affected

- `web/src/routes/projects/[id]/h2h/+page.svelte` — primary change
  - Import `* as Popover` from `$lib/components/ui/popover`
  - Wrap each record Button with `Popover.Root` / `Popover.Trigger` / `Popover.Content`
  - Remove the side panel Card and loading skeleton sibling div
  - Keep all state variables and `selectCell` logic unchanged

`Popover` is already installed (`src/lib/components/ui/popover`).

## Out of scope

- Changing the `SetDetailModal` that opens when clicking a set row
- Any changes to backend routes or data fetching
- Keyboard navigation improvements (existing behaviour is preserved)
