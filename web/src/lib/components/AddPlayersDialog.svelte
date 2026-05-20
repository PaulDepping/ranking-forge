<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Tabs from "$lib/components/ui/tabs";
  import { Button } from "$lib/components/ui/button";
  import TournamentTab from "./TournamentTab.svelte";
  import HandleTab from "./HandleTab.svelte";
  import NameTab from "./NameTab.svelte";
  import type { Player } from "$lib/types";

  let { projectId, players }: { projectId: string; players: Player[] } =
    $props();

  let open = $state(false);

  function close() {
    open = false;
  }
</script>

<Button onclick={() => (open = true)}>Add players</Button>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-md">
    <Dialog.Header>
      <Dialog.Title>Add players</Dialog.Title>
    </Dialog.Header>
    <Tabs.Root value="tournament">
      <Tabs.List class="w-full">
        <Tabs.Trigger value="tournament" class="flex-1"
          >From tournament</Tabs.Trigger
        >
        <Tabs.Trigger value="handle" class="flex-1">By handle</Tabs.Trigger>
        <Tabs.Trigger value="name" class="flex-1">By name</Tabs.Trigger>
      </Tabs.List>
      <Tabs.Content value="tournament" class="mt-4">
        <TournamentTab {projectId} {players} onClose={close} />
      </Tabs.Content>
      <Tabs.Content value="handle" class="mt-4">
        <HandleTab {projectId} onClose={close} />
      </Tabs.Content>
      <Tabs.Content value="name" class="mt-4">
        <NameTab {projectId} />
      </Tabs.Content>
    </Tabs.Root>
  </Dialog.Content>
</Dialog.Root>
