<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import type { SetRecord } from '$lib/types';
	import { formatDate } from '$lib/utils';

	interface Props {
		set: SetRecord | null;
		isWin: boolean;
		currentPlayerName: string;
		onClose: () => void;
	}

	let { set, isWin, currentPlayerName, onClose }: Props = $props();

	let open = $derived(set !== null);

	function toOrdinal(n: number): string {
		const s = ['th', 'st', 'nd', 'rd'];
		const v = n % 100;
		return n + (s[(v - 20) % 10] ?? s[v] ?? s[0]);
	}

	function phaseLabel(set: SetRecord): string | null {
		if (!set.phase_name) return null;
		return set.pool_identifier
			? `${set.phase_name} · ${set.pool_identifier}`
			: set.phase_name;
	}
</script>

<Dialog.Root
	{open}
	onOpenChange={(o) => {
		if (!o) onClose();
	}}
>
	<Dialog.Content class="max-w-sm">
		{#if set}
			<Dialog.Header>
				<Dialog.Title>{currentPlayerName} vs {set.opponent_name}</Dialog.Title>
				<Dialog.Description
					class={isWin
						? 'text-green-600 dark:text-green-400'
						: 'text-red-600 dark:text-red-400'}
				>
					{isWin ? 'Win' : 'Loss'}
				</Dialog.Description>
			</Dialog.Header>

			<div class="space-y-4 py-2 text-sm">
				<!-- Match -->
				<div>
					<p class="mb-1 border-b pb-1 text-xs uppercase tracking-wide text-muted-foreground">
						Match
					</p>
					<div class="grid grid-cols-2 gap-x-4 gap-y-3">
						{#if set.winner_score !== null || set.loser_score !== null}
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">
									{currentPlayerName} score
								</p>
								<p
									class={isWin
										? 'font-semibold text-green-600 dark:text-green-400'
										: 'font-semibold text-red-600 dark:text-red-400'}
								>
									{isWin ? (set.winner_score ?? '?') : (set.loser_score ?? '?')}
								</p>
							</div>
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">
									{set.opponent_name} score
								</p>
								<p
									class={isWin
										? 'font-semibold text-red-600 dark:text-red-400'
										: 'font-semibold text-green-600 dark:text-green-400'}
								>
									{isWin ? (set.loser_score ?? '?') : (set.winner_score ?? '?')}
								</p>
							</div>
						{/if}
						{#if set.winner_seed !== null || set.loser_seed !== null}
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">
									{currentPlayerName} seed
								</p>
								<p class={isWin ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'}>
									#{isWin ? (set.winner_seed ?? '?') : (set.loser_seed ?? '?')}
								</p>
							</div>
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">
									{set.opponent_name} seed
								</p>
								<p class={isWin ? 'text-red-600 dark:text-red-400' : 'text-green-600 dark:text-green-400'}>
									#{isWin ? (set.loser_seed ?? '?') : (set.winner_seed ?? '?')}
								</p>
							</div>
						{/if}
						<div>
							<p class="text-xs uppercase tracking-wide text-muted-foreground">Upset Factor</p>
							<p>{set.upset_factor}</p>
						</div>
					</div>
				</div>

				<!-- Tournament -->
				<div>
					<p class="mb-1 border-b pb-1 text-xs uppercase tracking-wide text-muted-foreground">
						Tournament
					</p>
					<div class="grid grid-cols-2 gap-x-4 gap-y-3">
						<div class="col-span-2">
							<p class="text-xs uppercase tracking-wide text-muted-foreground">Name</p>
							<p>{set.tournament_name} · {set.event_name}</p>
						</div>
						{#if phaseLabel(set)}
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">Phase</p>
								<p>{phaseLabel(set)}</p>
							</div>
						{/if}
						{#if set.round_name}
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">Round</p>
								<p>{set.round_name}</p>
							</div>
						{/if}
						{#if set.location}
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">Location</p>
								<p>{set.location}</p>
							</div>
						{/if}
						<div>
							<p class="text-xs uppercase tracking-wide text-muted-foreground">Date</p>
							<p>{formatDate(set.completed_at)}</p>
						</div>
						{#if set.num_entrants}
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">Entrants</p>
								<p>{set.num_entrants}</p>
							</div>
						{/if}
					</div>
				</div>

				<!-- Final Placements -->
				{#if set.winner_placement !== null || set.loser_placement !== null}
					<div>
						<p class="mb-1 border-b pb-1 text-xs uppercase tracking-wide text-muted-foreground">
							Final Placements
						</p>
						<div class="grid grid-cols-2 gap-x-4 gap-y-3">
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">
									{currentPlayerName}
								</p>
								<p class={isWin ? 'text-green-600 dark:text-green-400' : 'text-red-600 dark:text-red-400'}>
									{isWin
										? (set.winner_placement !== null ? toOrdinal(set.winner_placement) : '?')
										: (set.loser_placement !== null ? toOrdinal(set.loser_placement) : '?')}
								</p>
							</div>
							<div>
								<p class="text-xs uppercase tracking-wide text-muted-foreground">
									{set.opponent_name}
								</p>
								<p class={isWin ? 'text-red-600 dark:text-red-400' : 'text-green-600 dark:text-green-400'}>
									{isWin
										? (set.loser_placement !== null ? toOrdinal(set.loser_placement) : '?')
										: (set.winner_placement !== null ? toOrdinal(set.winner_placement) : '?')}
								</p>
							</div>
						</div>
					</div>
				{/if}
			</div>

			<div class="flex gap-4 border-t pt-3 text-sm">
				<a
					href={set.event_slug
						? `https://www.start.gg/${set.tournament_slug}/event/${set.event_slug}/set/${set.startgg_set_id}`
						: `https://www.start.gg/${set.tournament_slug}`}
					target="_blank"
					rel="noopener noreferrer"
					class="text-primary hover:underline"
				>↗ View set on start.gg</a>
				{#if set.vod_url}
					<a
						href={set.vod_url}
						target="_blank"
						rel="noopener noreferrer"
						class="text-primary hover:underline"
					>▶ Watch VOD</a>
				{/if}
			</div>
		{/if}
	</Dialog.Content>
</Dialog.Root>
