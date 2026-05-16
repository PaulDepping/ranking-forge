<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import { ScrollArea } from '$lib/components/ui/scroll-area';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { makeApi } from '$lib/api';
	import { invalidateAll } from '$app/navigation';
	import type { Player, TournamentEntrant } from '$lib/types';

	let {
		projectId,
		players,
		onClose
	}: { projectId: string; players: Player[]; onClose: () => void } = $props();

	let tournamentInput = $state('');
	let loading = $state(false);
	let fetchError = $state<string | null>(null);
	let entrants = $state<TournamentEntrant[]>([]);
	let search = $state('');
	let selected = $state(new Set<number>());
	let submitting = $state(false);

	const alreadyAddedIds = $derived(
		new Set(players.flatMap((p) => p.accounts.map((a) => a.startgg_user_id)))
	);

	const filteredEntrants = $derived(
		entrants.filter((e) => {
			const q = search.toLowerCase();
			return e.name.toLowerCase().includes(q) || e.handle.toLowerCase().includes(q);
		})
	);

	const selectedCount = $derived(selected.size);
	const alreadyAddedCount = $derived(
		entrants.filter((e) => alreadyAddedIds.has(e.startgg_user_id)).length
	);

	async function fetchEntrants() {
		if (!tournamentInput.trim()) return;
		loading = true;
		fetchError = null;
		entrants = [];
		selected = new Set();
		const api = makeApi(fetch, PUBLIC_API_URL);
		const res = await api.get(
			`/projects/${projectId}/tournament-entrants?tournament=${encodeURIComponent(tournamentInput.trim())}`
		);
		loading = false;
		if (res.ok) {
			entrants = await res.json();
		} else {
			const err = await res.json().catch(() => ({ message: 'Failed to fetch entrants' }));
			fetchError = err.message;
		}
	}

	function toggleEntrant(id: number) {
		const next = new Set(selected);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selected = next;
	}

	async function addSelected() {
		const entries = entrants
			.filter((e) => selected.has(e.startgg_user_id))
			.map((e) => ({ name: e.name, startgg_user_id: e.startgg_user_id, handle: e.handle }));
		if (!entries.length) return;
		submitting = true;
		const api = makeApi(fetch, PUBLIC_API_URL);
		const res = await api.post(`/projects/${projectId}/players/bulk`, { players: entries });
		submitting = false;
		if (res.ok) {
			await invalidateAll();
			onClose();
		}
	}
</script>

<div class="space-y-3">
	<div class="flex gap-2">
		<div class="flex-1">
			<Label for="tournament-input" class="sr-only">Tournament URL or handle</Label>
			<Input
				id="tournament-input"
				bind:value={tournamentInput}
				placeholder="genesis-9 or start.gg/tournament/genesis-9"
				disabled={loading}
			/>
		</div>
		<Button onclick={fetchEntrants} disabled={loading || !tournamentInput.trim()}>
			{loading ? 'Fetching…' : 'Fetch'}
		</Button>
	</div>

	{#if fetchError}
		<p class="text-sm text-destructive">{fetchError}</p>
	{/if}

	{#if entrants.length > 0}
		<Input bind:value={search} placeholder="Search entrants…" />
		<ScrollArea class="h-52 rounded-md border">
			<div class="divide-y">
				{#each filteredEntrants as entrant (entrant.startgg_user_id)}
					{@const isAdded = alreadyAddedIds.has(entrant.startgg_user_id)}
					<div class="flex items-center gap-3 px-3 py-2 text-sm" class:opacity-50={isAdded}>
						<Checkbox
							id="entrant-{entrant.startgg_user_id}"
							checked={selected.has(entrant.startgg_user_id)}
							disabled={isAdded}
							onCheckedChange={() => !isAdded && toggleEntrant(entrant.startgg_user_id)}
						/>
						<label
							for="entrant-{entrant.startgg_user_id}"
							class="flex flex-1 cursor-pointer items-center gap-2"
							class:cursor-default={isAdded}
						>
							<span class="font-medium">{entrant.name}</span>
							<span class="text-muted-foreground">{entrant.handle}</span>
						</label>
						{#if isAdded}
							<Badge variant="secondary" class="text-xs">already added</Badge>
						{/if}
					</div>
				{/each}
			</div>
		</ScrollArea>
		<div class="flex items-center justify-between">
			<span class="text-sm text-muted-foreground">
				{selectedCount} selected · {alreadyAddedCount} already added
			</span>
			<Button onclick={addSelected} disabled={selectedCount === 0 || submitting}>
				{submitting ? 'Adding…' : `Add ${selectedCount} player${selectedCount === 1 ? '' : 's'}`}
			</Button>
		</div>
	{/if}
</div>
