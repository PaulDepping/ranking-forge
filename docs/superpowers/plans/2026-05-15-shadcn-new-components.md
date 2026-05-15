# shadcn New Components Wave — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Install five new shadcn-svelte components (Empty, Collapsible, Skeleton, Tooltip, ScrollArea + Calendar) and apply them across the frontend to replace hand-rolled patterns.

**Architecture:** Pure frontend refactor — no backend or API changes. Each task is an isolated file edit. The date filter state in the tournaments page changes from `string` to `CalendarDate | undefined`; a derived string bridges to the existing filter logic so no test changes are needed.

**Tech Stack:** SvelteKit 5 (Svelte runes), shadcn-svelte, bits-ui, `@internationalized/date` (already installed), `@lucide/svelte` (already installed).

**Spec:** `docs/superpowers/specs/2026-05-15-shadcn-new-components-design.md`

---

## File Map

| File | Change |
|---|---|
| `web/src/routes/+layout.svelte` | Add `Tooltip.Provider` wrapper |
| `web/src/routes/projects/[id]/import/+page.svelte` | Replace raw `<label>` with `Label` |
| `web/src/routes/projects/[id]/tournaments/+page.svelte` | Collapsible filter, Button bracket trigger, Date Pickers, Empty states |
| `web/src/routes/projects/[id]/stats/+page.svelte` | ScrollArea, Empty state |
| `web/src/routes/projects/[id]/h2h/+page.svelte` | Skeleton, Tooltip on truncated names, Empty state |
| `web/src/routes/projects/[id]/players/+page.svelte` | Empty state |
| `web/src/routes/projects/+page.svelte` | Empty state |
| `web/CLAUDE.md` | Add new components to installed table |

All commands run from `web/`.

---

## Task 1: Install new components

**Files:**
- New directories under `web/src/lib/components/ui/`: `empty/`, `collapsible/`, `skeleton/`, `tooltip/`, `scroll-area/`, `calendar/`

- [ ] **Step 1: Install all six components**

```bash
cd web
npx shadcn-svelte@latest add empty collapsible skeleton tooltip scroll-area calendar
```

Accept any prompts. Expected: six new component directories created under `src/lib/components/ui/`.

- [ ] **Step 2: Verify components exist**

```bash
ls src/lib/components/ui/
```

Expected output includes: `calendar/`, `collapsible/`, `empty/`, `scroll-area/`, `skeleton/`, `tooltip/`

- [ ] **Step 3: Run type check**

```bash
npm run check
```

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/ui/
git commit -m "feat(web): install Empty, Collapsible, Skeleton, Tooltip, ScrollArea, Calendar components"
```

---

## Task 2: Add Tooltip.Provider to root layout

The `Tooltip` component requires a `Tooltip.Provider` ancestor. Place it in the root layout so all pages can use tooltips without per-page providers.

**Files:**
- Modify: `web/src/routes/+layout.svelte`

- [ ] **Step 1: Add import and wrap children**

Replace the current `+layout.svelte` with:

```svelte
<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { ModeWatcher } from 'mode-watcher';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Tooltip from '$lib/components/ui/tooltip';

	let { children, data } = $props();

	async function logout() {
		await fetch(`${PUBLIC_API_URL}/auth/logout`, { method: 'POST', credentials: 'include' });
		location.href = '/login';
	}
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<ModeWatcher />

