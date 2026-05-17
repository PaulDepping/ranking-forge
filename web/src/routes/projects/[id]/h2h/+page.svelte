<script lang="ts">
	import { PUBLIC_API_URL } from '$env/static/public';
	import { makeApi } from '$lib/api';
	import type { HeadToHeadEntry, H2HSet } from '$lib/types';
	import SetDetailModal from '$lib/components/SetDetailModal.svelte';
	import * as Table from '$lib/components/ui/table';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import * as Empty from '$lib/components/ui/empty';

	let { data } = $props();

	interface SelectedPair {
		rowPlayer: { id: string; name: string };
		colPlayer: { id: string; name: string };
		sets: H2HSet[];
		wins: number;
		losses: number;
	}

	let selectedPair = $state<SelectedPair | null>(null);
	let loading = $state(false);
	let selectedSet = $state<H2HSet | null>(null);
	let selectedIsWin = $state(false);

	async function selectCell(
		rowPlayer: { id: string; name: string },
		colPlayer: { id: string; name: string }
	) {
		if (
			selectedPair?.rowPlayer.id === rowPlayer.id &&
			selectedPair?.colPlayer.id === colPlayer.id
		) {
			selectedPair = null;
			return;
		}
		loading = true;
		selectedPair = null;
		try {
			const api = makeApi(fetch, PUBLIC_API_URL);
			const res = await api.get(
				`/projects/${data.project.id}/head-to-head/${rowPlayer.id}/${colPlayer.id}/sets`
			);
			const sets: H2HSet[] = res.ok ? await res.json() : [];
			selectedPair = {
				rowPlayer,
				colPlayer,
				sets,
				wins: sets.filter((s) => s.is_win).length,
				losses: sets.filter((s) => !s.is_win).length,
			};
		} finally {
			loading = false;
		}
	}

	function isSelected(rowId: string, colId: string): boolean {
		return selectedPair?.rowPlayer.id === rowId && selectedPair?.colPlayer.id === colId;
	}

	function getRecord(rowId: string, colId: string): HeadToHeadEntry | undefined {
		return data.h2h.find((e: HeadToHeadEntry) => e.player_id === rowId && e.opponent_id === colId);
	}
</script>

<div class="space-y-4">
	<h2 class="text-lg font-semibold">Head-to-head</h2>

	{#if data.players.length < 2 || data.h2h.length === 0}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No head-to-head data yet</Empty.Title>
				<Empty.Description>Import tournaments to generate head-to-head records.</Empty.Description>
			</Empty.Header>
		</Empty.Root>
	{:else}
		<div class="flex gap-4 items-start flex-wrap">
			<!-- Matrix -->
			<div>
				<Table.Root class="border-collapse">
					<Table.Header>
						<Table.Row>
							<Table.Head class="w-32 pb-2 pr-3 font-normal text-muted-foreground h-auto"></Table.Head>
							{#each data.players as col (col.id)}
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
							{/each}
						</Table.Row>
					</Table.Header>
					<Table.Body>
						{#each data.players as row (row.id)}
							<Table.Row class="border-0 hover:bg-transparent">
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

			<!-- Side panel -->
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
			{:else if selectedPair}
				<Card.Root class="py-0 min-w-[220px] flex-1 max-w-xs">
					<Card.Content class="p-3">
						<div class="mb-3 flex items-start justify-between gap-2 border-b border-border pb-2">
							<div>
								<p class="font-semibold text-sm">{selectedPair.rowPlayer.name} vs {selectedPair.colPlayer.name}</p>
								<p class="text-xs text-muted-foreground">{selectedPair.wins} wins · {selectedPair.losses} losses</p>
							</div>
							<Button
								variant="ghost"
								size="icon"
								onclick={() => (selectedPair = null)}
								aria-label="Close panel"
							>×</Button>
						</div>
						{#if selectedPair.sets.length === 0}
							<p class="text-xs text-muted-foreground">No sets found.</p>
						{:else}
							<div class="space-y-px">
								{#each selectedPair.sets as set, i (i)}
									<button
										class="w-full flex items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-muted/50 border-b border-border last:border-0"
										onclick={() => { selectedSet = set; selectedIsWin = set.is_win; }}
									>
										<span class={set.is_win ? 'font-bold text-green-600 dark:text-green-400 min-w-[12px]' : 'font-bold text-red-600 dark:text-red-400 min-w-[12px]'}>
											{set.is_win ? 'W' : 'L'}
										</span>
										{#if set.winner_score !== null && set.loser_score !== null}
											<span class="tabular-nums">
												{set.is_win ? `${set.winner_score}–${set.loser_score}` : `${set.loser_score}–${set.winner_score}`}
											</span>
										{/if}
										<span class="text-muted-foreground truncate flex-1 text-left">{set.tournament_name}</span>
										{#if set.round_name}
											<span class="text-muted-foreground shrink-0">{set.round_name}</span>
										{/if}
									</button>
								{/each}
							</div>
						{/if}
						<p class="mt-2 text-xs text-muted-foreground">Click a row for full details</p>
					</Card.Content>
				</Card.Root>
			{/if}
		</div>
	{/if}
</div>

<SetDetailModal
	set={selectedSet}
	isWin={selectedIsWin}
	currentPlayerName={selectedPair?.rowPlayer.name ?? ''}
	onClose={() => (selectedSet = null)}
/>
