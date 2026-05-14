<script lang="ts">
	import { enhance } from '$app/forms';
	import { invalidateAll } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Dialog from '$lib/components/ui/dialog';

	let { data, form } = $props();

	let linkDialogOpen = $state(false);
	let linkingPid = $state('');
	let linkingName = $state('');

	function openLinkDialog(pid: string, name: string) {
		linkingPid = pid;
		linkingName = name;
		linkDialogOpen = true;
	}
</script>

<div class="space-y-6">
	<h2 class="text-lg font-semibold">Players</h2>

	{#if form?.addError}
		<p class="text-sm text-destructive">{form.addError}</p>
	{/if}

	<!-- Add player form -->
	<form method="POST" action="?/addPlayer" use:enhance class="flex gap-2">
		<div class="flex-1">
			<Label for="new-player" class="sr-only">Player name</Label>
			<Input id="new-player" name="name" placeholder="Player name" required />
		</div>
		<Button type="submit">Add player</Button>
	</form>

	{#if data.players.length === 0}
		<p class="text-sm text-muted-foreground">No players yet. Add one above.</p>
	{:else}
		<div class="space-y-2">
			{#each data.players as player (player.id)}
				<div class="flex items-start justify-between rounded-md border border-border p-3">
					<div class="space-y-1">
						<p class="font-medium">{player.name}</p>
						<div class="flex flex-wrap gap-1">
							{#each player.accounts as account (account.id)}
								<form method="POST" action="?/unlinkAccount" use:enhance={() => {
									return async ({ result, update }) => {
										if (result.type === 'success') {
											await invalidateAll();
										} else {
											await update();
										}
									};
								}} class="inline-flex">
									<input type="hidden" name="pid" value={player.id} />
									<input type="hidden" name="aid" value={account.id} />
									<Badge variant="secondary" class="gap-1 pr-1">
										{account.display_name ?? account.slug}
										<button type="submit" class="ml-0.5 rounded-full hover:bg-muted" title="Remove">×</button>
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
					<form method="POST" action="?/deletePlayer" use:enhance>
						<input type="hidden" name="pid" value={player.id} />
						<Button
							type="submit"
							variant="ghost"
							size="sm"
							class="text-destructive hover:text-destructive"
							onclick={(e: MouseEvent) => { if (!confirm(`Remove ${player.name}?`)) e.preventDefault(); }}
						>Remove</Button>
					</form>
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
				<Label for="slug">start.gg slug</Label>
				<Input id="slug" name="slug" placeholder="user/abc123" required />
				<p class="text-xs text-muted-foreground">Find the slug in the start.gg profile URL</p>
			</div>
			<div class="flex justify-end gap-2">
				<Button type="button" variant="ghost" onclick={() => (linkDialogOpen = false)}>Cancel</Button>
				<Button type="submit">Link</Button>
			</div>
		</form>
	</Dialog.Content>
</Dialog.Root>
