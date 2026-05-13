<script lang="ts">
	import type { HeadToHeadEntry } from '$lib/types';

	let { data } = $props();

	function getRecord(rowId: string, colId: string): HeadToHeadEntry | undefined {
		return data.h2h.find((e) => e.player_id === rowId && e.opponent_id === colId);
	}
</script>

<div class="space-y-4">
	<h2 class="text-lg font-semibold">Head-to-head</h2>

	{#if data.players.length < 2 || data.h2h.length === 0}
		<p class="text-sm text-muted-foreground">No head-to-head data yet. Import tournaments first.</p>
	{:else}
		<div class="overflow-x-auto">
			<table class="text-sm border-collapse">
				<thead>
					<tr>
						<th class="w-32 pb-2 text-left text-muted-foreground font-normal pr-3"></th>
						{#each data.players as col (col.id)}
							<th class="pb-2 px-2 text-center font-medium" style="min-width:5rem">
								<span class="block truncate max-w-[5rem]" title={col.name}>{col.name}</span>
							</th>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each data.players as row (row.id)}
						<tr>
							<td class="py-1 pr-3 font-medium truncate max-w-[8rem]" title={row.name}>{row.name}</td>
							{#each data.players as col (col.id)}
								{#if row.id === col.id}
									<td class="py-1 px-2 text-center text-muted-foreground">—</td>
								{:else}
									{@const rec = getRecord(row.id, col.id)}
									<td class="py-1 px-2 text-center tabular-nums rounded
										{rec ? (rec.wins > rec.losses ? 'bg-green-50 dark:bg-green-950/30' : rec.wins < rec.losses ? 'bg-red-50 dark:bg-red-950/30' : '') : ''}">
										{#if rec}
											<span class={rec.wins > rec.losses ? 'text-green-700 dark:text-green-400' : rec.wins < rec.losses ? 'text-red-700 dark:text-red-400' : ''}>
												{rec.wins}–{rec.losses}
											</span>
										{:else}
											<span class="text-muted-foreground">—</span>
										{/if}
									</td>
								{/if}
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<p class="text-xs text-muted-foreground">Row player's record vs. column player</p>
	{/if}
</div>
