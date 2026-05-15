# Bracket Type Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the "Exclude ladder / matchmaking" checkbox in the tournament filter panel with a per-bracket-type three-state filter (neutral / required / excluded) exposed as a dropdown popover.

**Architecture:** Two files change: the standalone filter test file gets an updated `eventVisible` signature and new test cases; the Svelte page component gets new state, updated filter logic, and a popover UI. The filter logic lives entirely in the frontend — no backend changes.

**Tech Stack:** Svelte 5 (`$state`, `$derived`, `$effect`, `{#snippet}`), Tailwind CSS, Vitest (frontend unit tests).

---

## Files Changed

| File | What changes |
|---|---|
| `web/src/routes/projects/[id]/tournaments/filter.test.ts` | Update `eventVisible` signature, update call sites, remove old `excludeLadder` tests, add 8 new bracket filter tests |
| `web/src/routes/projects/[id]/tournaments/+page.svelte` | Replace `excludeLadder` bool with `bracketFilter` record, update `eventVisible`, add popover UI |

---

## Task 1: Update filter tests

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/filter.test.ts`

- [ ] **Step 1: Replace the entire test file**

The standalone `eventVisible` gets a `bracketFilter: Record<string, BracketTypeState>` param instead of `excludeLadder: boolean`. All existing call sites replace `false` with `{}`. The two old `excludeLadder` tests are removed and replaced with 8 new bracket filter tests.

Write the complete file:

```ts
import { describe, it, expect } from 'vitest';
import type { Tournament, TournamentEvent } from '$lib/types';

type BracketTypeState = 'neutral' | 'required' | 'excluded';

function makeEvent(overrides: Partial<TournamentEvent> = {}): TournamentEvent {
    return {
        id: 'e1', startgg_id: 1, name: 'Melee Singles',
        game_name: null, num_entrants: 100, start_at: null,
        included: true, event_type: 1, bracket_types: ['DOUBLE_ELIMINATION'],
        ...overrides,
    };
}

function makeTournament(events: TournamentEvent[], overrides: Partial<Tournament> = {}): Tournament {
    return {
        id: 't1', startgg_id: 1, name: 'Genesis 10', slug: 'tournament/genesis-10',
        city: 'San Jose', addr_state: 'CA', country_code: 'US',
        venue_name: null, online: false,
        start_at: '2025-01-12T00:00:00Z', end_at: null,
        events,
        ...overrides,
    };
}

// Standalone filter functions (same logic as in +page.svelte, with explicit params)
function tournamentVisible(
    t: Tournament,
    venueFilter: 'all' | 'online' | 'offline',
    dateFrom: string,
    dateTo: string,
): boolean {
    if (venueFilter === 'online' && !t.online) return false;
    if (venueFilter === 'offline' && t.online) return false;
    if (dateFrom && t.start_at && t.start_at.slice(0, 10) < dateFrom) return false;
    if (dateTo && t.start_at && t.start_at.slice(0, 10) > dateTo) return false;
    return true;
}

function eventVisible(
    e: TournamentEvent,
    t: Tournament,
    search: string,
    minEntrants: number | null,
    maxEntrants: number | null,
    eventType: 'all' | 'singles' | 'teams',
    bracketFilter: Record<string, BracketTypeState>,
): boolean {
    if (search.trim()) {
        const q = search.trim().toLowerCase();
        if (!e.name.toLowerCase().includes(q) && !t.name.toLowerCase().includes(q)) return false;
    }
    if (+minEntrants > 0 && (e.num_entrants ?? Infinity) < +minEntrants) return false;
    if (+maxEntrants > 0 && (e.num_entrants ?? 0) > +maxEntrants) return false;
    if (eventType === 'singles' && e.event_type !== null && e.event_type !== 1) return false;
    if (eventType === 'teams' && e.event_type !== null && e.event_type !== 2) return false;

    const required = Object.entries(bracketFilter)
        .filter(([, s]) => s === 'required')
        .map(([t]) => t);
    const excluded = Object.entries(bracketFilter)
        .filter(([, s]) => s === 'excluded')
        .map(([t]) => t);

    if (required.length > 0 || excluded.length > 0) {
        if (e.bracket_types.length === 0) return true;
        for (const r of required) {
            if (!e.bracket_types.includes(r)) return false;
        }
        for (const x of excluded) {
            if (e.bracket_types.includes(x)) return false;
        }
    }

    return true;
}

