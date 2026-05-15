<script lang="ts">
	import { untrack } from 'svelte';
	import { enhance } from '$app/forms';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Alert } from '$lib/components/ui/alert';
	import { Label } from '$lib/components/ui/label';
	import * as Card from '$lib/components/ui/card';
	import * as Popover from '$lib/components/ui/popover';
	import Calendar from '$lib/components/ui/calendar/calendar.svelte';
	import { type CalendarDate, getLocalTimeZone } from '@internationalized/date';
	import { PUBLIC_API_URL } from '$env/static/public';
	import type { Job } from '$lib/types';
	import { formatDate, formatDateTime } from '$lib/utils';

	let { data, form } = $props();

	let afterDate     = $state<CalendarDate | undefined>(undefined);
	let beforeDate    = $state<CalendarDate | undefined>(undefined);
	let afterDateOpen = $state(false);
	let beforeDateOpen = $state(false);

	const afterDateStr  = $derived(afterDate?.toString() ?? '');
	const beforeDateStr = $derived(beforeDate?.toString() ?? '');

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
		<Card.Root class="py-0">
			<Card.Content class="p-4 space-y-2">
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
					Started {formatDateTime(job.created_at)}
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
			</Card.Content>
		</Card.Root>
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
		<input type="hidden" name="after_date" value={afterDateStr} />
		<input type="hidden" name="before_date" value={beforeDateStr} />
		<div class="grid grid-cols-2 gap-4">
			<div class="space-y-1">
				<Label>From date</Label>
				<Popover.Root bind:open={afterDateOpen}>
					<Popover.Trigger>
						{#snippet child({ props })}
							<Button {...props} variant="outline" class="w-full justify-start font-normal">
								{afterDate
									? formatDate(afterDate.toDate(getLocalTimeZone()))
									: 'Pick date'}
							</Button>
						{/snippet}
					</Popover.Trigger>
					<Popover.Content class="w-auto overflow-hidden p-0" align="start">
						<Calendar
							type="single"
							bind:value={afterDate}
							captionLayout="dropdown"
							onValueChange={() => { afterDateOpen = false; }}
						/>
					</Popover.Content>
				</Popover.Root>
			</div>
			<div class="space-y-1">
				<Label>To date</Label>
				<Popover.Root bind:open={beforeDateOpen}>
					<Popover.Trigger>
						{#snippet child({ props })}
							<Button {...props} variant="outline" class="w-full justify-start font-normal">
								{beforeDate
									? formatDate(beforeDate.toDate(getLocalTimeZone()))
									: 'Pick date'}
							</Button>
						{/snippet}
					</Popover.Trigger>
					<Popover.Content class="w-auto overflow-hidden p-0" align="start">
						<Calendar
							type="single"
							bind:value={beforeDate}
							captionLayout="dropdown"
							onValueChange={() => { beforeDateOpen = false; }}
						/>
					</Popover.Content>
				</Popover.Root>
			</div>
		</div>
		<p class="text-xs text-muted-foreground">Leave blank to import all tournaments.</p>
		<Button type="submit">
			{job ? 'Re-import' : 'Start import'}
		</Button>
	</form>
</div>
