# shadcn-svelte Audit: Replace Hand-Rolled Components

**Date:** 2026-05-20  
**Scope:** Four targeted replacements across three files, plus one new component install.

## Background

An audit of the frontend found four places where hand-rolled Tailwind replicates what installed (or easily installable) shadcn-svelte components already provide. These violate the component policy in `web/CLAUDE.md`.

Ranking rows (`ranking/+page.svelte`) were considered but excluded: tight DnD list rows have no fitting shadcn equivalent and the DnD library (`svelte-dnd-action`) needs raw elements.

---

## Change 1 — Tournament list items → Card

**File:** `web/src/routes/projects/[id]/tournaments/+page.svelte`

Each tournament container is currently `<div class="rounded-md border border-border">` with a header div and an events div. Replace with:

- `<Card.Root>` as the outer wrapper
- `<Card.Header>` for the tournament name/location/date row
- `<Card.Title>` for tournament name
- `<Card.Description>` for the location + date line
- `<Card.Action>` for the event-count Badge (already in Card.Header)
- `<Card.Content class="p-0">` wrapping the events `<div class="divide-y divide-border border-t border-border">` so dividers render flush

---

## Change 2 — Invite link items → Card

**File:** `web/src/routes/projects/[id]/settings/+page.svelte`

Each invite link row is currently `<div class="flex items-center justify-between rounded-md border p-3 gap-2">`. Replace with:

- `<Card.Root>` as the outer wrapper
- `<Card.Header>` containing:
  - `<Card.Title class="capitalize">` for the role label
  - `<Card.Description>` for the expiry line (when present)
  - `<Card.Action>` for the Copy link + Revoke buttons

---

## Change 3 — Sort toggle → ToggleGroup

**File:** `web/src/lib/components/TournamentTab.svelte`

The Placement / Seed sort buttons are currently two `<Button>` elements with manually switched `variant` inside a `<div class="flex rounded-md border overflow-hidden">`. Replace with:

1. Install: `npx shadcn-svelte@latest add --yes --overwrite toggle-group`
2. Import `* as ToggleGroup from '$lib/components/ui/toggle-group'`
3. Replace the button div with:
   ```svelte
   <ToggleGroup.Root type="single" bind:value={sortMode}>
     <ToggleGroup.Item value="placement">Placement</ToggleGroup.Item>
     <ToggleGroup.Item value="seed">Seed</ToggleGroup.Item>
   </ToggleGroup.Root>
   ```
4. Update `web/CLAUDE.md` installed-components table to add Toggle Group.

The `sortMode` state variable and the `{#if activeTab !== 'all'}` conditional wrapper stay unchanged.

---

## Change 4 — H2H popover header → Popover sub-components

**File:** `web/src/routes/projects/[id]/h2h/+page.svelte`

The popover content currently wraps the matchup title and record in raw `<p>` tags inside a plain `<div>`. Use the already-installed `Popover` sub-components for the text content:

- The outer `<div class="mb-3 flex items-start justify-between gap-2 border-b border-border pb-2">` stays (needed for the horizontal layout with the close button).
- The inner `<div>` holding the two `<p>` tags is replaced with `<Popover.Header>`.
- `<p class="font-semibold text-sm">` → `<Popover.Title class="text-sm font-semibold">` (`Popover.Title` applies `font-medium` by default; `font-semibold` overrides to match the current weight).
- `<p class="text-xs text-muted-foreground">` → `<Popover.Description class="text-xs">` (`Popover.Description` applies `text-muted-foreground` by default).
- The `<Button variant="ghost" size="icon" onclick={() => (selectedPair = null)} aria-label="Close">×</Button>` close button stays unchanged. `Popover.Close` is an unstyled primitive — the existing controlled-Popover approach (setting `selectedPair = null` triggers `open` to become false) is correct and should not be changed.

---

## Out of Scope

- **Ranking rows** (`ranking/+page.svelte`): DnD requires raw elements; no fitting shadcn list-item component exists.
- **HandleTab results list**: Simple `divide-y` list with no shadcn equivalent.
- **All form fields, dialogs, tables, tabs**: Already using shadcn correctly.
