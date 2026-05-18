<script lang="ts">
	import { enhance } from '$app/forms';
	import { invalidateAll } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import AccountBadge from '$lib/components/AccountBadge.svelte';
	import type { Player } from '$lib/types';

	let { player, isEditing, form, onEdit, onCancelEdit, onOpenLinkDialog }: {
		player: Player;
		isEditing: boolean;
		form: { renameError?: string; renamePid?: string } | null;
		onEdit: () => void;
		onCancelEdit: () => void;
		onOpenLinkDialog: () => void;
	} = $props();

	let editingName = $state('');
	$effect(() => {
		if (isEditing) editingName = player.name;
	});
</script>

<div class="rounded-md border border-border p-3">
	{#if isEditing}
		<form
			method="POST"
			action="?/renamePlayer"
			use:enhance={() => {
				return async ({ result, update }) => {
					if (result.type === 'success') {
						onCancelEdit();
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
				<Button type="button" variant="ghost" size="sm" onclick={onCancelEdit}>Cancel</Button>
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
						<AccountBadge
							playerId={player.id}
							accountId={account.id}
							displayName={account.display_name}
							handle={account.handle}
						/>
					{/each}
					<Button
						type="button"
						variant="link"
						size="sm"
						class="h-auto p-0 text-xs"
						onclick={onOpenLinkDialog}
					>+ add account</Button>
				</div>
			</div>
			<div class="flex gap-1">
				<Button type="button" variant="ghost" size="sm" onclick={onEdit}>Edit</Button>
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
