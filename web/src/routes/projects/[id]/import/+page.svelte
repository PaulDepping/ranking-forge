<script lang="ts">
	import { enhance } from '$app/forms';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Alert } from '$lib/components/ui/alert';
	import { PUBLIC_API_URL } from '$env/static/public';
	import type { Job } from '$lib/types';

	let { data, form } = $props();

	// Local state so we can update after polling
	let job = $state<Job | null>(data.job ?? null);
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
		</div>
	{/if}

	<form
		method="POST"
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
		<Button type="submit">
			{job ? 'Re-import' : 'Start import'}
		</Button>
	</form>
</div>
