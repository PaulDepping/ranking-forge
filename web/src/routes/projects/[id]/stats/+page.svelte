<script lang="ts">
	import type { SetRecord } from '$lib/types';
	import SetDetailModal from '$lib/components/SetDetailModal.svelte';
	import * as Card from '$lib/components/ui/card';
	import { ScrollArea } from '$lib/components/ui/scroll-area';
	import * as Empty from '$lib/components/ui/empty';
	import { Button } from '$lib/components/ui/button';
	import { winRate } from '$lib/utils';

	let { data } = $props();

	let selectedSet = $state<SetRecord | null>(null);
	let selectedIsWin = $state(false);
	let selectedPlayerName = $state('');

	const trackedPlayerIds = $derived(new Set(data.stats.map((p) => p.player_id)));

	function openModal(set: SetRecord, isWin: boolean, playerName: string) {
		selectedSet = set;
		selectedIsWin = isWin;
		selectedPlayerName = playerName;
	}
</script>

<div class="space-y-4">
	<h2 class="text-lg font-semibold">Stats</h2>

	{#if data.stats.length === 0}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No stats yet</Empty.Title>
				<Empty.Description>Import tournaments and include some events to generate stats.</Empty.Description>
			</Empty.Header>
		</Empty.Root>
	{:else}
		<div class="grid gap-3" style="grid-template-columns: repeat(auto-fill, minmax(320px, 1fr))">
			{#each data.stats as player (player.player_id)}
				<Card.Root class="py-0">
					<Card.Content class="p-3">
					<div class="mb-2 flex items-baseline justify-between">
						<a href="/projects/{data.project.id}/players/{player.player_id}" class="font-semibold hover:underline">{player.name}</a>
						<span class="text-xs text-muted-foreground">
							W {player.wins.length} · L {player.losses.length} · {winRate(player.wins.length, player.losses.length, '0%')}
						</span>
					</div>
					<div class="flex gap-2">
						<div class="flex-1">
							<p class="mb-1 text-xs font-semibold uppercase tracking-wide text-green-600 dark:text-green-400">
								WINS ({player.wins.length})
							</p>
							<ScrollArea class="h-24 rounded border border-border bg-muted/20">
								{#each player.wins as set, i (i)}
									<Button
										variant="ghost"
										class="h-auto w-full border-b border-border px-2 py-1 text-xs last:border-0 justify-start whitespace-normal"
										onclick={() => openModal(set, true, player.name)}
									>
										{set.opponent_name} · UF {set.upset_factor}
									</Button>
								{/each}
							</ScrollArea>
						</div>
						<div class="flex-1">
							<p class="mb-1 text-xs font-semibold uppercase tracking-wide text-red-600 dark:text-red-400">
								LOSSES ({player.losses.length})
							</p>
							<ScrollArea class="h-24 rounded border border-border bg-muted/20">
								{#each player.losses as set, i (i)}
									<Button
										variant="ghost"
										class="h-auto w-full border-b border-border px-2 py-1 text-xs last:border-0 justify-start whitespace-normal"
										onclick={() => openModal(set, false, player.name)}
									>
										{set.opponent_name} · UF {set.upset_factor}
									</Button>
								{/each}
							</ScrollArea>
						</div>
					</div>
					</Card.Content>
				</Card.Root>
			{/each}
		</div>
	{/if}
</div>

<SetDetailModal
	set={selectedSet}
	isWin={selectedIsWin}
	currentPlayerName={selectedPlayerName}
	onClose={() => (selectedSet = null)}
	projectId={data.project.id}
	opponentPlayerId={selectedSet && trackedPlayerIds.has(selectedSet.opponent_id)
		? selectedSet.opponent_id
		: undefined}
/>
