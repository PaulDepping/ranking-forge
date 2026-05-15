<script lang="ts">
	import { untrack } from 'svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
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

	const venueLabel = $derived(
		({ all: 'Venue: All', online: 'Online only', offline: 'Offline only' } as const)[venueFilter]
	);
	const eventTypeLabel = $derived(
		({ all: 'All types', singles: 'Singles', teams: 'Teams' } as const)[eventType]
	);

	async function bulkSetIncluded(included: boolean) {
		const toChange = visibleTournaments
			.flatMap(t => t.events)
			.filter(e => e.included !== included);

		if (toChange.length === 0) return;

		// Optimistic update so checkboxes reflect the change immediately
		const idSet = new Set(toChange.map(e => e.id));
		tournaments = tournaments.map(t => ({
			...t,
			events: t.events.map(e => idSet.has(e.id) ? { ...e, included } : e)
		}));

		await Promise.all(toChange.map(e => toggleEvent(data.project.id, e.id, included)));
	}

	function handleToggle(projectId: string, event: TournamentEvent) {
		tournaments = tournaments.map((t) => ({
			...t,
			events: t.events.map((e) => (e.id === event.id ? { ...e, included: !e.included } : e))
		}));
		toggleEvent(projectId, event.id, !event.included);
	}
</script>

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

<div class="space-y-4">
	<h2 class="text-lg font-semibold">Tournaments</h2>

	{#if tournaments.length === 0}
		<p class="text-sm text-muted-foreground">No tournaments imported yet. Run an import first.</p>
	{:else}
		<!-- Status line + toggle -->
		<div class="flex items-center justify-between text-sm text-muted-foreground">
			<span>
				Showing <strong>{visibleTournaments.length}</strong> of {tournaments.length} tournaments
				· <strong>{visibleEventCount}</strong> of {totalEventCount} events
			</span>
			<Button variant="outline" size="sm" onclick={() => (filterOpen = !filterOpen)}>
				⚙ Filters &amp; Actions {filterOpen ? '▲' : '▼'}
			</Button>
		</div>

		<!-- Collapsible filter panel -->
		{#if filterOpen}
			<div class="rounded-md border border-border bg-muted/30 p-4 space-y-3">
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
							</div>
						{/if}
					</div>
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
		{/if}

		<!-- Tournament list — iterate visibleTournaments -->
		{#if visibleTournaments.length === 0}
			<p class="text-sm text-muted-foreground">No tournaments match the current filters.</p>
		{/if}
		<div class="space-y-3">
			{#each visibleTournaments as tournament (tournament.id)}
				<div class="rounded-md border border-border">
					<div class="flex items-start justify-between p-3">
						<div>
							<p class="font-medium">{tournament.name}</p>
							<p class="text-xs text-muted-foreground">
								{[tournament.city, tournament.addr_state, tournament.country_code]
									.filter(Boolean)
									.join(', ')}
								{tournament.online ? '(Online)' : ''}
								{tournament.start_at ? '· ' + new Date(tournament.start_at).toLocaleDateString() : ''}
							</p>
						</div>
						<Badge variant="outline">
							{tournament.events.length} event{tournament.events.length !== 1 ? 's' : ''}
						</Badge>
					</div>
					<div class="divide-y divide-border border-t border-border">
						{#each tournament.events as event (event.id)}
							<label class="flex cursor-pointer items-center justify-between px-4 py-2 hover:bg-accent/50">
								<div>
									<span class="text-sm">{event.name}</span>
									{#if event.num_entrants}
										<span class="ml-2 text-xs text-muted-foreground">{event.num_entrants} entrants</span>
									{/if}
								</div>
								<Checkbox
									checked={event.included}
									onCheckedChange={() => handleToggle(data.project.id, event)}
								/>
							</label>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
