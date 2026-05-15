# shadcn-svelte Full Coverage Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace every hand-rolled HTML primitive (`<input>`, `<select>`, `<button>`, `<table>`, etc.) with shadcn-svelte components across the SvelteKit frontend.

**Architecture:** 8 sequential tasks in two waves. Wave 1 uses already-installed components — no new installs, no risk. Wave 2 installs new components one at a time (`npx shadcn-svelte@latest add <name>`) then applies them. Each task touches one component type across all affected files.

**Tech Stack:** SvelteKit 5 (runes), shadcn-svelte v1, bits-ui v2, Tailwind CSS v4.

---

## Files changed

| File | Tasks |
|---|---|
| `web/src/routes/projects/[id]/tournaments/+page.svelte` | 1, 2, 4, 5, 7 |
| `web/src/routes/projects/[id]/import/+page.svelte` | 1, 4 |
| `web/src/routes/projects/[id]/h2h/+page.svelte` | 3, 4 |
| `web/src/routes/projects/[id]/stats/+page.svelte` | 4 |
| `web/src/routes/+layout.svelte` | 4 |
| `web/src/routes/projects/[id]/+layout.svelte` | 6 |
| `web/src/routes/projects/new/+page.svelte` | 8 |
| `web/CLAUDE.md` | 5, 6, 7, 8 (update installed-components list) |

---

