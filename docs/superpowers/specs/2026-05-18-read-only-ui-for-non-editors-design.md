# Read-only UI for Non-editors

**Date:** 2026-05-18

## Problem

Users without edit access (viewers on private projects, or anyone on public projects) currently see the full edit UI on the Tournaments and Ranking tabs: event checkboxes, bulk action buttons, drag handles, rank number editing, and the Save button. These controls are cosmetically broken for them — the API will reject writes — but they're confusing and imply an interactive experience that doesn't exist.

## Scope

Two pages: `ranking/+page.svelte` and `tournaments/+page.svelte`.

## Approach

Each page derives a `canEdit` boolean from `data.project.user_role` (already available via the layout server load):

```ts
const canEdit = $derived(
  data.project.user_role === 'editor' || data.project.user_role === 'owner'
);
```

This matches the pattern already used in `+layout.svelte` to filter which tabs are shown.

## Ranking Page Changes

When `!canEdit`, render a static numbered list:

- **Drag handle (`⠿`)**: hidden.
- **`use:dragHandleZone`**: removed from the container (DnD won't bind without the zone action).
- **Rank number**: render as plain `<span>` text instead of the ghost `<Button>` that triggers inline editing.
- **Save button and "Unsaved changes" text**: hidden.

The list of players and their stats (W/L record, win rate) remain visible — those are read-only data.

## Tournaments Page Changes

When `!canEdit`:

- **Event checkboxes**: hidden. The row still shows the event name and entrant count; the `<Label>` wrapper loses `cursor-pointer` and `hover:bg-accent/50` since there's nothing interactive.
- **Bulk action buttons** ("Include all visible" / "Exclude all visible"): hidden. The bulk-actions `<div>` at the bottom of the filter panel is removed.
- **Filter controls** (search, venue, date range, entrants, event type, bracket filter): remain visible. Filters are read-only and useful for navigating tournament data.

The collapsible trigger label changes from "Filters & Actions" to "Filters" when `!canEdit`, since there are no actions available.

## What Does Not Change

- The `+layout.svelte` tab filter (Players and Import are already hidden from non-editors).
- The Settings tab (already owner-only).
- Server-side authorization — the API already rejects unauthorized writes; this is purely a UI improvement.
- No new shared utilities — the `canEdit` derivation is a one-liner repeated in two files.
