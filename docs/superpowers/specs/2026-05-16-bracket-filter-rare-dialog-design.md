# Bracket Filter: Rare Types Dialog

**Date:** 2026-05-16
**Status:** Approved

## Problem

The bracket type filter popover contains 10 rows plus headers and a legend (~300 px tall). Floating UI's collision detection flips it upward when there is insufficient space below the trigger, and the popover can overflow the top of the viewport when there is also insufficient space above.

## Solution

Keep the 5 common bracket types directly in the popover. Replace the existing rare-types section with a single "All bracket types…" link button that opens a `Dialog` containing all 10 types. Dialogs are always viewport-centred, so overflow is impossible regardless of trigger position.

## Design

### Popover (unchanged except rare rows removed)

- Shows the 5 common types: Double Elim, Single Elim, Round Robin, Matchmaking, Swiss
- Keeps existing column headers (– / ✓ / ✕) and three-state buttons per row
- Keeps existing "Bracket Types" label and Reset button in the header
- Keeps existing legend
- Replaces the divider + 5 rare rows with a single "All bracket types…" link button

**Badge on the link button:** when any rare type has a non-neutral state (required or excluded), the button turns green and shows a count badge (e.g. "1 active"). This makes it discoverable that a rare-type filter is in effect without requiring the user to open the dialog.

### Dialog

- Title: "Bracket Types"
- Lists all 10 types in the same three-state grid layout as the popover
- A small "Rare formats" section label separates the 5 common rows from the 5 rare rows
- Shares `bracketFilter` reactive state directly — changes apply immediately without a confirm step
- Single "Done" button closes the dialog

### State

One new boolean: `bracketDialogOpen`. Everything else (`bracketFilter`, `bracketPopoverOpen`, `bracketReqCount`, `bracketExclCount`) stays as-is.

The derived `bracketTriggerLabel` on the main "Brackets ▾" button already counts req/excl across all 10 types, so it continues to reflect rare-type activity without changes.

A new derived value `rareActiveCount` counts how many rare types have a non-neutral state — used for the badge text.

### Reset behaviour

The existing Reset button in the popover header resets all 10 types (current behaviour, unchanged). There is no separate reset inside the dialog.

## Files changed

- `web/src/routes/projects/[id]/tournaments/+page.svelte` — only file that needs editing