describe('tournament filter', () => {
    it('venue filter hides online tournaments', () => {
        const t = makeTournament([], { online: true });
        expect(tournamentVisible(t, 'offline', '', '')).toBe(false);
        expect(tournamentVisible(t, 'online', '', '')).toBe(true);
    });

    it('date range filter hides tournaments outside range', () => {
        const t = makeTournament([], { start_at: '2024-06-01T00:00:00Z' });
        expect(tournamentVisible(t, 'all', '2025-01-01', '')).toBe(false);
        expect(tournamentVisible(t, 'all', '2024-01-01', '2024-12-31')).toBe(true);
        // boundary: start_at on exactly dateTo day must pass
        expect(tournamentVisible(t, 'all', '', '2024-06-01')).toBe(true);
    });

    it('null start_at passes date filter', () => {
        const t = makeTournament([], { start_at: null });
        expect(tournamentVisible(t, 'all', '2025-01-01', '2025-12-31')).toBe(true);
    });

    it('name search matches event name', () => {
        const t = makeTournament([]);
        const e = makeEvent({ name: 'Melee Doubles' });
        expect(eventVisible(e, t, 'doubles', null, null, 'all', {})).toBe(true);
        expect(eventVisible(e, t, 'singles', null, null, 'all', {})).toBe(false);
    });

    it('name search on tournament name shows all events', () => {
        const t = makeTournament([]);
        const e = makeEvent({ name: 'Melee Doubles' });
        expect(eventVisible(e, t, 'genesis', null, null, 'all', {})).toBe(true);
    });

    it('entrant range filter', () => {
        const t = makeTournament([]);
        const small = makeEvent({ num_entrants: 16 });
        const large = makeEvent({ num_entrants: 512 });
        expect(eventVisible(small, t, '', 32, null, 'all', {})).toBe(false);
        expect(eventVisible(large, t, '', 32, 200, 'all', {})).toBe(false);
        expect(eventVisible(large, t, '', 32, null, 'all', {})).toBe(true);
    });

    it('null num_entrants passes min/max filter', () => {
        const t = makeTournament([]);
        const e = makeEvent({ num_entrants: null });
        expect(eventVisible(e, t, '', 32, 100, 'all', {})).toBe(true);
    });

    it('eventType singles filter', () => {
        const t = makeTournament([]);
        const singles = makeEvent({ event_type: 1 });
        const teams = makeEvent({ event_type: 2 });
        expect(eventVisible(singles, t, '', null, null, 'singles', {})).toBe(true);
        expect(eventVisible(teams, t, '', null, null, 'singles', {})).toBe(false);
    });

    it('null event_type passes all eventType filters', () => {
        const t = makeTournament([]);
        const e = makeEvent({ event_type: null });
        expect(eventVisible(e, t, '', null, null, 'singles', {})).toBe(true);
        expect(eventVisible(e, t, '', null, null, 'teams', {})).toBe(true);
    });
});

