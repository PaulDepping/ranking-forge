<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { makeApi } from '$lib/api';
	import { invalidateAll } from '$app/navigation';
	import type { ByHandlesResult } from '$lib/types';

	let { projectId, onClose }: { projectId: string; onClose: () => void } = $props();

	let input = $state('');
	let submitting = $state(false);
	let results = $state<ByHandlesResult[]>([]);

	async function submit() {
		const handles = input.split('\n').map((h) => h.trim()).filter(Boolean);
		if (!handles.length) return;
		submitting = true;
		const api = makeApi(fetch, PUBLIC_API_URL);
		const res = await api.post(`/projects/${projectId}/players/by-handles`, { handles });
		submitting = false;
		if (res.ok) {
			results = await res.json();
		}
	}

	function done() {
		const anyCreated = results.some((r) => r.status === 'created');
		results = [];
		input = '';
		if (anyCreated) invalidateAll();
		onClose();
	}
</script>

<div class="space-y-3">
	{#if results.length === 0}
		<div class="space-y-2">
			<Label for="handles-input">One handle per line — bare handle, full slug, or full URL</Label>
			<Textarea
				id="handles-input"
				bind:value={input}
				placeholder={'mang0\nhttps://start.gg/user/armada'}
				rows={5}
				disabled={submitting}
				class="font-mono text-sm"
			/>
		</div>
		<Button onclick={submit} disabled={submitting || !input.trim()} class="w-full">
			{submitting ? 'Adding…' : 'Add players'}
		</Button>
	{:else}
		<div class="divide-y rounded-md border">
			{#each results as result (result.handle)}
				<div class="flex items-center gap-3 px-3 py-2 text-sm">
					{#if result.status === 'created'}
						<span class="text-green-600">✓</span>
						<span class="font-medium">{result.name}</span>
						<Badge variant="outline" class="ml-auto border-green-200 bg-green-50 text-xs text-green-700">
							created
						</Badge>
					{:else if result.status === 'skipped'}
						<span class="text-muted-foreground">–</span>
						<span class="font-medium">{result.name}</span>
						<Badge variant="secondary" class="ml-auto text-xs">already added</Badge>
					{:else}
						<span class="text-destructive">✕</span>
						<span class="text-muted-foreground">{result.handle}</span>
						<Badge variant="destructive" class="ml-auto text-xs">not found</Badge>
					{/if}
				</div>
			{/each}
		</div>
		<Button onclick={done} class="w-full">Done</Button>
	{/if}
</div>