<Tooltip.Provider>
	{#if data.user}
		<header class="border-b border-border bg-card">
			<div class="mx-auto flex max-w-5xl items-center justify-between px-4 py-3">
				<a href="/projects" class="font-semibold text-foreground hover:text-primary">RankingForge</a>
				<div class="flex items-center gap-4">
					<span class="text-sm text-muted-foreground">{data.user.username}</span>
					<ThemeToggle />
					<Button variant="ghost" size="sm" onclick={logout}>Logout</Button>
				</div>
			</div>
		</header>
	{/if}

	<main class="mx-auto max-w-5xl px-4 py-8">
		{@render children()}
	</main>
</Tooltip.Provider>
```

- [ ] **Step 2: Run type check**

```bash
npm run check
```

Expected: zero errors.

- [ ] **Step 3: Commit**

```bash
git add src/routes/+layout.svelte
git commit -m "feat(web): wrap root layout with Tooltip.Provider"
```

---

## Task 3: Fix raw labels on import page

Two bare `<label>` elements in the import page should use the `Label` component.

**Files:**
- Modify: `web/src/routes/projects/[id]/import/+page.svelte`

- [ ] **Step 1: Add Label import**

In the `<script>` block, the imports currently are:

```svelte
import { Button } from '$lib/components/ui/button';
import { Input } from '$lib/components/ui/input';
import { Badge } from '$lib/components/ui/badge';
import { Alert } from '$lib/components/ui/alert';
import * as Card from '$lib/components/ui/card';
```

Add `Label` import:

```svelte
import { Button } from '$lib/components/ui/button';
import { Input } from '$lib/components/ui/input';
import { Badge } from '$lib/components/ui/badge';
import { Alert } from '$lib/components/ui/alert';
import { Label } from '$lib/components/ui/label';
import * as Card from '$lib/components/ui/card';
```

- [ ] **Step 2: Replace raw label elements**

Find in the template:

```svelte
<div class="grid grid-cols-2 gap-4">
	<div class="space-y-1">
		<label for="after_date" class="text-sm font-medium">From date</label>
		<Input id="after_date" name="after_date" type="date" />
	</div>
	<div class="space-y-1">
		<label for="before_date" class="text-sm font-medium">To date</label>
		<Input id="before_date" name="before_date" type="date" />
	</div>
</div>
```

Replace with:

```svelte
<div class="grid grid-cols-2 gap-4">
	<div class="space-y-1">
		<Label for="after_date">From date</Label>
		<Input id="after_date" name="after_date" type="date" />
	</div>
	<div class="space-y-1">
		<Label for="before_date">To date</Label>
		<Input id="before_date" name="before_date" type="date" />
	</div>
</div>
```

- [ ] **Step 3: Run type check**

```bash
npm run check
```

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
git add src/routes/projects/\[id\]/import/+page.svelte
git commit -m "refactor(web): use Label component in import page date fields"
```

---

## Task 4: Collapsible filter panel on tournaments page

Replace the manual `{#if filterOpen}` toggle with `Collapsible.Root/Trigger/Content`.

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Add Collapsible import**

In the `<script>` block, after the existing imports, add:

```svelte
import * as Collapsible from '$lib/components/ui/collapsible';
```

- [ ] **Step 2: Replace the status line + filter panel structure**

Find the comment `<!-- Status line + toggle -->` and everything through the closing `{/if}` of the filter panel (approximately lines 232–363 in the current file). Replace with:

```svelte
<Collapsible.Root bind:open={filterOpen}>
	<!-- Status line + toggle -->
	<div class="flex items-center justify-between text-sm text-muted-foreground">
		<span>
			Showing <strong>{visibleTournaments.length}</strong> of {tournaments.length} tournaments
			· <strong>{visibleEventCount}</strong> of {totalEventCount} events
		</span>
		<Collapsible.Trigger>
			{#snippet child({ props })}
				<Button {...props} variant="outline" size="sm">
					⚙ Filters &amp; Actions {filterOpen ? '▲' : '▼'}
				</Button>
			{/snippet}
		</Collapsible.Trigger>
	</div>

	<Collapsible.Content>
		<!-- Collapsible filter panel -->
		<div class="rounded-md border border-border bg-muted/30 p-4 space-y-3 mt-2">
			<!-- Header: label + clear button -->
			<div class="flex items-center justify-between">
				<span class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Filters</span>
				<Button type="button" variant="ghost" size="sm" onclick={resetAllFilters}>Clear filters</Button>
			</div>

			<!-- Row 1: search + venue -->
			<div class="flex flex-wrap gap-2">
				<Input
					type="text"
					placeholder="Search tournament or event name…"
					bind:value={search}
					class="flex-1 min-w-48"
				/>
				<Select.Root bind:value={venueFilter}>
					<Select.Trigger class="w-36">{venueLabel}</Select.Trigger>
					<Select.Content>
						<Select.Item value="all">Venue: All</Select.Item>
						<Select.Item value="online">Online only</Select.Item>
						<Select.Item value="offline">Offline only</Select.Item>
					</Select.Content>
				</Select.Root>
			</div>

			<!-- Row 2: entrant range + date range -->
			<div class="flex flex-wrap gap-2 items-center">
				<div class="flex items-center gap-1.5">
					<span class="text-xs text-muted-foreground whitespace-nowrap">Entrants</span>
					<Input type="number" min="0" placeholder="min" bind:value={minEntrants} class="w-20" />
					<span class="text-muted-foreground">–</span>
					<Input type="number" min="0" placeholder="max" bind:value={maxEntrants} class="w-20" />
				</div>
				<div class="flex items-center gap-1.5">
					<span class="text-xs text-muted-foreground">From</span>
					<Input type="date" bind:value={dateFrom} class="w-auto" />
					<span class="text-xs text-muted-foreground">To</span>
					<Input type="date" bind:value={dateTo} class="w-auto" />
				</div>
			</div>

			<!-- Row 3: event type + bracket filter -->
			<div class="flex flex-wrap gap-4 items-center">
				<div class="flex items-center gap-2">
					<span class="text-xs text-muted-foreground whitespace-nowrap">Event type</span>
					<Select.Root bind:value={eventType}>
						<Select.Trigger class="w-28">{eventTypeLabel}</Select.Trigger>
						<Select.Content>
							<Select.Item value="all">All types</Select.Item>
							<Select.Item value="singles">Singles</Select.Item>
							<Select.Item value="teams">Teams</Select.Item>
						</Select.Content>
					</Select.Root>
				</div>

				<Popover.Root bind:open={bracketPopoverOpen}>
					<Popover.Trigger>
						{#snippet child({ props })}
							<Button
								{...props}
								variant="outline"
								size="sm"
								class={bracketReqCount > 0 || bracketExclCount > 0
									? 'border-primary text-primary'
									: ''}
							>
								{bracketTriggerLabel}
							</Button>
						{/snippet}
					</Popover.Trigger>
					<Popover.Content class="w-64 p-3" align="start">
						<div class="flex justify-between items-center mb-2">
							<span class="text-xs text-muted-foreground uppercase tracking-wide">Bracket Types</span>
							<Button type="button" variant="ghost" size="sm" onclick={resetBracketFilter}>Reset</Button>
						</div>

						<!-- Column headers -->
						<div class="grid grid-cols-[1fr_28px_28px_28px] gap-1 mb-1">
							<span></span>
							<span class="text-xs text-muted-foreground text-center">–</span>
							<span class="text-xs text-muted-foreground text-center">✓</span>
							<span class="text-xs text-muted-foreground text-center">✕</span>
						</div>

						<!-- Common bracket types -->
						{#each COMMON_BRACKET_TYPES as bt}
							{@render bracketRow(bt)}
						{/each}

						<div class="border-t border-border my-1.5"></div>

						<!-- Rarer bracket types -->
						{#each RARE_BRACKET_TYPES as bt}
							{@render bracketRow(bt)}
						{/each}

						<!-- Legend -->
						<div class="mt-2 pt-2 border-t border-border flex gap-3 flex-wrap">
							<span class="text-[10px] text-muted-foreground"><span class="text-indigo-400">–</span> don't care</span>
							<span class="text-[10px] text-muted-foreground"><span class="text-green-400">✓</span> required</span>
							<span class="text-[10px] text-muted-foreground"><span class="text-red-400">✕</span> excluded</span>
						</div>
					</Popover.Content>
				</Popover.Root>
			</div>

			<!-- Divider + bulk actions -->
			<div class="flex items-center justify-between border-t border-border pt-3">
				<span class="text-xs text-muted-foreground">
					Bulk actions apply to {visibleEventCount} visible event{visibleEventCount !== 1 ? 's' : ''}
				</span>
				<div class="flex gap-2">
					<Button variant="outline" size="sm" onclick={() => bulkSetIncluded(true)}>
						✓ Include all visible
					</Button>
					<Button
						variant="outline"
						size="sm"
						class="border-destructive text-destructive hover:bg-destructive/10"
						onclick={() => bulkSetIncluded(false)}
					>
						✕ Exclude all visible
					</Button>
				</div>
			</div>
		</div>
	</Collapsible.Content>
</Collapsible.Root>
```

- [ ] **Step 3: Run type check**

```bash
npm run check
```

Expected: zero errors.

- [ ] **Step 4: Run unit tests**

```bash
npm run test:unit
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/routes/projects/\[id\]/tournaments/+page.svelte
git commit -m "refactor(web): use Collapsible for tournaments filter panel"
```

---

## Task 5: Date Picker for tournament From/To date filters

Replace `<Input type="date">` with Calendar + Popover date pickers. The filter predicate keeps string comparisons via derived values.

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Add imports**

Add to the `<script>` block imports:

```svelte
import Calendar from '$lib/components/ui/calendar/calendar.svelte';
import { type CalendarDate, getLocalTimeZone } from '@internationalized/date';
```

- [ ] **Step 2: Update date state variables**

Find in the `<script>` block:

```svelte
let dateFrom    = $state('');
let dateTo      = $state('');
```

Replace with:

```svelte
let dateFrom      = $state<CalendarDate | undefined>(undefined);
let dateTo        = $state<CalendarDate | undefined>(undefined);
let dateFromOpen  = $state(false);
let dateToOpen    = $state(false);

const dateFromStr = $derived(dateFrom?.toString() ?? '');
const dateToStr   = $derived(dateTo?.toString() ?? '');
```

- [ ] **Step 3: Update tournamentVisible to use derived strings**

Find:

```svelte
function tournamentVisible(t: Tournament): boolean {
	if (venueFilter === 'online' && !t.online) return false;
	if (venueFilter === 'offline' && t.online) return false;
	if (dateFrom && t.start_at && t.start_at.slice(0, 10) < dateFrom) return false;
	if (dateTo && t.start_at && t.start_at.slice(0, 10) > dateTo) return false;
	return true;
}
```

Replace with:

```svelte
function tournamentVisible(t: Tournament): boolean {
	if (venueFilter === 'online' && !t.online) return false;
	if (venueFilter === 'offline' && t.online) return false;
	if (dateFromStr && t.start_at && t.start_at.slice(0, 10) < dateFromStr) return false;
	if (dateToStr && t.start_at && t.start_at.slice(0, 10) > dateToStr) return false;
	return true;
}
```

- [ ] **Step 4: Update resetAllFilters**

Find:

```svelte
dateFrom    = '';
dateTo      = '';
```

Replace with:

```svelte
dateFrom    = undefined;
dateTo      = undefined;
```

- [ ] **Step 5: Replace the date input row in the template**

Inside the Collapsible.Content, find the date range row:

```svelte
<div class="flex items-center gap-1.5">
	<span class="text-xs text-muted-foreground">From</span>
	<Input type="date" bind:value={dateFrom} class="w-auto" />
	<span class="text-xs text-muted-foreground">To</span>
	<Input type="date" bind:value={dateTo} class="w-auto" />
</div>
```

Replace with:

```svelte
<div class="flex items-center gap-1.5">
	<span class="text-xs text-muted-foreground">From</span>
	<Popover.Root bind:open={dateFromOpen}>
		<Popover.Trigger>
			{#snippet child({ props })}
				<Button {...props} variant="outline" size="sm" class="w-32 justify-start font-normal">
					{dateFrom
						? dateFrom.toDate(getLocalTimeZone()).toLocaleDateString()
						: 'Pick date'}
				</Button>
			{/snippet}
		</Popover.Trigger>
		<Popover.Content class="w-auto overflow-hidden p-0" align="start">
			<Calendar
				type="single"
				bind:value={dateFrom}
				captionLayout="dropdown"
				onValueChange={() => { dateFromOpen = false; }}
			/>
		</Popover.Content>
	</Popover.Root>
	<span class="text-xs text-muted-foreground">To</span>
	<Popover.Root bind:open={dateToOpen}>
		<Popover.Trigger>
			{#snippet child({ props })}
				<Button {...props} variant="outline" size="sm" class="w-32 justify-start font-normal">
					{dateTo
						? dateTo.toDate(getLocalTimeZone()).toLocaleDateString()
						: 'Pick date'}
				</Button>
			{/snippet}
		</Popover.Trigger>
		<Popover.Content class="w-auto overflow-hidden p-0" align="start">
			<Calendar
				type="single"
				bind:value={dateTo}
				captionLayout="dropdown"
				onValueChange={() => { dateToOpen = false; }}
			/>
		</Popover.Content>
	</Popover.Root>
</div>
```

- [ ] **Step 6: Run type check**

```bash
npm run check
```

Expected: zero errors.

- [ ] **Step 7: Run unit tests**

```bash
npm run test:unit
```

Expected: all tests pass. (The `filter.test.ts` tests a standalone copy of the logic using strings and does not need changes.)

- [ ] **Step 8: Commit**

```bash
git add src/routes/projects/\[id\]/tournaments/+page.svelte
git commit -m "feat(web): replace date inputs with Calendar date pickers in tournament filter"
```

---

## Task 6: Empty states on tournaments page

Replace two plain `<p>` empty-state messages with `Empty` components.

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Add Empty import**

Add to the `<script>` block imports:

```svelte
import * as Empty from '$lib/components/ui/empty';
```

- [ ] **Step 2: Replace "no tournaments" empty state**

Find:

```svelte
{#if tournaments.length === 0}
	<p class="text-sm text-muted-foreground">No tournaments imported yet. Run an import first.</p>
{:else}
```

Replace with:

```svelte
{#if tournaments.length === 0}
	<Empty.Root>
		<Empty.Header>
			<Empty.Title>No tournaments yet</Empty.Title>
			<Empty.Description>Run an import to pull in tournaments from start.gg.</Empty.Description>
		</Empty.Header>
	</Empty.Root>
{:else}
```

- [ ] **Step 3: Replace "no matching filters" empty state**

Find (inside the `{:else}` block, inside the tournament list section):

```svelte
{#if visibleTournaments.length === 0}
	<p class="text-sm text-muted-foreground">No tournaments match the current filters.</p>
{/if}
```

Replace with:

```svelte
{#if visibleTournaments.length === 0}
	<Empty.Root>
		<Empty.Header>
			<Empty.Title>No results</Empty.Title>
			<Empty.Description>No tournaments match the current filters.</Empty.Description>
		</Empty.Header>
	</Empty.Root>
{/if}
```

- [ ] **Step 4: Run type check and unit tests**

```bash
npm run check && npm run test:unit
```

Expected: zero errors, all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/routes/projects/\[id\]/tournaments/+page.svelte
git commit -m "refactor(web): use Empty component for tournament empty states"
```

---

## Task 7: ScrollArea and Empty state on stats page

Replace the hand-rolled overflow containers with ScrollArea and the plain-text empty state with Empty.

**Files:**
- Modify: `web/src/routes/projects/[id]/stats/+page.svelte`

- [ ] **Step 1: Add imports**

Replace the current script block:

```svelte
<script lang="ts">
	import type { SetRecord } from '$lib/types';
	import SetDetailModal from '$lib/components/SetDetailModal.svelte';
	import * as Card from '$lib/components/ui/card';
```

With:

```svelte
<script lang="ts">
	import type { SetRecord } from '$lib/types';
	import SetDetailModal from '$lib/components/SetDetailModal.svelte';
	import * as Card from '$lib/components/ui/card';
	import { ScrollArea } from '$lib/components/ui/scroll-area';
	import * as Empty from '$lib/components/ui/empty';
```

- [ ] **Step 2: Replace the wins scroll container**

Find:

```svelte
<div class="h-24 overflow-y-auto rounded border border-border bg-muted/20">
	{#each player.wins as set, i (i)}
		<button
			class="w-full border-b border-border px-2 py-1 text-left text-xs last:border-0 hover:bg-muted/50"
			onclick={() => openModal(set, true, player.name)}
		>
			<span>{set.opponent_name} · UF {set.upset_factor}</span>
		</button>
	{/each}
</div>
```

Replace with:

```svelte
<ScrollArea class="h-24 rounded border border-border bg-muted/20">
	{#each player.wins as set, i (i)}
		<button
			class="w-full border-b border-border px-2 py-1 text-left text-xs last:border-0 hover:bg-muted/50"
			onclick={() => openModal(set, true, player.name)}
		>
			<span>{set.opponent_name} · UF {set.upset_factor}</span>
		</button>
	{/each}
</ScrollArea>
```

- [ ] **Step 3: Replace the losses scroll container**

Find:

```svelte
<div class="h-24 overflow-y-auto rounded border border-border bg-muted/20">
	{#each player.losses as set, i (i)}
		<button
			class="w-full border-b border-border px-2 py-1 text-left text-xs last:border-0 hover:bg-muted/50"
			onclick={() => openModal(set, false, player.name)}
		>
			<span>{set.opponent_name} · UF {set.upset_factor}</span>
		</button>
	{/each}
</div>
```

Replace with:

```svelte
<ScrollArea class="h-24 rounded border border-border bg-muted/20">
	{#each player.losses as set, i (i)}
		<button
			class="w-full border-b border-border px-2 py-1 text-left text-xs last:border-0 hover:bg-muted/50"
			onclick={() => openModal(set, false, player.name)}
		>
			<span>{set.opponent_name} · UF {set.upset_factor}</span>
		</button>
	{/each}
</ScrollArea>
```

- [ ] **Step 4: Replace the empty state**

Find:

```svelte
{#if data.stats.length === 0}
	<p class="text-sm text-muted-foreground">No stats yet. Import tournaments and include some events first.</p>
{:else}
```

Replace with:

```svelte
{#if data.stats.length === 0}
	<Empty.Root>
		<Empty.Header>
			<Empty.Title>No stats yet</Empty.Title>
			<Empty.Description>Import tournaments and include some events to generate stats.</Empty.Description>
		</Empty.Header>
	</Empty.Root>
{:else}
```

- [ ] **Step 5: Run type check and unit tests**

```bash
npm run check && npm run test:unit
```

Expected: zero errors, all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/routes/projects/\[id\]/stats/+page.svelte
git commit -m "refactor(web): use ScrollArea and Empty in stats page"
```

---

## Task 8: Skeleton, Tooltip, and Empty state on H2H page

Three improvements: loading skeleton, tooltips on truncated player names, and Empty for the no-data state.

**Files:**
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`

- [ ] **Step 1: Add imports**

Replace the current imports section in `<script>`:

```svelte
import type { HeadToHeadEntry, H2HSet } from '$lib/types';
import SetDetailModal from '$lib/components/SetDetailModal.svelte';
import * as Table from '$lib/components/ui/table';
import { Button } from '$lib/components/ui/button';
import * as Card from '$lib/components/ui/card';
```

With:

```svelte
import type { HeadToHeadEntry, H2HSet } from '$lib/types';
import SetDetailModal from '$lib/components/SetDetailModal.svelte';
import * as Table from '$lib/components/ui/table';
import { Button } from '$lib/components/ui/button';
import * as Card from '$lib/components/ui/card';
import { Skeleton } from '$lib/components/ui/skeleton';
import * as Tooltip from '$lib/components/ui/tooltip';
import * as Empty from '$lib/components/ui/empty';
```

- [ ] **Step 2: Replace the loading state**

Find:

```svelte
{#if loading}
	<div class="flex items-center justify-center rounded-md border border-border p-6 text-sm text-muted-foreground min-w-[200px]">
		Loading…
	</div>
```

Replace with:

```svelte
{#if loading}
	<div class="min-w-[220px] flex-1 max-w-xs space-y-3">
		<Skeleton class="h-8 w-full rounded-md" />
		<Skeleton class="h-4 w-3/4" />
		<div class="space-y-1.5 mt-1">
			<Skeleton class="h-7 w-full" />
			<Skeleton class="h-7 w-full" />
			<Skeleton class="h-7 w-full" />
		</div>
	</div>
```

- [ ] **Step 3: Add Tooltip to column header names**

Find the column header cell:

```svelte
<Table.Head class="px-2 pb-2 text-center font-medium h-auto" style="min-width:5rem">
	<span class="block max-w-[5rem] truncate" title={col.name}>{col.name}</span>
</Table.Head>
```

Replace with:

```svelte
<Table.Head class="px-2 pb-2 text-center font-medium h-auto" style="min-width:5rem">
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<span {...props} class="block max-w-[5rem] truncate">{col.name}</span>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content>
			<p>{col.name}</p>
		</Tooltip.Content>
	</Tooltip.Root>
</Table.Head>
```

- [ ] **Step 4: Add Tooltip to row label cells**

Find:

```svelte
<Table.Cell class="max-w-[8rem] truncate py-1 pr-3 font-medium" title={row.name}>{row.name}</Table.Cell>
```

Replace with:

```svelte
<Table.Cell class="py-1 pr-3 font-medium">
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<span {...props} class="block max-w-[8rem] truncate">{row.name}</span>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content>
			<p>{row.name}</p>
		</Tooltip.Content>
	</Tooltip.Root>
</Table.Cell>
```

- [ ] **Step 5: Replace the empty state**

Find:

```svelte
{#if data.players.length < 2 || data.h2h.length === 0}
	<p class="text-sm text-muted-foreground">No head-to-head data yet. Import tournaments first.</p>
{:else}
```

Replace with:

```svelte
{#if data.players.length < 2 || data.h2h.length === 0}
	<Empty.Root>
		<Empty.Header>
			<Empty.Title>No head-to-head data yet</Empty.Title>
			<Empty.Description>Import tournaments to generate head-to-head records.</Empty.Description>
		</Empty.Header>
	</Empty.Root>
{:else}
```

- [ ] **Step 6: Run type check and tests**

```bash
npm run check && npm run test:unit
```

Expected: zero errors, all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/routes/projects/\[id\]/h2h/+page.svelte
git commit -m "refactor(web): use Skeleton, Tooltip, and Empty in H2H page"
```

---

## Task 9: Empty states on players and projects pages

**Files:**
- Modify: `web/src/routes/projects/[id]/players/+page.svelte`
- Modify: `web/src/routes/projects/+page.svelte`

- [ ] **Step 1: Players page — add import and replace empty state**

Add import to `<script>` in `players/+page.svelte`:

```svelte
import * as Empty from '$lib/components/ui/empty';
```

Find:

```svelte
{#if data.players.length === 0}
	<p class="text-sm text-muted-foreground">No players yet. Add one above.</p>
{:else}
```

Replace with:

```svelte
{#if data.players.length === 0}
	<Empty.Root>
		<Empty.Header>
			<Empty.Title>No players yet</Empty.Title>
			<Empty.Description>Add a player above to get started.</Empty.Description>
		</Empty.Header>
	</Empty.Root>
{:else}
```

- [ ] **Step 2: Projects page — add import and replace empty state**

Add import to `<script>` in `projects/+page.svelte` (currently only imports `enhance`, `Button`, and `Card` components):

```svelte
import * as Empty from '$lib/components/ui/empty';
```

Find:

```svelte
{#if data.projects.length === 0}
	<p class="text-muted-foreground">No projects yet. Create one to get started.</p>
{:else}
```

Replace with:

```svelte
{#if data.projects.length === 0}
	<Empty.Root>
		<Empty.Header>
			<Empty.Title>No projects yet</Empty.Title>
			<Empty.Description>Create a project to start building a power ranking.</Empty.Description>
		</Empty.Header>
		<Empty.Content>
			<Button href="/projects/new">New project</Button>
		</Empty.Content>
	</Empty.Root>
{:else}
```

- [ ] **Step 3: Run type check and tests**

```bash
npm run check && npm run test:unit
```

Expected: zero errors, all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/routes/projects/\[id\]/players/+page.svelte src/routes/projects/+page.svelte
git commit -m "refactor(web): use Empty component for players and projects empty states"
```

---

## Task 10: Update CLAUDE.md

Add the six new components to the installed-components table in `web/CLAUDE.md`.

**Files:**
- Modify: `web/CLAUDE.md`

- [ ] **Step 1: Add rows to the installed components table**

Find the table in `web/CLAUDE.md`:

```markdown
| Tabs | `$lib/components/ui/tabs` |
| Table | `$lib/components/ui/table` |
```

Add after the last row:

```markdown
| Tabs | `$lib/components/ui/tabs` |
| Table | `$lib/components/ui/table` |
| Calendar | `$lib/components/ui/calendar` |
| Collapsible | `$lib/components/ui/collapsible` |
| Empty | `$lib/components/ui/empty` |
| Scroll Area | `$lib/components/ui/scroll-area` |
| Skeleton | `$lib/components/ui/skeleton` |
| Tooltip | `$lib/components/ui/tooltip` |
```

Also update the CLAUDE.md at the root of the project: find the line in the `## Frontend` section:

```
Installed components: `alert`, `badge`, `button`, `card`, `dialog`, `input`, `label`, `select`, `separator`, `table`. Install others as needed with `npx shadcn-svelte@latest add <name>`.
```

Replace with:

```
Installed components: `alert`, `badge`, `button`, `calendar`, `card`, `checkbox`, `collapsible`, `command`, `dialog`, `empty`, `input`, `label`, `popover`, `scroll-area`, `select`, `separator`, `skeleton`, `table`, `tabs`, `textarea`, `tooltip`. Install others as needed with `npx shadcn-svelte@latest add <name>`.
```

- [ ] **Step 2: Run final full test suite**

```bash
npm run test:unit && npm run test:e2e
```

Expected: all tests pass.

- [ ] **Step 3: Final commit**

```bash
git add CLAUDE.md web/CLAUDE.md
git commit -m "docs: update CLAUDE.md with newly installed shadcn components"
```