describe('bracket type filter', () => {
    it('all neutral — no filtering applied', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['MATCHMAKING'] });
        expect(eventVisible(e, t, '', null, null, 'all', {})).toBe(true);
    });

    it('required type present — passes', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['DOUBLE_ELIMINATION'] });
        expect(eventVisible(e, t, '', null, null, 'all', { DOUBLE_ELIMINATION: 'required' })).toBe(true);
    });

    it('required type absent — filtered out', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['ROUND_ROBIN'] });
        expect(eventVisible(e, t, '', null, null, 'all', { DOUBLE_ELIMINATION: 'required' })).toBe(false);
    });

    it('excluded type present — filtered out', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['MATCHMAKING'] });
        expect(eventVisible(e, t, '', null, null, 'all', { MATCHMAKING: 'excluded' })).toBe(false);
    });

    it('excluded type absent — passes', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: ['DOUBLE_ELIMINATION'] });
        expect(eventVisible(e, t, '', null, null, 'all', { MATCHMAKING: 'excluded' })).toBe(true);
    });

    it('multiple required types — event must have all of them', () => {
        const t = makeTournament([]);
        const hasAll = makeEvent({ bracket_types: ['ROUND_ROBIN', 'DOUBLE_ELIMINATION'] });
        const missingOne = makeEvent({ bracket_types: ['DOUBLE_ELIMINATION'] });
        const filter: Record<string, BracketTypeState> = {
            ROUND_ROBIN: 'required',
            DOUBLE_ELIMINATION: 'required',
        };
        expect(eventVisible(hasAll, t, '', null, null, 'all', filter)).toBe(true);
        expect(eventVisible(missingOne, t, '', null, null, 'all', filter)).toBe(false);
    });

    it('required + excluded on different types — excluded wins when both present', () => {
        const t = makeTournament([]);
        // Event has the required type but also the excluded type → rejected
        const e = makeEvent({ bracket_types: ['DOUBLE_ELIMINATION', 'MATCHMAKING'] });
        const filter: Record<string, BracketTypeState> = {
            DOUBLE_ELIMINATION: 'required',
            MATCHMAKING: 'excluded',
        };
        expect(eventVisible(e, t, '', null, null, 'all', filter)).toBe(false);
    });

    it('empty bracket_types passes regardless of filter state', () => {
        const t = makeTournament([]);
        const e = makeEvent({ bracket_types: [] });
        const filter: Record<string, BracketTypeState> = {
            DOUBLE_ELIMINATION: 'required',
            MATCHMAKING: 'excluded',
        };
        expect(eventVisible(e, t, '', null, null, 'all', filter)).toBe(true);
    });
});
```

- [ ] **Step 2: Run the tests**

```bash
cd web && npm run test:unit -- filter
```

Expected: all tests pass (the standalone functions in the test file are self-contained).

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/projects/[id]/tournaments/filter.test.ts
git commit -m "test(tournaments): update bracket filter tests, remove excludeLadder"
```

---

## Task 2: Update +page.svelte — logic and UI

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Replace the `<script>` block**

Replace the entire `<script lang="ts">` block (lines 1–95 in the original) with:

