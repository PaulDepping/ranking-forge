<script lang="ts">
	import { untrack } from 'svelte';
	import { Badge } from '$lib/components/ui/badge';
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
		<div class="space-y-3">
			{#each tournaments as tournament (tournament.id)}
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
						<Badge variant="outline">{tournament.events.length} event{tournament.events.length !== 1 ? 's' : ''}</Badge>
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
