<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { PUBLIC_API_URL } from '$env/static/public';
	import type { TournamentEvent } from '$lib/types';

	let { data } = $props();

	// Local copy for optimistic toggle updates
	let tournaments = $state([...data.tournaments]);

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