## Task 1: Input — tournaments filter panel + import date fields

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`
- Modify: `web/src/routes/projects/[id]/import/+page.svelte`

- [ ] **Step 1: Add Input import to tournaments page**

At the top of the `<script>` in `tournaments/+page.svelte`, add:
```svelte
import { Input } from "$lib/components/ui/input";
```
(alongside the existing Badge and Button imports)

- [ ] **Step 2: Replace the five raw inputs in the filter panel**

Find and replace each raw `<input>` in the collapsible filter panel (`{#if filterOpen}`):

Search box (Row 1):
```svelte
<!-- REMOVE -->
<input
    type="text"
    placeholder="Search tournament or event name…"
    bind:value={search}
    class="flex-1 min-w-48 rounded-md border border-input bg-background px-3 py-1.5 text-sm"
/>
<!-- ADD -->
<Input
    type="text"
    placeholder="Search tournament or event name…"
    bind:value={search}
    class="flex-1 min-w-48"
/>
```

Min entrants (Row 2):
```svelte
<!-- REMOVE -->
<input
    type="number"
    min="0"
    placeholder="min"
    bind:value={minEntrants}
    class="w-20 rounded-md border border-input bg-background px-2 py-1.5 text-sm"
/>
<!-- ADD -->
<Input type="number" min="0" placeholder="min" bind:value={minEntrants} class="w-20" />
```

Max entrants (Row 2):
```svelte
<!-- REMOVE -->
<input
    type="number"
    min="0"
    placeholder="max"
    bind:value={maxEntrants}
    class="w-20 rounded-md border border-input bg-background px-2 py-1.5 text-sm"
/>
<!-- ADD -->
<Input type="number" min="0" placeholder="max" bind:value={maxEntrants} class="w-20" />
```

Date from (Row 2):
```svelte
<!-- REMOVE -->
<input
    type="date"
    bind:value={dateFrom}
    class="rounded-md border border-input bg-background px-2 py-1.5 text-sm"
/>
<!-- ADD -->
<Input type="date" bind:value={dateFrom} class="w-auto" />
```

Date to (Row 2):
```svelte
<!-- REMOVE -->
<input
    type="date"
    bind:value={dateTo}
    class="rounded-md border border-input bg-background px-2 py-1.5 text-sm"
/>
<!-- ADD -->
<Input type="date" bind:value={dateTo} class="w-auto" />
```

- [ ] **Step 3: Replace the two raw date inputs in the import page**

In `import/+page.svelte`, add Input to the imports:
```svelte
import { Input } from '$lib/components/ui/input';
```

Replace both date inputs (they each carry the full `flex h-9 w-full rounded-md border...` class string):

```svelte
<!-- REMOVE -->
<input
    id="after_date"
    name="after_date"
    type="date"
    class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
/>
<!-- ADD -->
<Input id="after_date" name="after_date" type="date" />
```

```svelte
<!-- REMOVE -->
<input
    id="before_date"
    name="before_date"
    type="date"
    class="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
/>
<!-- ADD -->
<Input id="before_date" name="before_date" type="date" />
```

- [ ] **Step 4: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass (no behaviour changed, only markup).

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/[id]/tournaments/+page.svelte \
        web/src/routes/projects/[id]/import/+page.svelte
git commit -m "refactor(web): use Input component in filter panel and import dates"
```

---

## Task 2: Select — venue and event-type filter dropdowns

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Add Select import and label-lookup deriveds**

Add to the `<script>` imports:
```svelte
import * as Select from "$lib/components/ui/select";
```

Add these two derived label lookups after the existing `$derived` blocks:
```svelte
const venueLabel = $derived(
    ({ all: "Venue: All", online: "Online only", offline: "Offline only" } as const)[venueFilter]
);
const eventTypeLabel = $derived(
    ({ all: "All types", singles: "Singles", teams: "Teams" } as const)[eventType]
);
```

- [ ] **Step 2: Replace the venue `<select>`**

```svelte
<!-- REMOVE -->
<select
    bind:value={venueFilter}
    class="rounded-md border border-input bg-background px-3 py-1.5 text-sm"
>
    <option value="all">Venue: All</option>
    <option value="online">Online only</option>
    <option value="offline">Offline only</option>
</select>

<!-- ADD -->
<Select.Root bind:value={venueFilter}>
    <Select.Trigger class="w-36">{venueLabel}</Select.Trigger>
    <Select.Content>
        <Select.Item value="all">Venue: All</Select.Item>
        <Select.Item value="online">Online only</Select.Item>
        <Select.Item value="offline">Offline only</Select.Item>
    </Select.Content>
</Select.Root>
```

- [ ] **Step 3: Replace the event-type `<select>`**

```svelte
<!-- REMOVE -->
<select
    bind:value={eventType}
    class="rounded-md border border-input bg-background px-3 py-1.5 text-sm"
>
    <option value="all">All</option>
    <option value="singles">Singles</option>
    <option value="teams">Teams</option>
</select>

<!-- ADD -->
<Select.Root bind:value={eventType}>
    <Select.Trigger class="w-28">{eventTypeLabel}</Select.Trigger>
    <Select.Content>
        <Select.Item value="all">All types</Select.Item>
        <Select.Item value="singles">Singles</Select.Item>
        <Select.Item value="teams">Teams</Select.Item>
    </Select.Content>
</Select.Root>
```

- [ ] **Step 4: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/[id]/tournaments/+page.svelte
git commit -m "refactor(web): use Select component for venue and event-type filters"
```

---

## Task 3: Table — H2H matrix

**Files:**
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`

- [ ] **Step 1: Add Table import**

Add to the `<script>` imports:
```svelte
import * as Table from "$lib/components/ui/table";
```

- [ ] **Step 2: Replace the raw `<table>` with Table components**

The table lives inside `<div class="overflow-x-auto">`. Replace the entire `<table>…</table>` block (the outer `<div class="overflow-x-auto">` wrapper is removed because `Table.Root` already wraps in a scrollable container):

```svelte
<!-- REMOVE the outer <div class="overflow-x-auto"> and inner <table> -->
<div class="overflow-x-auto">
    <table class="border-collapse text-sm">
        <thead>
            <tr>
                <th class="w-32 pb-2 pr-3 text-left font-normal text-muted-foreground"></th>
                {#each data.players as col (col.id)}
                    <th class="px-2 pb-2 text-center font-medium" style="min-width:5rem">
                        <span class="block max-w-[5rem] truncate" title={col.name}>{col.name}</span>
                    </th>
                {/each}
            </tr>
        </thead>
        <tbody>
            {#each data.players as row (row.id)}
                <tr>
                    <td class="max-w-[8rem] truncate py-1 pr-3 font-medium" title={row.name}>{row.name}</td>
                    {#each data.players as col (col.id)}
                        ...
                    {/each}
                </tr>
            {/each}
        </tbody>
    </table>
    <p class="mt-1 text-xs text-muted-foreground">Row player's record vs. column player</p>
</div>

<!-- ADD (Table.Root provides the overflow-x-auto wrapper internally) -->
<div>
    <Table.Root class="border-collapse">
        <Table.Header>
            <Table.Row>
                <Table.Head class="w-32 pb-2 pr-3 font-normal text-muted-foreground h-auto"></Table.Head>
                {#each data.players as col (col.id)}
                    <Table.Head class="px-2 pb-2 text-center font-medium h-auto" style="min-width:5rem">
                        <span class="block max-w-[5rem] truncate" title={col.name}>{col.name}</span>
                    </Table.Head>
                {/each}
            </Table.Row>
        </Table.Header>
        <Table.Body>
            {#each data.players as row (row.id)}
                <Table.Row class="border-0 hover:bg-transparent">
                    <Table.Cell class="max-w-[8rem] truncate py-1 pr-3 font-medium" title={row.name}>{row.name}</Table.Cell>
                    {#each data.players as col (col.id)}
                        {#if row.id === col.id}
                            <Table.Cell class="px-2 py-1 text-center text-muted-foreground">—</Table.Cell>
                        {:else}
                            {@const rec = getRecord(row.id, col.id)}
                            <Table.Cell class="px-2 py-1 text-center tabular-nums">
                                {#if rec}
                                    <button
                                        class="rounded px-1
                                            {isSelected(row.id, col.id)
                                                ? 'ring-2 ring-primary bg-primary/10'
                                                : rec.wins > rec.losses
                                                    ? 'bg-green-50 dark:bg-green-950/30 text-green-700 dark:text-green-400'
                                                    : rec.wins < rec.losses
                                                        ? 'bg-red-50 dark:bg-red-950/30 text-red-700 dark:text-red-400'
                                                        : ''}"
                                        onclick={() => selectCell(row, col)}
                                    >
                                        {rec.wins}–{rec.losses}
                                    </button>
                                {:else}
                                    <span class="text-muted-foreground">—</span>
                                {/if}
                            </Table.Cell>
                        {/if}
                    {/each}
                </Table.Row>
            {/each}
        </Table.Body>
    </Table.Root>
    <p class="mt-1 text-xs text-muted-foreground">Row player's record vs. column player</p>
</div>
```

Note: `Table.Head` adds `h-10` by default; `h-auto` overrides it to match the original. `Table.Row` adds a bottom border and hover state; `border-0 hover:bg-transparent` disables both to preserve the matrix look.

- [ ] **Step 3: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/projects/[id]/h2h/+page.svelte
git commit -m "refactor(web): use Table components for H2H matrix"
```

---

## Task 4: Button + Card — misc raw elements

**Files:**
- Modify: `web/src/routes/+layout.svelte`
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`
- Modify: `web/src/routes/projects/[id]/import/+page.svelte`
- Modify: `web/src/routes/projects/[id]/stats/+page.svelte`

### Button replacements

- [ ] **Step 1: Replace logout button in layout**

In `routes/+layout.svelte`, add Button import:
```svelte
import { Button } from '$lib/components/ui/button';
```

Replace the raw logout button:
```svelte
<!-- REMOVE -->
<button
    onclick={logout}
    class="text-sm text-muted-foreground hover:text-foreground"
>Logout</button>

<!-- ADD -->
<Button variant="ghost" size="sm" onclick={logout}>Logout</Button>
```

- [ ] **Step 2: Replace "Clear filters" and bracket "Reset" buttons in tournaments**

In `tournaments/+page.svelte` (Button is already imported), find the two raw buttons inside the filter panel:

```svelte
<!-- REMOVE "Clear filters" -->
<button
    type="button"
    onclick={resetAllFilters}
    class="text-xs text-muted-foreground hover:text-foreground"
    >Clear filters</button
>
<!-- ADD -->
<Button type="button" variant="ghost" size="sm" onclick={resetAllFilters}>Clear filters</Button>
```

```svelte
<!-- REMOVE bracket "Reset" -->
<button
    type="button"
    onclick={resetBracketFilter}
    class="text-xs text-muted-foreground hover:text-foreground"
    >Reset</button
>
<!-- ADD -->
<Button type="button" variant="ghost" size="sm" onclick={resetBracketFilter}>Reset</Button>
```

- [ ] **Step 3: Replace H2H close-panel button**

In `h2h/+page.svelte`, add Button import:
```svelte
import { Button } from '$lib/components/ui/button';
```

Replace the `×` close button:
```svelte
<!-- REMOVE -->
<button
    class="text-muted-foreground hover:text-foreground text-lg leading-none"
    onclick={() => (selectedPair = null)}
    aria-label="Close panel"
>×</button>

<!-- ADD -->
<Button
    variant="ghost"
    size="icon"
    onclick={() => (selectedPair = null)}
    aria-label="Close panel"
>×</Button>
```

### Card replacements

- [ ] **Step 4: Replace import job-status box with Card**

In `import/+page.svelte`, add Card imports:
```svelte
import * as Card from '$lib/components/ui/card';
```

Replace the job-status `div`:
```svelte
<!-- REMOVE -->
<div class="rounded-md border border-border p-4 space-y-2">
    ...job status content...
</div>

<!-- ADD (Card provides rounded-xl, ring-1, bg-card; CardContent adds px-6 by default, overridden here) -->
<Card.Root class="py-0">
    <Card.Content class="p-4 space-y-2">
        <!-- Move all content from the removed <div> here unchanged:
             the flex row with Status/Badge/polling span, optional error <p>,
             the "Started …" <p>, and the optional Retry <form> -->
    </Card.Content>
</Card.Root>
```

- [ ] **Step 5: Replace stats player cards with Card**

In `stats/+page.svelte`, add Card imports:
```svelte
import * as Card from '$lib/components/ui/card';
```

Replace each player-stat `div` (inside `{#each data.stats as player}`):
```svelte
<!-- REMOVE -->
<div class="rounded-md border border-border p-3">
    ...player stat content...
</div>

<!-- ADD -->
<Card.Root class="py-0">
    <Card.Content class="p-3">
        <!-- Move all content from the removed <div> here unchanged:
             the mb-2 flex row (player name + W/L/rate), the flex gap-2 row
             containing the WINS column and LOSSES column each with their
             h-24 scrollable lists of set buttons -->
    </Card.Content>
</Card.Root>
```

- [ ] **Step 6: Replace H2H side panel with Card**

In `h2h/+page.svelte`, add Card imports:
```svelte
import * as Card from '$lib/components/ui/card';
```

Replace the side panel `div` (the `{:else if selectedPair}` branch):
```svelte
<!-- REMOVE -->
<div class="rounded-md border border-border p-3 min-w-[220px] flex-1 max-w-xs">
    ...
</div>

<!-- ADD -->
<Card.Root class="py-0 min-w-[220px] flex-1 max-w-xs">
    <Card.Content class="p-3">
        <!-- Move all content from the removed <div> here unchanged:
             the mb-3 flex row (player names + win/loss counts + close button),
             the {#if sets.length === 0} empty state, the {#else} set list,
             and the "Click a row for full details" hint paragraph.
             The close <Button> from Step 3 stays as-is. -->
    </Card.Content>
</Card.Root>
```

- [ ] **Step 7: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
git add web/src/routes/+layout.svelte \
        web/src/routes/projects/[id]/tournaments/+page.svelte \
        web/src/routes/projects/[id]/h2h/+page.svelte \
        web/src/routes/projects/[id]/import/+page.svelte \
        web/src/routes/projects/[id]/stats/+page.svelte
git commit -m "refactor(web): use Button and Card for misc raw elements"
```

---

## Task 5: Checkbox — event include/exclude toggles

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`
- Modify: `web/CLAUDE.md`

- [ ] **Step 1: Install the Checkbox component**

```bash
cd web && npx shadcn-svelte@latest add checkbox
```
Expected: creates `src/lib/components/ui/checkbox/` with `checkbox.svelte` and `index.ts`.

- [ ] **Step 2: Add Checkbox import**

In `tournaments/+page.svelte`:
```svelte
import { Checkbox } from "$lib/components/ui/checkbox";
```

- [ ] **Step 3: Replace each event checkbox**

Find the `<label>` row that wraps each event in the tournament list:
```svelte
<!-- REMOVE -->
<input
    type="checkbox"
    checked={event.included}
    onchange={() =>
        handleToggle(data.project.id, event)}
    class="h-4 w-4 rounded border-border accent-primary"
/>

<!-- ADD -->
<Checkbox
    checked={event.included}
    onCheckedChange={() => handleToggle(data.project.id, event)}
/>
```

The outer `<label class="flex cursor-pointer items-center justify-between ...">` stays unchanged — clicking anywhere in the row still activates the checkbox.

- [ ] **Step 4: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 5: Update the installed-components list in web/CLAUDE.md**

Add `Checkbox` to the table in `web/CLAUDE.md`:
```markdown
| Checkbox | `$lib/components/ui/checkbox` |
```

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/[id]/tournaments/+page.svelte web/CLAUDE.md
git commit -m "refactor(web): use Checkbox for event include/exclude toggles"
```

---

## Task 6: Tabs — project navigation

**Files:**
- Modify: `web/src/routes/projects/[id]/+layout.svelte`
- Modify: `web/CLAUDE.md`

- [ ] **Step 1: Install the Tabs component**

```bash
cd web && npx shadcn-svelte@latest add tabs
```
Expected: creates `src/lib/components/ui/tabs/` with `tabs.svelte`, `tabs-list.svelte`, `tabs-trigger.svelte`, `tabs-content.svelte`, and `index.ts`.

- [ ] **Step 2: Rewrite the navigation in `[id]/+layout.svelte`**

Replace the entire `<script>` and `<nav>` sections with the version below. The `isActive` helper is replaced by a `currentTab` derived value; `tabHref` is kept as-is.

Full updated file:
```svelte
<script lang="ts">
    import { page } from '$app/state';
    import { goto } from '$app/navigation';
    import { Separator } from '$lib/components/ui/separator';
    import * as Tabs from '$lib/components/ui/tabs';

    let { children, data } = $props();

    const tabs = [
        { label: 'Players', href: 'players' },
        { label: 'Import', href: 'import' },
        { label: 'Tournaments', href: 'tournaments' },
        { label: 'Stats', href: 'stats' },
        { label: 'H2H', href: 'h2h' }
    ];

    function tabHref(slug: string) {
        return `/projects/${data.project.id}/${slug}`;
    }

    const currentTab = $derived(
        tabs.find(t => page.url.pathname.startsWith(tabHref(t.href)))?.href ?? tabs[0].href
    );
</script>

<div class="space-y-4">
    <div>
        <a href="/projects" class="text-sm text-muted-foreground hover:text-foreground">← Projects</a>
        <h1 class="mt-1 text-2xl font-bold">{data.project.name}</h1>
        {#if data.project.game_name}
            <p class="text-sm text-muted-foreground">{data.project.game_name}</p>
        {/if}
    </div>

    <Tabs.Root value={currentTab} onValueChange={(v) => v && goto(tabHref(v))}>
        <Tabs.List>
            {#each tabs as tab (tab.href)}
                <Tabs.Trigger value={tab.href}>{tab.label}</Tabs.Trigger>
            {/each}
        </Tabs.List>
    </Tabs.Root>

    <Separator />

    {@render children()}
</div>
```

Note: `Tabs.Content` is intentionally omitted — SvelteKit renders page content via `{@render children()}`. The `Tabs.Root` only provides the tab strip; navigation is handled by `goto`.

- [ ] **Step 3: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 4: Verify navigation in the browser**

Start the dev server and navigate to any project. Confirm:
- The active tab is highlighted.
- Clicking a tab navigates to the correct route.
- The active tab updates correctly when using browser back/forward.

```bash
cd web && npm run dev
```

- [ ] **Step 5: Update the installed-components list in web/CLAUDE.md**

Add `Tabs` to the table in `web/CLAUDE.md`:
```markdown
| Tabs | `$lib/components/ui/tabs` |
```

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/[id]/+layout.svelte web/CLAUDE.md
git commit -m "refactor(web): use Tabs component for project navigation"
```

---

## Task 7: Popover — bracket-type filter

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`
- Modify: `web/CLAUDE.md`

- [ ] **Step 1: Install the Popover component**

```bash
cd web && npx shadcn-svelte@latest add popover
```
Expected: creates `src/lib/components/ui/popover/` with `popover.svelte`, `popover-content.svelte`, `popover-trigger.svelte`, and `index.ts`.

- [ ] **Step 2: Add Popover import**

In `tournaments/+page.svelte`:
```svelte
import * as Popover from "$lib/components/ui/popover";
```

- [ ] **Step 3: Delete the click-outside `$effect`**

Remove the entire `$effect` block that manages `bracketPopoverOpen` click-outside detection (lines ~111–121):
```svelte
// DELETE this entire block:
$effect(() => {
    if (!bracketPopoverOpen) return;
    function handleClick(ev: MouseEvent) {
        const wrapper = document.getElementById("bracket-popover-wrapper");
        if (wrapper && !wrapper.contains(ev.target as Node)) {
            bracketPopoverOpen = false;
        }
    }
    document.addEventListener("click", handleClick);
    return () => document.removeEventListener("click", handleClick);
});
```

- [ ] **Step 4: Replace the hand-rolled popover markup**

Find the `<div class="relative" id="bracket-popover-wrapper">` block (Row 3 of the filter panel) and replace it:

```svelte
<!-- REMOVE -->
<div class="relative" id="bracket-popover-wrapper">
    <button
        type="button"
        onclick={() => (bracketPopoverOpen = !bracketPopoverOpen)}
        class="rounded-md border px-3 py-1.5 text-sm {bracketReqCount > 0 || bracketExclCount > 0
            ? 'border-primary text-primary'
            : 'border-input text-foreground bg-background'}"
    >
        {bracketTriggerLabel}
    </button>

    {#if bracketPopoverOpen}
        <div class="absolute top-full mt-1 left-0 z-50 w-64 rounded-md border border-border bg-popover shadow-lg p-3">
            ...bracket rows content...
        </div>
    {/if}
</div>

<!-- ADD -->
<Popover.Root bind:open={bracketPopoverOpen}>
    <Popover.Trigger
        class="rounded-md border px-3 py-1.5 text-sm {bracketReqCount > 0 || bracketExclCount > 0
            ? 'border-primary text-primary'
            : 'border-input text-foreground bg-background'}"
    >
        {bracketTriggerLabel}
    </Popover.Trigger>
    <Popover.Content class="w-64 p-3" align="start">
        <!-- Move all content from the removed inner <div> here unchanged:
             the flex justify-between header (label + Reset Button),
             the column-headers grid (–/✓/✕),
             {#each COMMON_BRACKET_TYPES as bt}{@render bracketRow(bt)}{/each},
             the border-t divider,
             {#each RARE_BRACKET_TYPES as bt}{@render bracketRow(bt)}{/each},
             and the legend section. -->
    </Popover.Content>
</Popover.Root>
```

The bracket rows content (headers, `{#each COMMON_BRACKET_TYPES}`, divider, `{#each RARE_BRACKET_TYPES}`, legend) moves verbatim into `Popover.Content`. No other changes.

- [ ] **Step 5: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 6: Verify the bracket popover in the browser**

Start the dev server, navigate to a project's Tournaments page, and confirm:
- The "Brackets ▾" button opens the popover.
- Clicking outside the popover closes it (handled by Popover natively — no custom logic needed).
- Required/excluded states still update correctly.
- The trigger label updates to show counts.

```bash
cd web && npm run dev
```

- [ ] **Step 7: Update the installed-components list in web/CLAUDE.md**

Add `Popover` to the table in `web/CLAUDE.md`:
```markdown
| Popover | `$lib/components/ui/popover` |
```

- [ ] **Step 8: Commit**

```bash
git add web/src/routes/projects/[id]/tournaments/+page.svelte web/CLAUDE.md
git commit -m "refactor(web): use Popover for bracket-type filter"
```

---

## Task 8: Command — game search combobox

**Files:**
- Modify: `web/src/routes/projects/new/+page.svelte`
- Modify: `web/CLAUDE.md`

- [ ] **Step 1: Install the Command component**

```bash
cd web && npx shadcn-svelte@latest add command
```
Expected: creates `src/lib/components/ui/command/` with command components and `index.ts`.

- [ ] **Step 2: Rewrite the game-search section**

This task changes the UX from "type-in-field, results appear below" to the standard shadcn combobox pattern: a trigger button that shows the current selection, clicking it opens a Popover containing a Command search. `Popover` was installed in Task 7.

Replace the entire `<script>` block and the game-search `<div class="space-y-2">` in `new/+page.svelte`:

Updated `<script>`:
```svelte
<script lang="ts">
    import { enhance } from '$app/forms';
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';
    import { Alert } from '$lib/components/ui/alert';
    import * as Popover from '$lib/components/ui/popover';
    import * as Command from '$lib/components/ui/command';
    import { PUBLIC_API_URL } from '$env/static/public';
    import type { Game } from '$lib/types';

    let { form } = $props();

    let gameQuery = $state('');
    let gameResults = $state<Game[]>([]);
    let selectedGame = $state<Game | null>(null);
    let searching = $state(false);
    let gameSearchOpen = $state(false);
    let searchTimeout: ReturnType<typeof setTimeout>;

    function onCommandInput(value: string) {
        gameQuery = value;
        selectedGame = null;
        clearTimeout(searchTimeout);
        if (value.length < 2) { gameResults = []; return; }
        searching = true;
        searchTimeout = setTimeout(async () => {
            const res = await fetch(`${PUBLIC_API_URL}/games?q=${encodeURIComponent(value)}`, { credentials: 'include' });
            gameResults = res.ok ? await res.json() : [];
            searching = false;
        }, 300);
    }

    function selectGame(g: Game) {
        selectedGame = g;
        gameQuery = g.display_name ?? g.name;
        gameResults = [];
        gameSearchOpen = false;
    }
</script>
```

Replace the game-search field (the `<div class="space-y-2">` block that contained the `<div class="relative">` wrapper):
```svelte
<div class="space-y-2">
    <Label for="game-search">Game (optional)</Label>
    <Popover.Root bind:open={gameSearchOpen}>
        <Popover.Trigger class="w-full flex h-9 items-center justify-start rounded-md border border-input bg-transparent px-3 text-sm text-left">
            {#if selectedGame}
                {selectedGame.display_name ?? selectedGame.name}
            {:else}
                <span class="text-muted-foreground">Search start.gg games…</span>
            {/if}
        </Popover.Trigger>
        <Popover.Content class="p-0 w-80" align="start">
            <Command.Root shouldFilter={false}>
                <Command.Input
                    placeholder="Search start.gg games…"
                    value={gameQuery}
                    oninput={(e) => onCommandInput((e.target as HTMLInputElement).value)}
                />
                <Command.List>
                    {#if searching}
                        <Command.Empty>Searching…</Command.Empty>
                    {:else if gameQuery.length >= 2 && gameResults.length === 0}
                        <Command.Empty>No games found.</Command.Empty>
                    {:else}
                        {#each gameResults as g (g.id)}
                            <Command.Item
                                value={g.id.toString()}
                                onSelect={() => selectGame(g)}
                            >
                                {g.display_name ?? g.name}
                            </Command.Item>
                        {/each}
                    {/if}
                </Command.List>
            </Command.Root>
        </Popover.Content>
    </Popover.Root>
    {#if searching}
        <p class="text-xs text-muted-foreground">Searching…</p>
    {/if}
</div>
```

The two hidden inputs (for form submission) remain unchanged:
```svelte
<input type="hidden" name="game_id" value={selectedGame?.id ?? ''} />
<input type="hidden" name="game_name" value={selectedGame ? (selectedGame.display_name ?? selectedGame.name) : ''} />
```

- [ ] **Step 3: Run unit tests**

```bash
cd web && npm run test:unit
```
Expected: all tests pass.

- [ ] **Step 4: Verify game search in the browser**

Start the dev server, navigate to `/projects/new`, and confirm:
- The game field shows a clickable trigger with placeholder text.
- Clicking opens the Command popover with a search input.
- Typing ≥ 2 characters triggers a debounced fetch and shows results.
- Selecting a game closes the popover and shows the game name in the trigger.
- The form submits with the correct `game_id` and `game_name` hidden fields.

```bash
cd web && npm run dev
```

- [ ] **Step 5: Update the installed-components list in web/CLAUDE.md**

Add `Command` to the table in `web/CLAUDE.md`:
```markdown
| Command | `$lib/components/ui/command` |
```

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/new/+page.svelte web/CLAUDE.md
git commit -m "refactor(web): use Command + Popover for game search combobox"
```

---

## Done

All 8 tasks complete. Run the full test suite to confirm nothing regressed:

```bash
bash test.sh
```
