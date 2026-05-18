<script lang="ts">
	import { untrack } from 'svelte';
	import { enhance } from '$app/forms';
	import { invalidateAll } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Separator } from '$lib/components/ui/separator';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';

	let { data, form } = $props();

	let name = $state(untrack(() => data.project.name));
	$effect(() => { name = data.project.name; });

	let deleteDialogOpen = $state(false);
	let deleteFormEl = $state<HTMLFormElement | null>(null);
</script>

<div class="max-w-lg space-y-8">
	<div class="space-y-3">
		<h2 class="text-lg font-semibold">Project name</h2>
		<form
			method="POST"
			action="?/rename"
			class="flex gap-2"
			use:enhance={() => {
				return async ({ result, update }) => {
					if (result.type === 'success' && result.data?.project) {
						name = (result.data.project as { name: string }).name;
						await invalidateAll();
					} else {
						await update();
					}
				};
			}}
		>
			<Label for="project-name" class="sr-only">Project name</Label>
			<Input id="project-name" name="name" bind:value={name} class="flex-1" />
			<Button type="submit">Save</Button>
		</form>
		{#if form?.renameError}
			<p class="text-sm text-destructive">{form.renameError}</p>
		{/if}
	</div>

	<Separator />

	<div class="space-y-3">
		<h2 class="text-lg font-semibold text-destructive">Danger zone</h2>
		<div class="flex items-center justify-between rounded-md border border-destructive/40 p-4">
			<div>
				<p class="font-medium">Delete this project</p>
				<p class="text-sm text-muted-foreground">
					Permanently removes all players, tournaments, and stats.
				</p>
			</div>
			<form method="POST" action="?/delete" use:enhance bind:this={deleteFormEl}>
				<Button
					type="button"
					variant="destructive"
					onclick={() => { deleteDialogOpen = true; }}
				>Delete project</Button>
			</form>
		</div>
		{#if form?.deleteError}
			<p class="mt-2 text-sm text-destructive">{form.deleteError}</p>
		{/if}
	</div>
</div>

<AlertDialog.Root bind:open={deleteDialogOpen}>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>Delete this project?</AlertDialog.Title>
			<AlertDialog.Description>
				Permanently removes all players, tournaments, and stats. This cannot be undone.
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
			<AlertDialog.Action
				class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
				onclick={() => { deleteDialogOpen = false; deleteFormEl?.requestSubmit(); }}
			>Delete project</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
