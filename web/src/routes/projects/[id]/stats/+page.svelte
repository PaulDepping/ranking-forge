<script lang="ts">
	import type { SetRecord } from '$lib/types';

	let { data } = $props();

	let expanded = $state<Record<string, 'wins' | 'losses' | null>>({});

	function toggle(playerId: string, tab: 'wins' | 'losses') {
		expanded[playerId] = expanded[playerId] === tab ? null : tab;
	}

	function totalUF(wins: SetRecord[]) {
		return wins.reduce((s, r) => s + r.upset_factor, 0);
	}
</script>

<div class="space-y-4">
	<h2 class="text-lg font-semibold">Stats</h2>

	{#if data.stats.length === 0}
		<p class="text-sm text-muted-foreground">No stats yet. Import tournaments and include some events first.</p>
	{:else}
		<div class="rounded-md border border-border">
			<table class="w-full text-sm">
				<thead>
					<tr class="border-b border-border bg-muted/50">
						<th class="py-2 pl-4 text-left font-medium">#</th>
						<th class="py-2 text-left font-medium">Player</th>
						<th class="py-2 text-right font-medium pr-2">Agg. UF</th>
						<th class="py-2 text-right font-medium pr-2">Wins</th>
						<th class="py-2 text-right font-medium pr-4">Losses</th>
					</tr>
				</thead>
				<tbody>
					{#each data.stats as player, i (player.player_id)}
						<tr class="border-b border-border last:border-0">
							<td class="py-2 pl-4 text-muted-foreground">{i + 1}</td>
							<td class="py-2 font-medium">{player.name}</td>
							<td class="py-2 text-right pr-2 tabular-nums">{totalUF(player.wins).toFixed(1)}</td>
							<td class="py-2 text-right pr-2">
								<button
									onclick={() => toggle(player.player_id, 'wins')}
									class="text-primary underline"
								>{player.wins.length}</button>
							</td>
							<td class="py-2 text-right pr-4">
								<button
									onclick={() => toggle(player.player_id, 'losses')}
									class="text-muted-foreground underline"
								>{player.losses.length}</button>
							</td>
						</tr>
						{#if expanded[player.player_id]}
							{@const sets = expanded[player.player_id] === 'wins' ? player.wins : player.losses}
							{@const label = expanded[player.player_id] === 'wins' ? 'Wins' : 'Losses'}
							<tr class="border-b border-border bg-muted/30">
								<td colspan={5} class="px-8 py-3">
									<p class="mb-2 text-xs font-semibold text-muted-foreground">{label}</p>
									<div class="space-y-1">
										{#each sets as set (set.opponent_id + set.upset_factor)}
											<div class="flex justify-between text-xs">
												<span>{set.opponent_name}</span>
												<span class="tabular-nums text-muted-foreground">UF {set.upset_factor.toFixed(1)}</span>
											</div>
										{/each}
									</div>
								</td>
							</tr>
						{/if}
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</div>
