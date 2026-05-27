# Wide Page Heading Alignment

**Date:** 2026-05-22
**Status:** Approved

## Problem

The H2H and Stats pages use `page.data.wide = true`, which causes the project layout to constrain the title/tabs area to `max-w-5xl mx-auto px-4` while leaving `{@render children()}` full-width. Both pages wrap all their content (heading + body) in a single `<div class="space-y-4 px-4">`, so the `<h2>` sits at the far-left edge of the viewport rather than aligned with the nav above it.

## Solution (Option A — per-page heading wrapper)

Split the single outer div in each wide page into two sibling sections:

1. **Heading zone**: `<div class="mx-auto max-w-5xl px-4">` — contains only the `<h2>`.
2. **Body zone**: existing content (table / stats grid) with no width constraint.

The outer wrapper keeps `space-y-4` for spacing. The empty state falls in the body zone; its centered content is visually unaffected by the width.

## Files changed

- `web/src/routes/projects/[id]/h2h/+page.svelte`
- `web/src/routes/projects/[id]/stats/+page.svelte`

## Constraints

- `max-w-5xl` matches the value in `+layout.svelte:40` (the project title/tabs constraint).
- No layout changes, no new abstractions.
- The H2H table's `overflow-x-auto` wrapper remains unchanged and continues to scroll horizontally on narrow viewports.
