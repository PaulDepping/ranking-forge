<script lang="ts">
  import { untrack } from "svelte";
  import { enhance } from "$app/forms";
  import { invalidateAll } from "$app/navigation";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Separator } from "$lib/components/ui/separator";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";
  import * as Card from "$lib/components/ui/card";
  import * as Table from "$lib/components/ui/table";
  import * as Select from "$lib/components/ui/select";
  let { data, form } = $props();

  let name = $state(untrack(() => data.project.name));
  $effect(() => {
    name = data.project.name;
  });

  let deleteDialogOpen = $state(false);
  let deleteFormEl = $state<HTMLFormElement | null>(null);

  let addMemberRole = $state<"editor" | "viewer">("editor");
  let createLinkRole = $state<"editor" | "viewer">("editor");

  const roleLabel = (r: "editor" | "viewer") =>
    r === "editor" ? "Editor" : "Viewer";
</script>

<div class="max-w-lg space-y-8">
  <!-- Project name -->
  <div class="space-y-3">
    <h2 class="text-lg font-semibold">Project name</h2>
    <form
      method="POST"
      action="?/rename"
      class="flex gap-2"
      use:enhance={() => {
        return async ({ result, update }) => {
          if (result.type === "success" && result.data?.project) {
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

  <!-- Members -->
  <div class="space-y-4">
    <h2 class="text-lg font-semibold">Members</h2>

    <Table.Root>
      <Table.Header>
        <Table.Row>
          <Table.Head>Member</Table.Head>
          <Table.Head>Role</Table.Head>
          <Table.Head></Table.Head>
        </Table.Row>
      </Table.Header>
      <Table.Body>
        {#each data.members as member}
          <Table.Row>
            <Table.Cell>
              <div class="font-medium">{member.display_name}</div>
              <div class="text-xs text-muted-foreground">{member.email}</div>
            </Table.Cell>
            <Table.Cell class="capitalize">{member.role}</Table.Cell>
            <Table.Cell class="text-right">
              <form
                method="POST"
                action="?/removeMember"
                use:enhance
                class="inline"
              >
                <input type="hidden" name="user_id" value={member.user_id} />
                <Button type="submit" variant="ghost" size="sm">Remove</Button>
              </form>
            </Table.Cell>
          </Table.Row>
        {/each}
      </Table.Body>
    </Table.Root>

    <form
      method="POST"
      action="?/addMember"
      use:enhance
      class="flex gap-2 items-end"
    >
      <div class="flex-1 space-y-1">
        <Label for="member-email">Add by email</Label>
        <Input
          id="member-email"
          name="email"
          type="email"
          placeholder="player@example.com"
        />
      </div>
      <Select.Root
        type="single"
        value={addMemberRole}
        onValueChange={(v) => (addMemberRole = v as "editor" | "viewer")}
      >
        <Select.Trigger class="w-32">{roleLabel(addMemberRole)}</Select.Trigger>
        <Select.Content>
          <Select.Item value="editor">Editor</Select.Item>
          <Select.Item value="viewer">Viewer</Select.Item>
        </Select.Content>
      </Select.Root>
      <input type="hidden" name="role" value={addMemberRole} />
      <Button type="submit">Add</Button>
    </form>
    {#if form && "memberError" in form && form.memberError}
      <p class="text-sm text-destructive">{form.memberError}</p>
    {/if}
  </div>

  <Separator />

  <!-- Invite links -->
  <div class="space-y-4">
    <h2 class="text-lg font-semibold">Invite links</h2>

    {#each data.inviteLinks as link}
      <Card.Root class="py-0 gap-0">
        <Card.Header class="p-3">
          <Card.Title class="text-sm font-medium capitalize"
            >{link.role}</Card.Title
          >
          {#if link.expires_at}
            <Card.Description class="text-xs">
              expires {new Date(link.expires_at).toLocaleDateString()}
            </Card.Description>
          {/if}
          <Card.Action class="flex gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onclick={() =>
                navigator.clipboard.writeText(
                  `${location.origin}/invite/${link.id}`,
                )}
            >
              Copy link
            </Button>
            <form
              method="POST"
              action="?/revokeInviteLink"
              use:enhance
              class="inline"
            >
              <input type="hidden" name="link_id" value={link.id} />
              <Button type="submit" variant="ghost" size="sm">Revoke</Button>
            </form>
          </Card.Action>
        </Card.Header>
      </Card.Root>
    {/each}

    <form
      method="POST"
      action="?/createInviteLink"
      use:enhance
      class="flex gap-2 items-end"
    >
      <Select.Root
        type="single"
        value={createLinkRole}
        onValueChange={(v) => (createLinkRole = v as "editor" | "viewer")}
      >
        <Select.Trigger class="w-32">{roleLabel(createLinkRole)}</Select.Trigger
        >
        <Select.Content>
          <Select.Item value="editor">Editor</Select.Item>
          <Select.Item value="viewer">Viewer</Select.Item>
        </Select.Content>
      </Select.Root>
      <input type="hidden" name="role" value={createLinkRole} />
      <Button type="submit">Create invite link</Button>
    </form>
    {#if form && "linkError" in form && form.linkError}
      <p class="text-sm text-destructive">{form.linkError}</p>
    {/if}
  </div>

  <Separator />

  <!-- Danger zone -->
  <div class="space-y-3">
    <h2 class="text-lg font-semibold text-destructive">Danger zone</h2>
    <div
      class="flex items-center justify-between rounded-md border border-destructive/40 p-4"
    >
      <div>
        <p class="font-medium">Delete this project</p>
        <p class="text-sm text-muted-foreground">
          Permanently removes all players, tournaments, and stats.
        </p>
      </div>
      <form
        method="POST"
        action="?/delete"
        use:enhance
        bind:this={deleteFormEl}
      >
        <Button
          type="button"
          variant="destructive"
          onclick={() => {
            deleteDialogOpen = true;
          }}
        >
          Delete project
        </Button>
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
        Permanently removes all players, tournaments, and stats. This cannot be
        undone.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
        onclick={() => {
          deleteDialogOpen = false;
          deleteFormEl?.requestSubmit();
        }}>Delete project</AlertDialog.Action
      >
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
