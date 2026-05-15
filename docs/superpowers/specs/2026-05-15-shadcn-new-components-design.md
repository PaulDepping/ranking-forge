---
name: shadcn-new-components
description: Install and apply Empty, Collapsible, Skeleton, Tooltip, ScrollArea, DatePicker, and fix raw labels/trigger in the SvelteKit frontend
metadata:
  type: project
---

# shadcn-svelte: New Components Wave

**Date:** 2026-05-15
**Scope:** Install five new shadcn-svelte components and apply them across the frontend to replace hand-rolled patterns that survived the Phase 1 refactor.

---

## Goal

After Phase 1 (Input, Select, Table, Tabs, Popover, Command, Checkbox), several patterns remain that have proper shadcn equivalents: plain-text empty states, a manual `{#if filterOpen}` panel, raw `<label>` elements, a text loading indicator, `title` attributes on truncated text, overflow-y scroll divs, and native date inputs.

---

## Components to Install

| Component | Install command | Notes |
|---|---|---|
| `empty` | `npx shadcn-svelte@latest add empty` | Structured empty-state layout |
| `collapsible` | `npx shadcn-svelte@latest add collapsible` | Animated expand/collapse |
| `skeleton` | `npx shadcn-svelte@latest add skeleton` | Loading placeholder |
| `tooltip` | `npx shadcn-svelte@latest add tooltip` | Hover/focus label (needs Provider in layout) |
| `scroll-area` | `npx shadcn-svelte@latest add scroll-area` | Scoped scroll container |
| `calendar` | `npx shadcn-svelte@latest add calendar` | Date picker (paired with existing Popover) |

`Label` is already installed. `Popover` is already installed and used for the DatePicker pattern.

---

## Changes by File

### `src/routes/projects/[id]/+layout.svelte`

- Wrap `{@render children()}` (or the outer container) with `<Tooltip.Provider>` so all pages can use `Tooltip` without each page needing its own provider.
- Import: `import * as Tooltip from '$lib/components/ui/tooltip'`

### `src/routes/projects/[id]/tournaments/+page.svelte`

**Collapsible filter panel**
- Replace manual `filterOpen` boolean + `{#if filterOpen}` block with `Collapsible.Root bind:open={filterOpen}` + `Collapsible.Content`.
- The existing "⚙ Filters & Actions" `Button` becomes `Collapsible.Trigger asChild` (or wrap it).
- Animated open/close replaces the instant show/hide.

**Button as Popover.Trigger for bracket filter**
- The bracket-filter trigger currently uses raw Tailwind on `<Popover.Trigger class="rounded-md border px-3 py-1.5 text-sm ...">`.
- Replace with `<Popover.Trigger asChild><Button variant="outline" size="sm" class={...}>...</Button></Popover.Trigger>` so the trigger is a proper `Button`.

**Date Picker for From/To dates**
- Add two pieces of state: `dateFromOpen = $state(false)` and `dateToOpen = $state(false)`.
- Replace each `<Input type="date" bind:value={dateFrom}>` with a Popover + Calendar pattern:
  - `<Popover.Root bind:open={dateFromOpen}>` wrapping a `Button` trigger (shows formatted date or placeholder) and `<Popover.Content><Calendar bind:value={dateFrom} …/></Popover.Content>`.
- The `dateFrom`/`dateTo` state changes from `string` (ISO date string) to a `DateValue` (from `@internationalized/date`) or stays as a string depending on what shadcn Calendar emits — check the shadcn-svelte Calendar API at implementation time.
- The filter predicate in `tournamentVisible` compares dates; update the comparison to match the new type.

**Empty states**
- "No tournaments imported yet. Run an import first." → `<Empty.Root>…</Empty.Root>` with title + description.
- "No tournaments match the current filters." → same pattern.

### `src/routes/projects/[id]/import/+page.svelte`

**Raw `<label>` elements**
- Lines 113 and 117 use bare `<label for="..." class="text-sm font-medium">`. Replace with `<Label for="...">` from `$lib/components/ui/label`.

**Empty state** (none currently, but the page has no empty state — skip)

### `src/routes/projects/[id]/stats/+page.svelte`

**ScrollArea for wins/losses columns**
- Each player card has two `<div class="h-24 overflow-y-auto rounded border border-border bg-muted/20">` containers.
- Replace with `<ScrollArea class="h-24 rounded border border-border bg-muted/20">` + `<ScrollArea.Viewport>` wrapping the button list.

**Empty state**
- "No stats yet. Import tournaments and include some events first." → `<Empty.Root>`.

### `src/routes/projects/[id]/h2h/+page.svelte`

**Skeleton for loading state**
- The `{#if loading}` branch renders a `<div>Loading…</div>` beside the matrix.
- Replace with a `Skeleton` block that approximates the side panel dimensions (e.g. `<Skeleton class="h-48 w-[220px] rounded-md">`).

**Tooltip for truncated player names**
- Column headers and row cells use `<span class="... truncate" title={col.name}>` — the `title` attribute doesn't work on mobile.
- Wrap each truncated span with `<Tooltip.Root><Tooltip.Trigger asChild><span …>…</span></Tooltip.Trigger><Tooltip.Content>{col.name}</Tooltip.Content></Tooltip.Root>`.
- Apply only where the name is actually truncated (i.e. the column headers — `max-w-[5rem]` — and the row label cells — `max-w-[8rem]`).

**Empty state**
- "No head-to-head data yet. Import tournaments first." → `<Empty.Root>`.

### `src/routes/projects/[id]/players/+page.svelte`

**Empty state**
- "No players yet. Add one above." → `<Empty.Root>`.

### `src/routes/projects/+page.svelte`

**Empty state**
- "No projects yet. Create one to get started." → `<Empty.Root>`.

---

## Empty State Design

All empty states follow the same structure:

```svelte
<Empty.Root>
  <Empty.Header>
    <Empty.Title>No X yet</Empty.Title>
    <Empty.Description>Brief instruction on what to do.</Empty.Description>
  </Empty.Header>
</Empty.Root>
```

Where it makes sense to include an action button (e.g. projects page "New project"), use `<Empty.Content>` with a `Button`.

---

## Non-goals

- The bracket-type tri-state row buttons (neutral/required/excluded) are custom domain UI — no shadcn equivalent.
- `SetDetailModal` internal grid layout stays as-is.
- No visual redesign: swaps must be style-neutral.
- No new tests required for pure component swaps. Manually verify the DatePicker and Collapsible in the browser after implementation.

---

## Update `web/CLAUDE.md`

Add newly installed components to the installed-components table in `web/CLAUDE.md`:

| Component | Import path |
|---|---|
| Collapsible | `$lib/components/ui/collapsible` |
| Empty | `$lib/components/ui/empty` |
| Skeleton | `$lib/components/ui/skeleton` |
| Tooltip | `$lib/components/ui/tooltip` |
| Scroll Area | `$lib/components/ui/scroll-area` |
| Calendar | `$lib/components/ui/calendar` |