```svelte
<script lang="ts">
	import { untrack } from 'svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { PUBLIC_API_URL } from '$env/static/public';
	import type { Tournament, TournamentEvent } from '$lib/types';

	let { data } = $props();

	// Local copy for optimistic toggle updates; synced when server data changes
	let tournaments = $state(untrack(() => [...data.tournaments]));
	$effect(() => { tournaments = [...data.tournaments]; });

	// Filter state
	let filterOpen  = $state(false);
	let search      = $state('');
	let venueFilter = $state<'all' | 'online' | 'offline'>('all');
	let minEntrants = $state<number | null>(null);
	let maxEntrants = $state<number | null>(null);
	let dateFrom    = $state('');
	let dateTo      = $state('');
	let eventType   = $state<'all' | 'singles' | 'teams'>('all');

	// Bracket type filter
	type BracketTypeState = 'neutral' | 'required' | 'excluded';

	const BRACKET_TYPES = [
		'DOUBLE_ELIMINATION',
		'SINGLE_ELIMINATION',
		'ROUND_ROBIN',
		'MATCHMAKING',
		'SWISS',
		'EXHIBITION',
		'RACE',
		'CIRCUIT',
		'CUSTOM_SCHEDULE',
		'ELIMINATION_ROUNDS',
	] as const;

	// First 5 are common; last 5 are rare (separated by a divider in the UI)
	const COMMON_BRACKET_TYPES = BRACKET_TYPES.slice(0, 5);
	const RARE_BRACKET_TYPES   = BRACKET_TYPES.slice(5);

	const BRACKET_TYPE_LABELS: Record<string, string> = {
		DOUBLE_ELIMINATION:  'Double Elim',
		SINGLE_ELIMINATION:  'Single Elim',
		ROUND_ROBIN:         'Round Robin',
		MATCHMAKING:         'Matchmaking',
		SWISS:               'Swiss',
		EXHIBITION:          'Exhibition',
		RACE:                'Race',
		CIRCUIT:             'Circuit',
		CUSTOM_SCHEDULE:     'Custom Schedule',
		ELIMINATION_ROUNDS:  'Elim. Rounds',
	};

	let bracketFilter = $state<Record<string, BracketTypeState>>(
		Object.fromEntries(BRACKET_TYPES.map(t => [t, 'neutral' as BracketTypeState]))
	);
	let bracketPopoverOpen = $state(false);

	const bracketReqCount  = $derived(Object.values(bracketFilter).filter(s => s === 'required').length);
	const bracketExclCount = $derived(Object.values(bracketFilter).filter(s => s === 'excluded').length);
	const bracketTriggerLabel = $derived(
		bracketReqCount === 0 && bracketExclCount === 0
			? 'Brackets ▾'
			: [
				bracketReqCount  > 0 ? `${bracketReqCount} req`  : '',
				bracketExclCount > 0 ? `${bracketExclCount} excl` : '',
			  ].filter(Boolean).join(' · ') + ' ▾'
	);

	function setBracketState(type: string, clicked: BracketTypeState) {
		// Clicking the already-active state resets to neutral
		bracketFilter = {
			...bracketFilter,
			[type]: bracketFilter[type] === clicked ? 'neutral' : clicked,
		};
	}

	function resetBracketFilter() {
		bracketFilter = Object.fromEntries(
			BRACKET_TYPES.map(t => [t, 'neutral' as BracketTypeState])
		);
	}

	// Close popover on outside click
	$effect(() => {
		if (!bracketPopoverOpen) return;
		function handleClick(ev: MouseEvent) {
			const wrapper = document.getElementById('bracket-popover-wrapper');
			if (wrapper && !wrapper.contains(ev.target as Node)) {
				bracketPopoverOpen = false;
			}
		}
		document.addEventListener('click', handleClick);
		return () => document.removeEventListener('click', handleClick);
	});

	async function toggleEvent(projectId: string, eventId: string, included: boolean) {
		const res = await fetch(`${PUBLIC_API_URL}/projects/${projectId}/events/${eventId}`, {
			method: 'PATCH',
			credentials: 'include',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ included })
		});
		if (!res.ok) return;

		const updated = await res.json();
		tournaments = tournaments.map((t) => ({
			...t,
			events: t.events.map((e) => (e.id === updated.event_id ? { ...e, included: updated.included } : e))
		}));
	}

	function tournamentVisible(t: Tournament): boolean {
		if (venueFilter === 'online' && !t.online) return false;
		if (venueFilter === 'offline' && t.online) return false;
		if (dateFrom && t.start_at && t.start_at.slice(0, 10) < dateFrom) return false;
		if (dateTo && t.start_at && t.start_at.slice(0, 10) > dateTo) return false;
		return true;
	}

	function eventVisible(e: TournamentEvent, t: Tournament): boolean {
		if (search.trim()) {
			const q = search.trim().toLowerCase();
			const nameMatch = e.name.toLowerCase().includes(q);
			const tournamentMatch = t.name.toLowerCase().includes(q);
			if (!nameMatch && !tournamentMatch) return false;
		}
		if (+minEntrants > 0 && (e.num_entrants ?? Infinity) < +minEntrants) return false;
		if (+maxEntrants > 0 && (e.num_entrants ?? 0) > +maxEntrants) return false;
		if (eventType === 'singles' && e.event_type !== null && e.event_type !== 1) return false;
		if (eventType === 'teams' && e.event_type !== null && e.event_type !== 2) return false;

		const required = Object.entries(bracketFilter)
			.filter(([, s]) => s === 'required')
			.map(([t]) => t);
		const excluded = Object.entries(bracketFilter)
			.filter(([, s]) => s === 'excluded')
			.map(([t]) => t);

		if (required.length > 0 || excluded.length > 0) {
			if (e.bracket_types.length === 0) return true;
			for (const r of required) {
				if (!e.bracket_types.includes(r)) return false;
			}
			for (const x of excluded) {
				if (e.bracket_types.includes(x)) return false;
			}
		}

		return true;
	}

	const visibleTournaments = $derived(
		tournaments
			.filter(t => tournamentVisible(t))
			.map(t => ({ ...t, events: t.events.filter(e => eventVisible(e, t)) }))
			.filter(t => t.events.length > 0)
	);

	const totalEventCount   = $derived(tournaments.reduce((n, t) => n + t.events.length, 0));
	const visibleEventCount = $derived(visibleTournaments.reduce((n, t) => n + t.events.length, 0));

	async function bulkSetIncluded(included: boolean) {
		for (const t of visibleTournaments) {
			for (const e of t.events) {
				if (e.included !== included) {
					await toggleEvent(data.project.id, e.id, included);
				}
			}
		}
	}

	function handleToggle(projectId: string, event: TournamentEvent) {
		tournaments = tournaments.map((t) => ({
			...t,
			events: t.events.map((e) => (e.id === event.id ? { ...e, included: !e.included } : e))
		}));
		toggleEvent(projectId, event.id, !event.included);
	}
</script>
```

