<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import type { SetRecord } from '$lib/types';

	interface Props {
		set: SetRecord | null;
		isWin: boolean;
		currentPlayerName: string;
		onClose: () => void;
	}

	let { set, isWin, currentPlayerName, onClose }: Props = $props();

	let open = $derived(set !== null);

	function formatDate(s: string | null): string {
		if (!s) return 'Unknown';
		return new Date(s).toLocaleDateString('en-US', {
			month: 'short',
			day: 'numeric',
			year: 'numeric'
		});
	}

	function score(): string {
		if (!set || set.winner_score === null || set.loser_score === null) return '';
		return isWin
			? `${set.winner_score}–${set.loser_score}`
			: `${set.loser_score}–${set.winner_score}`;
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
					{isWin ? 'Win' : 'Loss'}{score() ? ` · ${score()}` : ''}
				</Dialog.Description>
			</Dialog.Header>
			<div class="grid grid-cols-2 gap-x-4 gap-y-3 py-2 text-sm">
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Tournament</p>
					<p>{set.tournament_name}</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Event</p>
					<p>{set.event_name}</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Round</p>
					<p>{set.round_name ?? 'Unknown'}</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Date</p>
					<p>{formatDate(set.completed_at)}</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Seeds</p>
					<p>
						<span class="text-green-600 dark:text-green-400">{set.winner_seed ?? '?'}</span>
						<span class="text-muted-foreground"> vs </span>
						<span class="text-red-600 dark:text-red-400">{set.loser_seed ?? '?'}</span>
					</p>
				</div>
				<div>
					<p class="text-xs uppercase tracking-wide text-muted-foreground">Upset Factor</p>
					<p>{set.upset_factor}</p>
				</div>
			</div>
			{#if set.tournament_slug || set.vod_url}
				<div class="flex gap-4 border-t pt-3 text-sm">
					{#if set.tournament_slug}
						<a
							href="https://www.start.gg/{set.tournament_slug}"
							target="_blank"
							rel="noopener noreferrer"
							class="text-primary hover:underline"
						>↗ View on start.gg</a>
					{/if}
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
		{/if}
	</Dialog.Content>
</Dialog.Root>
