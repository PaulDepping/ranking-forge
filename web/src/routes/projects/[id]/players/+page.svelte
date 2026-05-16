<script lang="ts">
	import { enhance } from '$app/forms';
	import { invalidateAll } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Empty from '$lib/components/ui/empty';
	import AddPlayersDialog from '$lib/components/AddPlayersDialog.svelte';

	let { data, form } = $props();

	let linkDialogOpen = $state(false);
	let linkingPid = $state('');
	let linkingName = $state('');
	let editingPid = $state('');
	let editingName = $state('');

	function openLinkDialog(pid: string, name: string) {
		linkingPid = pid;
		linkingName = name;
		linkDialogOpen = true;
	}

	function startEdit(pid: string, name: string) {
		editingPid = pid;
		editingName = name;
	}

	function cancelEdit() {
		editingPid = '';
		editingName = '';
	}
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<h2 class="text-lg font-semibold">Players</h2>
		<AddPlayersDialog projectId={data.project.id} players={data.players} />
	</div>

	{#if data.players.length === 0}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No players yet</Empty.Title>
				<Empty.Description>Use "Add players" to get started.</Empty.Description>
			</Empty.Header>
		</Empty.Root>
	{:else}
		<div class="space-y-2">
			{#each data.players as player (player.id)}
				<div class="rounded-md border border-border p-3">
					{#if editingPid === player.id}
						<form
							method="POST"
							action="?/renamePlayer"
							use:enhance={() => {
								return async ({ result, update }) => {
									if (result.type === 'success') {
										cancelEdit();
										await invalidateAll();
									} else {
										await update();
									}
								};
							}}
						>
							<input type="hidden" name="pid" value={player.id} />
							<div class="flex items-center gap-2">
								<Input name="name" bind:value={editingName} class="flex-1" />
								<Button type="submit" size="sm">Save</Button>
								<Button type="button" variant="ghost" size="sm" onclick={cancelEdit}>Cancel</Button>
							</div>
							{#if form?.renameError && form.renamePid === player.id}
								<p class="mt-1 text-sm text-destructive">{form.renameError}</p>
							{/if}
						</form>
					{:else}
						<div class="flex items-start justify-between">
							<div class="space-y-1">
								<p class="font-medium">{player.name}</p>
								<div class="flex flex-wrap gap-1">
									{#each player.accounts as account (account.id)}
										<form
											method="POST"
											action="?/unlinkAccount"
											use:enhance={() => {
												return async ({ result, update }) => {
													if (result.type === 'success') {
														await invalidateAll();
													} else {
														await update();
													}
												};
											}}
											class="inline-flex"
										>
											<input type="hidden" name="pid" value={player.id} />
											<input type="hidden" name="aid" value={account.id} />
											<Badge variant="secondary" class="gap-1 pr-1">
												{account.display_name ?? account.handle}
												<button
													type="submit"
													class="ml-0.5 rounded-full hover:bg-muted"
													title="Remove">×</button
												>
											</Badge>
										</form>
									{/each}
									<button
										type="button"
										onclick={() => openLinkDialog(player.id, player.name)}
										class="text-xs text-muted-foreground underline hover:text-foreground"
									>+ add account</button>
								</div>
							</div>
							<div class="flex gap-1">
								<Button
									type="button"
									variant="ghost"
									size="sm"
									onclick={() => startEdit(player.id, player.name)}
								>Edit</Button>
								<form method="POST" action="?/deletePlayer" use:enhance>
									<input type="hidden" name="pid" value={player.id} />
									<Button
										type="submit"
										variant="ghost"
										size="sm"
										class="text-destructive hover:text-destructive"
										onclick={(e: MouseEvent) => {
											if (!confirm(`Remove ${player.name}?`)) e.preventDefault();
										}}
									>Remove</Button>
								</form>
							</div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Link account dialog -->
<Dialog.Root bind:open={linkDialogOpen}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Link start.gg account</Dialog.Title>
			<Dialog.Description>Add a start.gg account for {linkingName}</Dialog.Description>
		</Dialog.Header>
		{#if form?.linkError && form.linkPid === linkingPid}
			<p class="text-sm text-destructive">{form.linkError}</p>
		{/if}
		<form
			method="POST"
			action="?/linkAccount"
			use:enhance={() => {
				return async ({ result, update }) => {
					if (result.type === 'success') {
						linkDialogOpen = false;
						await invalidateAll();
					} else {
						await update();
					}
				};
			}}
			class="space-y-4"
		>
			<input type="hidden" name="pid" value={linkingPid} />
			<div class="space-y-2">
				<Label for="handle">start.gg handle</Label>
				<Input id="handle" name="handle" placeholder="mang0" required />
				<p class="text-xs text-muted-foreground">Accepts bare handle, full slug, or full URL</p>
			</div>
			<div class="flex justify-end gap-2">
				<Button type="button" variant="ghost" onclick={() => (linkDialogOpen = false)}>Cancel</Button>
				<Button type="submit">Link</Button>
			</div>
		</form>
	</Dialog.Content>
</Dialog.Root>