- [ ] **Step 2: Replace row 3 in the filter panel template**

In the template, find the existing row 3 block (the `<!-- Row 3: event type + ladder -->` comment and its `<div>`). Replace it with:

```svelte
				<!-- Row 3: event type + bracket filter -->
				<div class="flex flex-wrap gap-4 items-center">
					<div class="flex items-center gap-2">
						<span class="text-xs text-muted-foreground whitespace-nowrap">Event type</span>
						<select
							bind:value={eventType}
							class="rounded-md border border-input bg-background px-3 py-1.5 text-sm"
						>
							<option value="all">All</option>
							<option value="singles">Singles</option>
							<option value="teams">Teams</option>
						</select>
					</div>

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
								<div class="flex justify-between items-center mb-2">
									<span class="text-xs text-muted-foreground uppercase tracking-wide">Bracket Types</span>
									<button
										type="button"
										onclick={resetBracketFilter}
										class="text-xs text-muted-foreground hover:text-foreground"
									>Reset</button>
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
							</div>
						{/if}
					</div>
				</div>
```

- [ ] **Step 3: Add the `{#snippet bracketRow}` block**

Add this snippet anywhere in the template before it is used (conventionally right before the filter panel `{#if filterOpen}` block, after the closing `</script>`):

```svelte
{#snippet bracketRow(bt: string)}
	<div class="grid grid-cols-[1fr_28px_28px_28px] gap-1 items-center py-0.5">
		<span class="text-xs">{BRACKET_TYPE_LABELS[bt]}</span>
		{#each (['neutral', 'required', 'excluded'] as const) as s}
			<button
				type="button"
				onclick={() => setBracketState(bt, s)}
				class="h-6 w-6 rounded border text-xs font-bold flex items-center justify-center
					{bracketFilter[bt] === s
						? s === 'required'
							? 'border-green-500 bg-green-950 text-green-400'
							: s === 'excluded'
								? 'border-red-500 bg-red-950 text-red-400'
								: 'border-indigo-500 bg-indigo-950 text-indigo-400'
						: 'border-border bg-muted/30 text-transparent hover:text-muted-foreground'}"
			>
				{s === 'neutral' ? '–' : s === 'required' ? '✓' : '✕'}
			</button>
		{/each}
	</div>
{/snippet}
```

- [ ] **Step 4: Run the unit tests**

```bash
cd web && npm run test:unit -- filter
```

Expected: all tests pass.

- [ ] **Step 5: Start the dev server and verify manually**

```bash
cd web && npm run dev
```

Open `http://localhost:5173`, navigate to a project's Tournaments tab, open the filter panel, and verify:

1. Row 3 shows "Event type" select + "Brackets ▾" button (no old checkbox).
2. Clicking "Brackets ▾" opens the popover with all 10 types.
3. Double Elim / Single Elim / Round Robin / Matchmaking / Swiss appear above the divider; the other 5 below.
4. Clicking `✓` on Double Elim highlights it green and updates the trigger to `1 req ▾`. Events without Double Elim disappear.
5. Clicking `✕` on Matchmaking highlights it red and updates the trigger to `1 req · 1 excl ▾`. Events with Matchmaking disappear.
6. Clicking an already-active button resets it to neutral.
7. Reset button returns all to neutral.
8. Clicking outside the popover closes it.
9. Events with empty `bracket_types` are unaffected by any filter state.

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/[id]/tournaments/+page.svelte
git commit -m "feat(tournaments): replace excludeLadder checkbox with bracket type filter popover"
```
