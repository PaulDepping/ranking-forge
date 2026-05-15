# Reset All Filters Button — Design Spec

**Date:** 2026-05-15
**Status:** Approved

## Overview

Add a "Clear filters" button to the tournament page's collapsible filter panel that resets all six filter dimensions to their defaults in a single action.

## Scope

- File: `web/src/routes/projects/[id]/tournaments/+page.svelte`
- No backend changes required.

## Filter Dimensions Reset

All six filter state variables are reset:

| Variable | Default |
|---|---|
| `search` | `''` |
| `venueFilter` | `'all'` |
| `minEntrants` | `null` |
| `maxEntrants` | `null` |
| `dateFrom` | `''` |
| `dateTo` | `''` |
| `eventType` | `'all'` |
| `bracketFilter` | all entries `'neutral'` |

## UI Placement

A header row is added at the top of the collapsible filter panel (`#if filterOpen` block), spanning the full width with two elements:

- **Left:** `"Filters"` label (`text-xs font-medium text-muted-foreground uppercase tracking-wide`)
- **Right:** `"Clear filters"` button (`text-xs text-muted-foreground hover:text-foreground`)

This mirrors the existing pattern in the bracket popover ("Bracket Types" label + "Reset" button).

The button is **always visible** when the panel is open — not conditionally rendered based on active filter state.

## Implementation

### `resetAllFilters()` function

Add a new function in the `<script>` block:

```ts
function resetAllFilters() {
    search      = '';
    venueFilter = 'all';
    minEntrants = null;
    maxEntrants = null;
    dateFrom    = '';
    dateTo      = '';
    eventType   = 'all';
    bracketFilter = Object.fromEntries(
        BRACKET_TYPES.map(t => [t, 'neutral' as BracketTypeState])
    );
}
```

The bracket reset logic is inlined here (identical to the existing `resetBracketFilter()`), so `resetBracketFilter()` is no longer needed separately and can be removed — the bracket popover's "Reset" button can call `resetAllFilters()` instead, or the bracket popover can keep its own dedicated reset. Either is acceptable; keeping `resetBracketFilter()` avoids a behavior change (the bracket popover reset would then clear all filters, not just brackets). **Decision: keep `resetBracketFilter()` as-is for the bracket popover; `resetAllFilters()` is a separate function.**

### Header row markup

Inserted as the first child inside the filter panel `div`:

```svelte
<div class="flex items-center justify-between">
    <span class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Filters</span>
    <button
        type="button"
        onclick={resetAllFilters}
        class="text-xs text-muted-foreground hover:text-foreground"
    >Clear filters</button>
</div>
```

## Out of Scope

- No change to the bracket popover's internal "Reset" button behavior.
- No "filters active" indicator or conditional visibility for the Clear button.
- No backend changes.
