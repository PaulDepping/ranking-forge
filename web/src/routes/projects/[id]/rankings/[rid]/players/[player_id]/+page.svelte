<script lang="ts">
  import type { SetRecord } from "$lib/types";
  import SetDetailModal from "$lib/components/SetDetailModal.svelte";
  import * as Card from "$lib/components/ui/card";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import * as Empty from "$lib/components/ui/empty";
  import { Button } from "$lib/components/ui/button";
  import * as Table from "$lib/components/ui/table";
  import { winRate, toOrdinal, formatDate } from "$lib/utils";
  import { previousPage } from "$lib/stores/navigation";

  let { data } = $props();

  let selectedSet = $state<SetRecord | null>(null);
  let selectedIsWin = $state(false);

  const backHref = $derived(
    $previousPage ??
      `/projects/${data.projectId}/rankings/${data.rankingId}/stats`,
  );

  function openModal(set: SetRecord, isWin: boolean) {
    selectedSet = set;
    selectedIsWin = isWin;
  }

  const wins = $derived(data.stats.wins);
  const losses = $derived(data.stats.losses);
  const winRateStr = $derived(winRate(wins.length, losses.length, "0%"));
  const tournamentCount = $derived(data.tournaments.length);
</script>

<div class="space-y-6">
  <!-- Back button -->
  <Button variant="link" class="px-0" href={backHref}>← Back</Button>

  <!-- Header -->
  <div>
    <h2 class="text-2xl font-bold">{data.stats.name}</h2>
    <p class="text-sm text-muted-foreground">
      {wins.length} W · {losses.length} L · {winRateStr} win rate · {tournamentCount}
      tournaments
    </p>
  </div>

  <!-- Wins / Losses side by side -->
  <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
    <!-- Wins card -->
    <Card.Root>
      <Card.Header class="pb-2">
        <Card.Title class="text-base text-green-600 dark:text-green-400">
          Wins ({wins.length})
        </Card.Title>
      </Card.Header>
      <Card.Content class="pt-0">
        {#if wins.length === 0}
          <p class="text-sm text-muted-foreground">No wins yet.</p>
        {:else}
          <ScrollArea class="h-48 rounded border border-border bg-muted/20">
            {#each wins as set, i (i)}
              <Button
                variant="ghost"
                class="h-auto w-full border-b border-border px-2 py-1 text-xs last:border-0 justify-start whitespace-normal"
                onclick={() => openModal(set, true)}
              >
                {set.opponent_name} · UF {set.upset_factor} · {set.tournament_name}
              </Button>
            {/each}
          </ScrollArea>
        {/if}
      </Card.Content>
    </Card.Root>

    <!-- Losses card -->
    <Card.Root>
      <Card.Header class="pb-2">
        <Card.Title class="text-base text-red-600 dark:text-red-400">
          Losses ({losses.length})
        </Card.Title>
      </Card.Header>
      <Card.Content class="pt-0">
        {#if losses.length === 0}
          <p class="text-sm text-muted-foreground">No losses yet.</p>
        {:else}
          <ScrollArea class="h-48 rounded border border-border bg-muted/20">
            {#each losses as set, i (i)}
              <Button
                variant="ghost"
                class="h-auto w-full border-b border-border px-2 py-1 text-xs last:border-0 justify-start whitespace-normal"
                onclick={() => openModal(set, false)}
              >
                {set.opponent_name} · UF {set.upset_factor} · {set.tournament_name}
              </Button>
            {/each}
          </ScrollArea>
        {/if}
      </Card.Content>
    </Card.Root>
  </div>

  <!-- Tournament history -->
  <div>
    <h3 class="mb-2 text-base font-semibold">
      Tournament history ({data.tournaments.length})
    </h3>
    {#if data.tournaments.length === 0}
      <Empty.Root>
        <Empty.Header>
          <Empty.Title>No tournament history</Empty.Title>
          <Empty.Description
            >No included tournaments found for this player in this ranking.</Empty.Description
          >
        </Empty.Header>
      </Empty.Root>
    {:else}
      <Table.Root>
        <Table.Header>
          <Table.Row>
            <Table.Head>Tournament · Event</Table.Head>
            <Table.Head>Placement</Table.Head>
            <Table.Head>Entrants</Table.Head>
            <Table.Head>Date</Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each data.tournaments as t, i (i)}
            <Table.Row>
              <Table.Cell class="font-medium">
                {t.tournament_name} · {t.event_name}
              </Table.Cell>
              <Table.Cell
                class={t.placement !== null && t.placement <= 3
                  ? "text-green-600 dark:text-green-400"
                  : ""}
              >
                {t.placement !== null ? toOrdinal(t.placement) : "—"}
              </Table.Cell>
              <Table.Cell>{t.num_entrants ?? "—"}</Table.Cell>
              <Table.Cell>{formatDate(t.start_at)}</Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    {/if}
  </div>
</div>

<SetDetailModal
  set={selectedSet}
  isWin={selectedIsWin}
  currentPlayerName={data.stats.name}
  onClose={() => (selectedSet = null)}
  projectId={data.projectId}
  rankingId={data.rankingId}
  opponentPlayerId={selectedSet &&
  selectedSet.opponent_id !== null &&
  data.trackedPlayerIds.has(selectedSet.opponent_id)
    ? selectedSet.opponent_id
    : undefined}
/>
