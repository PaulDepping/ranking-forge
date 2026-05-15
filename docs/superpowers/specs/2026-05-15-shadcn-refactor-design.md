# shadcn-svelte Full Coverage Refactor

**Date:** 2026-05-15
**Scope:** Replace all hand-rolled HTML elements and raw inputs with shadcn-svelte components across the SvelteKit frontend.

---

## Goal

Maximize use of shadcn-svelte components throughout the frontend so every interactive primitive (inputs, selects, checkboxes, tabs, popovers, tables, cards) comes from the library rather than being hand-rolled with manual Tailwind classes. This ensures consistent styling, accessibility attributes, and dark-mode behavior project-wide.

---

## Audit: What Changes

### Already-installed components used inconsistently

These components exist in `src/lib/components/ui/` but are not applied everywhere they should be.

| Component | Files with hand-rolled equivalents |
|---|---|
| `Input` | `routes/projects/[id]/tournaments/+page.svelte` — search text input, min/max entrant number inputs, date-from/date-to inputs; `routes/projects/[id]/import/+page.svelte` — after_date and before_date date inputs (currently carry 5 manual Tailwind classes that duplicate what `Input` already provides) |
| `Select` | `routes/projects/[id]/tournaments/+page.svelte` — venue filter `<select>`, event-type filter `<select>` |
| `Table` | `routes/projects/[id]/h2h/+page.svelte` — the full H2H matrix `<table>` |
| `Button` | `routes/+layout.svelte` — logout raw `<button>`; `routes/projects/[id]/tournaments/+page.svelte` — "Clear filters" and bracket-popover "Reset" raw `<button>` elements; `routes/projects/[id]/h2h/+page.svelte` — close-panel `<button>×</button>` |
| `Card` | `routes/projects/[id]/import/+page.svelte` — job status box; `routes/projects/[id]/stats/+page.svelte` — each player stat card; `routes/projects/[id]/h2h/+page.svelte` — selected-pair side panel |

### New components to install

These are not yet present in `src/lib/components/ui/` and require `npx shadcn-svelte@latest add <name>`.

| Component | File and usage |
|---|---|
| `Checkbox` | `routes/projects/[id]/tournaments/+page.svelte` — event include/exclude `<input type="checkbox">` wrapped in a `<label>` |
| `Tabs` | `routes/projects/[id]/+layout.svelte` — the 5 navigation links (Players, Import, Tournaments, Stats, H2H) currently rendered as styled `<a>` elements |
| `Popover` | `routes/projects/[id]/tournaments/+page.svelte` — bracket-type filter popover, currently hand-rolled with a `$effect` for click-outside detection and manual absolute positioning |
| `Command` | `routes/projects/new/+page.svelte` — game search autocomplete, currently a hand-rolled `<ul>/<li>/<button>` dropdown |

---

## Order of Work

### Wave 1 — use already-installed components (no new dependencies)

**PR 1 — Input everywhere it's missing**
- Tournaments filter panel: replace `<input type="text">` (search), `<input type="number">` (min/max entrants), and both `<input type="date">` elements with `<Input>`.
- Import page: replace both `<input type="date">` elements (which carry manual `flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring` classes) with `<Input type="date">`.

**PR 2 — Select for filter dropdowns**
- Tournaments: replace `<select bind:value={venueFilter}>` and `<select bind:value={eventType}>` with the shadcn `Select` component (`Select`, `SelectTrigger`, `SelectValue`, `SelectContent`, `SelectItem`).
- Note: shadcn `Select` uses `onValueChange` rather than `bind:value`; update filter state accordingly.

**PR 3 — Table for H2H matrix**
- H2H page: replace `<table>`, `<thead>`, `<tbody>`, `<tr>`, `<th>`, `<td>` with `Table`, `TableHeader`, `TableBody`, `TableRow`, `TableHead`, `TableCell` from `$lib/components/ui/table`. The interactive cell buttons inside `<td>` remain unchanged.

**PR 4 — Button + Card for misc raw elements**
- Button replacements:
  - Layout header: `<button onclick={logout}>Logout</button>` → `<Button variant="ghost" size="sm">Logout</Button>`
  - Tournaments "Clear filters": raw `<button>` → `<Button variant="ghost" size="sm">`
  - Tournaments bracket-popover "Reset": raw `<button>` → `<Button variant="ghost" size="sm">`
  - H2H close panel `<button>×</button>` → `<Button variant="ghost" size="icon">×</Button>`
- Card replacements:
  - Import job status box `div.rounded-md.border.p-4` → `<Card><CardContent class="pt-4">…</CardContent></Card>`
  - Stats each player card `div.rounded-md.border.p-3` → `<Card><CardContent class="p-3">…</CardContent></Card>`
  - H2H side panel `div.rounded-md.border.p-3` → `<Card><CardContent class="p-3">…</CardContent></Card>`

### Wave 2 — add new components

**PR 5 — Checkbox for event toggles**
- Install: `npx shadcn-svelte@latest add checkbox`
- Tournaments: replace each `<input type="checkbox" checked={event.included} onchange={...}>` inside a `<label>` with `<Checkbox checked={event.included} onCheckedChange={(v) => handleToggle(data.project.id, event)} />`. Keep the surrounding `<label>` or convert to a flex row if the Checkbox primitive makes the label redundant.

**PR 6 — Tabs for project navigation**
- Install: `npx shadcn-svelte@latest add tabs`
- `[id]/+layout.svelte`: replace the hand-rolled `<nav>` with `<Tabs value={currentTab}>` + `<TabsList>` + one `<TabsTrigger>` per tab.
- Bind `value` to the current route segment (derived from `page.url.pathname`). Use `onValueChange` to call `goto(tabHref(value))` for navigation. `TabsContent` is not used — SvelteKit's `{@render children()}` handles page content.

**PR 7 — Popover for bracket-type filter**
- Install: `npx shadcn-svelte@latest add popover`
- Tournaments: replace the `div#bracket-popover-wrapper` + `$effect` click-outside handler with `<Popover bind:open={bracketPopoverOpen}>` + `<PopoverTrigger>` (the existing trigger button) + `<PopoverContent>` (the bracket rows panel). The `$effect` click-outside listener is deleted entirely — `Popover` handles dismissal natively.

**PR 8 — Command for game search combobox**
- Install: `npx shadcn-svelte@latest add command`
- New project page: replace the hand-rolled `<ul>/<li>/<button>` autocomplete dropdown with a `Popover` + `Command` + `CommandInput` + `CommandList` + `CommandItem` pattern. The search-on-type fetch logic (`onGameInput`, debounce, `gameResults`) is retained; `CommandInput` feeds the query and `CommandItem` fires `selectGame`. The hidden `<input name="game_id">` and `<input name="game_name">` remain for form submission.

---

## Non-goals

- The bracket-type row buttons (neutral/required/excluded tri-state) are domain-specific and intentionally left as custom elements — no shadcn equivalent exists.
- The `SetDetailModal` internal layout (`div.grid`, `div.space-y-4`) stays as-is; it already uses `Dialog`.
- No visual redesign: component swaps should be style-neutral (same look, better primitives).

---

## Testing

Each PR should leave all existing Vitest unit tests and Playwright e2e tests passing. No new tests are required for pure component swaps. The `Tabs` PR (PR 6) touches navigation logic and warrants a quick manual check of tab switching in the browser. The `Command` PR (PR 8) changes the game-search UX and should be manually verified with the dev server.
