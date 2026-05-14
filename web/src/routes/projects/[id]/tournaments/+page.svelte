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
	let filterOpen    = $state(false);
	let search        = $state('');
	let venueFilter   = $state<'all' | 'online' | 'offline'>('all');
	let minEntrants   = $state<number | null>(null);
	let maxEntrants   = $state<number | null>(null);
	let dateFrom      = $state('');
	let dateTo        = $state('');
	let eventType     = $state<'all' | 'singles' | 'teams'>('all');
	let excludeLadder = $state(false);

	async function toggleEvent(projectId: string, eventId: string, included: boolean) {
		const res = await fetch(`${PUBLIC_API_URL}/projects/${projectId}/events/${eventId}`, {
			method: 'PATCH',
			credentials: 'include',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ included })
		});
		if (!res.ok) return;

		// Optimistic update already applied; sync from server response
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
		if (minEntrants !== null && (e.num_entrants ?? Infinity) < minEntrants) return false;
		if (maxEntrants !== null && (e.num_entrants ?? 0) > maxEntrants) return false;
		if (eventType === 'singles' && e.event_type !== null && e.event_type !== 1) return false;
		if (eventType === 'teams' && e.event_type !== null && e.event_type !== 2) return false;
		if (excludeLadder && e.bracket_types.length > 0 &&
			e.bracket_types.every(bt => bt === 'MATCHMAKING')) return false;
		return true;
	}

	const visibleTournaments = $derived(
		tournaments
			.filter(t => tournamentVisible(t))
			.map(t => ({ ...t, events: t.events.filter(e => eventVisible(e, t)) }))
			.filter(t => t.events.length > 0)
	);

	const totalEventCount = $derived(tournaments.reduce((n, t) => n + t.events.length, 0));
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
		// Optimistic update
		tournaments = tournaments.map((t) => ({
			...t,
			events: t.events.map((e) => (e.id === event.id ? { ...e, included: !e.included } : e))
		}));
		toggleEvent(projectId, event.id, !event.included);
	}
</script>

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
				<!-- Row 1: search + venue -->
				<div class="flex flex-wrap gap-2">
					<input
						type="text"
						placeholder="Search tournament or event name…"
						bind:value={search}
						class="flex-1 min-w-48 rounded-md border border-input bg-background px-3 py-1.5 text-sm"
					/>
					<select
						bind:value={venueFilter}
						class="rounded-md border border-input bg-background px-3 py-1.5 text-sm"
					>
						<option value="all">Venue: All</option>
						<option value="online">Online only</option>
						<option value="offline">Offline only</option>
					</select>
				</div>

				<!-- Row 2: entrant range + date range -->
				<div class="flex flex-wrap gap-2 items-center">
					<div class="flex items-center gap-1.5">
						<span class="text-xs text-muted-foreground whitespace-nowrap">Entrants</span>
						<input
							type="number"
							min="0"
							placeholder="min"
							bind:value={minEntrants}
							class="w-20 rounded-md border border-input bg-background px-2 py-1.5 text-sm"
						/>
						<span class="text-muted-foreground">–</span>
						<input
							type="number"
							min="0"
							placeholder="max"
							bind:value={maxEntrants}
							class="w-20 rounded-md border border-input bg-background px-2 py-1.5 text-sm"
						/>
					</div>
					<div class="flex items-center gap-1.5">
						<span class="text-xs text-muted-foreground">From</span>
						<input
							type="date"
							bind:value={dateFrom}
							class="rounded-md border border-input bg-background px-2 py-1.5 text-sm"
						/>
						<span class="text-xs text-muted-foreground">To</span>
						<input
							type="date"
							bind:value={dateTo}
							class="rounded-md border border-input bg-background px-2 py-1.5 text-sm"
						/>
					</div>
				</div>

				<!-- Row 3: event type + ladder -->
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
					<label class="flex items-center gap-2 cursor-pointer text-sm">
						<input type="checkbox" bind:checked={excludeLadder} class="h-4 w-4 accent-primary" />
						Exclude ladder / matchmaking
					</label>
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
								<input
									type="checkbox"
									checked={event.included}
									onchange={() => handleToggle(data.project.id, event)}
									class="h-4 w-4 rounded border-border accent-primary"
								/>
							</label>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>
