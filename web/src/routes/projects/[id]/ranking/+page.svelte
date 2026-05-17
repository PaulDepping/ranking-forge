<script lang="ts">
	import { untrack } from 'svelte';
	import { dragHandleZone, dragHandle } from 'svelte-dnd-action';
	import type { DndEvent } from 'svelte-dnd-action';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { makeApi } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Empty from '$lib/components/ui/empty';
	import type { PlayerStats } from '$lib/types';

	let { data } = $props();

	type RankItem = { id: string; name: string };

	let items = $state<RankItem[]>(untrack(() => data.players.map((p: RankItem) => ({ id: p.id, name: p.name }))));
	let savedIds = $state<string[]>(untrack(() => data.players.map((p: RankItem) => p.id)));

	const statsMap = $derived<Record<string, PlayerStats>>(
		Object.fromEntries((data.stats as PlayerStats[]).map((s) => [s.player_id, s]))
	);

	const hasChanges = $derived(
		items.length !== savedIds.length || items.some((item, i) => item.id !== savedIds[i])
	);

	let saveStatus = $state<'idle' | 'saving' | 'saved'>('idle');

	function handleConsider(e: CustomEvent<DndEvent<RankItem>>) {
		items = e.detail.items;
	}

	function handleFinalize(e: CustomEvent<DndEvent<RankItem>>) {
		items = e.detail.items;
	}

	let editingId = $state<string | null>(null);
	let editingValue = $state('');
	let editInput = $state<HTMLInputElement | null>(null);

	$effect(() => {
		if (editingId && editInput) {
			editInput.focus();
			editInput.select();
		}
	});

	function startEdit(id: string, rank: number) {
		editingId = id;
		editingValue = String(rank);
	}

	function commitEdit() {
		if (!editingId) return;
		const n = parseInt(editingValue, 10);
		if (!isNaN(n)) {
			const clamped = Math.max(1, Math.min(n, items.length));
			const idx = items.findIndex((i) => i.id === editingId);
			if (idx !== -1) {
				const copy = [...items];
				const [item] = copy.splice(idx, 1);
				copy.splice(clamped - 1, 0, item);
				items = copy;
			}
		}
		editingId = null;
	}

	function onRankKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter') commitEdit();
		if (e.key === 'Escape') editingId = null;
	}

	async function save() {
		saveStatus = 'saving';
		const api = makeApi(fetch, PUBLIC_API_URL);
		const res = await api.putRanking(data.project.id, items.map((i) => i.id));
		if (res.ok) {
			savedIds = items.map((i) => i.id);
			saveStatus = 'saved';
			setTimeout(() => { saveStatus = 'idle'; }, 2000);
		} else {
			saveStatus = 'idle';
		}
	}

	function wlRecord(s: PlayerStats | undefined): string {
		if (!s) return '';
		return `${s.wins.length}W · ${s.losses.length}L`;
	}

	function winRate(s: PlayerStats | undefined): string {
		if (!s) return '';
		const total = s.wins.length + s.losses.length;
		if (total === 0) return '';
		return `${Math.round((s.wins.length / total) * 100)}%`;
	}

	function isMoved(id: string, currentIndex: number): boolean {
		return hasChanges && savedIds[currentIndex] !== id;
	}
</script>

{#if data.players.length === 0}
	<Empty.Root>
		<Empty.Header>
			<Empty.Title>No players</Empty.Title>
			<Empty.Description>Add players to start building your ranking.</Empty.Description>
		</Empty.Header>
	</Empty.Root>
{:else}
	<div class="space-y-4">
		<div class="flex items-center justify-between">
			<h2 class="text-lg font-semibold">Ranking</h2>
			<div class="flex items-center gap-3">
				{#if hasChanges && saveStatus !== 'saved'}
					<span class="text-sm text-muted-foreground">Unsaved changes</span>
				{/if}
				<Button
					onclick={save}
					disabled={!hasChanges || saveStatus === 'saving'}
					size="sm"
					variant={saveStatus === 'saved' ? 'outline' : 'default'}
				>
					{saveStatus === 'saving' ? 'Saving…' : saveStatus === 'saved' ? 'Saved ✓' : 'Save'}
				</Button>
			</div>
		</div>

		<div
			class="flex max-w-xl flex-col gap-1"
			use:dragHandleZone={{ items, flipDurationMs: 0 }}
			onconsider={handleConsider}
			onfinalize={handleFinalize}
		>
			{#each items as item, i (item.id)}
				{@const s = statsMap[item.id]}
				{@const moved = isMoved(item.id, i)}
				<div
					class="flex items-center gap-3 rounded-md border px-3 py-2.5 text-sm transition-colors
					{moved ? 'border-primary/40 bg-primary/5' : 'bg-card'}"
				>
					<span
						use:dragHandle
						class="cursor-grab select-none text-base text-muted-foreground active:cursor-grabbing"
					>
						⠿
					</span>

					{#if editingId === item.id}
						<Input
							bind:ref={editInput}
							type="number"
							class="h-7 w-12 px-1 text-center text-xs [appearance:textfield]"
							bind:value={editingValue}
							onblur={commitEdit}
							onkeydown={onRankKeydown}
						/>
					{:else}
						<Button
							variant="ghost"
							size="sm"
							class="h-7 w-8 rounded p-0 text-xs font-normal text-muted-foreground"
							onclick={() => startEdit(item.id, i + 1)}
						>
							{i + 1}
						</Button>
					{/if}

					<span class="flex-1 font-semibold">{item.name}</span>

					{#if s}
						<span class="text-xs text-muted-foreground">{wlRecord(s)}</span>
						<span class="min-w-[36px] text-right text-xs font-semibold">{winRate(s)}</span>
					{/if}
				</div>
			{/each}
		</div>

		<p class="text-xs text-muted-foreground">
			Click the rank number to edit · Drag ⠿ to reorder · Click Save to persist
		</p>
	</div>
{/if}
