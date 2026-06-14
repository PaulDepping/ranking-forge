<script lang="ts">
  import { enhance } from "$app/forms";
  import { invalidateAll } from "$app/navigation";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Empty from "$lib/components/ui/empty";
  import AddPlayersDialog from "$lib/components/AddPlayersDialog.svelte";
  import PlayerCard from "$lib/components/PlayerCard.svelte";

  let { data, form } = $props();

  let linkDialogOpen = $state(false);
  let linkingPid = $state("");
  let linkingName = $state("");
  let editingPid = $state("");

  function openLinkDialog(pid: string, name: string) {
    linkingPid = pid;
    linkingName = name;
    linkDialogOpen = true;
  }

  function startEdit(pid: string) {
    editingPid = pid;
  }

  function cancelEdit() {
    editingPid = "";
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
        <PlayerCard
          {player}
          isEditing={editingPid === player.id}
          {form}
          onEdit={() => startEdit(player.id)}
          onCancelEdit={cancelEdit}
          onOpenLinkDialog={() => openLinkDialog(player.id, player.name)}
        />
      {/each}
    </div>
  {/if}
</div>

<!-- Link account dialog -->
<Dialog.Root bind:open={linkDialogOpen}>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>Link start.gg account</Dialog.Title>
      <Dialog.Description
        >Add a start.gg account for {linkingName}</Dialog.Description
      >
    </Dialog.Header>
    {#if form?.linkError && form.linkPid === linkingPid}
      <p class="text-sm text-destructive">{form.linkError}</p>
    {/if}
    <form
      method="POST"
      action="?/linkAccount"
      use:enhance={() => {
        return async ({ result, update }) => {
          if (result.type === "success") {
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
        <p class="text-xs text-muted-foreground">
          Accepts bare handle, full slug, or full URL
        </p>
      </div>
      <div class="flex justify-end gap-2">
        <Button
          type="button"
          variant="ghost"
          onclick={() => (linkDialogOpen = false)}>Cancel</Button
        >
        <Button type="submit">Link</Button>
      </div>
    </form>
  </Dialog.Content>
</Dialog.Root>
