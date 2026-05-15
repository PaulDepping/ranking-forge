<script lang="ts">
	import { untrack } from 'svelte';
	import { enhance } from '$app/forms';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Badge } from '$lib/components/ui/badge';
	import { Alert } from '$lib/components/ui/alert';
	import { PUBLIC_API_URL } from '$env/static/public';
	import type { Job } from '$lib/types';

	let { data, form } = $props();

	// Local state so we can update after polling; synced when server data changes
	let job = $state<Job | null>(untrack(() => data.job ?? null));
	$effect(() => { job = data.job ?? null; });
	let polling = $state(false);

	const statusColors: Record<string, 'default' | 'secondary' | 'destructive' | 'outline'> = {
		pending: 'secondary',
		running: 'default',
		done: 'outline',
		failed: 'destructive'
	};

	function startPolling() {
		if (polling) return;
		polling = true;
		const interval = setInterval(async () => {
			const res = await fetch(`${PUBLIC_API_URL}/projects/${data.project.id}/import`, { credentials: 'include' });
			if (res.ok) {
				const updated: Job = await res.json();
				job = updated;
				if (updated.status === 'done' || updated.status === 'failed') {
					clearInterval(interval);
					polling = false;
				}
			}
		}, 3000);
	}

	$effect(() => {
		if (job?.status === 'pending' || job?.status === 'running') {
			startPolling();
		}
	});
</script>

<div class="space-y-6 max-w-lg">
	<h2 class="text-lg font-semibold">Import tournaments</h2>
	<p class="text-sm text-muted-foreground">
		Fetches all start.gg tournaments for your players and imports them. This may take a minute.
	</p>

	{#if form?.error}
		<Alert variant="destructive">{form.error}</Alert>
	{/if}

	{#if job}
		<div class="rounded-md border border-border p-4 space-y-2">
			<div class="flex items-center gap-2">
				<span class="text-sm font-medium">Status:</span>
				<Badge variant={statusColors[job.status]}>{job.status}</Badge>
				{#if polling}
					<span class="text-xs text-muted-foreground animate-pulse">updating…</span>
				{/if}
			</div>
			{#if job.error}
				<p class="text-sm text-destructive">{job.error}</p>
			{/if}
			<p class="text-xs text-muted-foreground">
				Started {new Date(job.created_at).toLocaleString()}
			</p>
			{#if job.status === 'failed'}
				<form
					method="POST"
					use:enhance={() => {
						return ({ result }) => {
							if (result.type === 'success' && result.data?.job) {
								job = result.data.job as Job;
								startPolling();
							}
						};
					}}
				>
					<input type="hidden" name="after_date" value={job.after_date ?? ''} />
					<input type="hidden" name="before_date" value={job.before_date ?? ''} />
					<Button type="submit" variant="outline" size="sm">Retry</Button>
				</form>
			{/if}
		</div>
	{/if}

	<form
		method="POST"
		class="space-y-4"
		use:enhance={({ cancel }) => {
			if (job?.status === 'pending' || job?.status === 'running') {
				if (!confirm('An import is already running. Start a new one?')) cancel();
			}
			return ({ result }) => {
				if (result.type === 'success' && result.data?.job) {
					job = result.data.job as Job;
					startPolling();
				}
			};
		}}
	>
		<div class="grid grid-cols-2 gap-4">
			<div class="space-y-1">
				<label for="after_date" class="text-sm font-medium">From date</label>
				<Input id="after_date" name="after_date" type="date" />
			</div>
			<div class="space-y-1">
				<label for="before_date" class="text-sm font-medium">To date</label>
				<Input id="before_date" name="before_date" type="date" />
			</div>
		</div>
		<p class="text-xs text-muted-foreground">Leave blank to import all tournaments.</p>
		<Button type="submit">
			{job ? 'Re-import' : 'Start import'}
		</Button>
	</form>
</div>
