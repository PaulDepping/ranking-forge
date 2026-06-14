<script lang="ts">
  import { untrack } from "svelte";
  import { dragHandleZone, dragHandle } from "svelte-dnd-action";
  import type { DndEvent } from "svelte-dnd-action";
  import { makeApi } from "$lib/api";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as Empty from "$lib/components/ui/empty";
  import type { RankingPlayerWithScore, PlayerStats } from "$lib/types";
  import { winRate } from "$lib/utils";

  let { data } = $props();

  type RankItem = {
    id: string;
    name: string;
    computed_rating: number | null;
    display_data: Record<string, unknown> | null;
  };

  const sortedPlayers = $derived(
    [...(data.players as RankingPlayerWithScore[])].sort(
      (a, b) => a.rank_position - b.rank_position,
    ),
  );

  const computedOrder = $derived(
    data.ranking.algorithm
      ? [...(data.players as RankingPlayerWithScore[])]
          .filter((p) => p.computed_rating !== null)
          .sort((a, b) => (b.computed_rating ?? 0) - (a.computed_rating ?? 0))
          .map((p) => p.player_id)
      : [],
  );

  let items = $state<RankItem[]>(
    untrack(() =>
      sortedPlayers.map((p) => ({
        id: p.player_id,
        name: p.name,
        computed_rating: p.computed_rating ?? null,
        display_data: (p.display_data as Record<string, unknown>) ?? null,
      })),
    ),
  );
  let savedIds = $state<string[]>(
    untrack(() => sortedPlayers.map((p) => p.player_id)),
  );

  const statsMap = $derived<Record<string, PlayerStats>>(
    Object.fromEntries(
      (data.stats as PlayerStats[]).map((s) => [s.player_id, s]),
    ),
  );

  const hasChanges = $derived(
    items.length !== savedIds.length ||
      items.some((item, i) => item.id !== savedIds[i]),
  );

  const canEdit = $derived(
    data.project.user_role === "editor" || data.project.user_role === "owner",
  );

  const isAlgorithmic = $derived(!!data.ranking.algorithm);

  let saveStatus = $state<"idle" | "saving" | "saved">("idle");

  function handleConsider(e: CustomEvent<DndEvent<RankItem>>) {
    items = e.detail.items;
  }

  function handleFinalize(e: CustomEvent<DndEvent<RankItem>>) {
    items = e.detail.items;
  }

  let editingId = $state<string | null>(null);
  let editingValue = $state("");
  let editInput = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (editingId && editInput) {
      editInput.focus();
      editInput.select();
    }
  });

  function startEdit(id: string, rank: number) {
    editingId = id;
    editingValue = String(rank);
  }

  function commitEdit() {
    if (!editingId) return;
    const n = parseInt(editingValue, 10);
    if (!isNaN(n)) {
      const clamped = Math.max(1, Math.min(n, items.length));
      const idx = items.findIndex((i) => i.id === editingId);
      if (idx !== -1) {
        const copy = [...items];
        const [item] = copy.splice(idx, 1);
        copy.splice(clamped - 1, 0, item);
        items = copy;
      }
    }
    editingId = null;
  }

  function onRankKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") commitEdit();
    if (e.key === "Escape") editingId = null;
  }

  async function save() {
    saveStatus = "saving";
    const api = makeApi(fetch);
    const res = await api.putRanking(
      data.project.id,
      data.ranking.id,
      items.map((i) => i.id),
    );
    if (res.ok) {
      savedIds = items.map((i) => i.id);
      saveStatus = "saved";
      setTimeout(() => {
        saveStatus = "idle";
      }, 2000);
    } else {
      saveStatus = "idle";
    }
  }

  async function syncToAlgorithm() {
    const ratingMap = new Map(
      items.map((item) => [item.id, item.computed_rating ?? -Infinity]),
    );
    items = [...items].sort(
      (a, b) => (ratingMap.get(b.id) ?? 0) - (ratingMap.get(a.id) ?? 0),
    );
    await save();
  }

  function computedDelta(
    playerId: string,
    currentIndex: number,
  ): number | null {
    if (!isAlgorithmic || computedOrder.length === 0) return null;
    const computedIndex = computedOrder.indexOf(playerId);
    if (computedIndex === -1) return null;
    return computedIndex - currentIndex;
  }

  function wlRecord(s: PlayerStats | undefined): string {
    if (!s) return "";
    return `${s.wins.length}W · ${s.losses.length}L`;
  }

  function isMoved(id: string, currentIndex: number): boolean {
    return hasChanges && savedIds[currentIndex] !== id;
  }

  function formatRating(item: RankItem): string {
    if (item.computed_rating === null) return "";
    if (item.display_data?.rd !== undefined) {
      return `${Math.round(item.computed_rating)} ± ${Math.round(item.display_data.rd as number)}`;
    }
    return String(Math.round(item.computed_rating));
  }
</script>

{#if data.players.length === 0}
  <Empty.Root>
    <Empty.Header>
      <Empty.Title>No players</Empty.Title>
      <Empty.Description
        >Add players to start building your ranking.</Empty.Description
      >
    </Empty.Header>
  </Empty.Root>
{:else}
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold">Ranking</h2>
      {#if canEdit}
        <div class="flex items-center gap-3">
          {#if isAlgorithmic}
            <Button
              variant="outline"
              size="sm"
              onclick={syncToAlgorithm}
              disabled={saveStatus === "saving"}
            >
              Sync to algorithm
            </Button>
          {/if}
          {#if hasChanges && saveStatus !== "saved"}
            <span class="text-sm text-muted-foreground">Unsaved changes</span>
          {/if}
          <Button
            onclick={save}
            disabled={!hasChanges || saveStatus === "saving"}
            size="sm"
            variant={saveStatus === "saved" ? "outline" : "default"}
          >
            {saveStatus === "saving"
              ? "Saving…"
              : saveStatus === "saved"
                ? "Saved ✓"
                : "Save"}
          </Button>
        </div>
      {/if}
    </div>

    {#if canEdit}
      <div
        class="flex max-w-xl flex-col gap-1"
        use:dragHandleZone={{ items, flipDurationMs: 0 }}
        onconsider={handleConsider}
        onfinalize={handleFinalize}
      >
        {#each items as item, i (item.id)}
          {@const s = statsMap[item.id]}
          {@const moved = isMoved(item.id, i)}
          {@const delta = computedDelta(item.id, i)}
          <div
            class="flex items-center gap-3 rounded-md border px-3 py-2.5 text-sm transition-colors
              {moved ? 'border-primary/40 bg-primary/5' : 'bg-card'}"
          >
            <span
              use:dragHandle
              class="cursor-grab select-none text-base text-muted-foreground active:cursor-grabbing"
            >
              ⠿
            </span>

            {#if editingId === item.id}
              <Input
                bind:ref={editInput}
                type="number"
                class="h-7 w-12 px-1 text-center text-xs [appearance:textfield]"
                bind:value={editingValue}
                onblur={commitEdit}
                onkeydown={onRankKeydown}
              />
            {:else}
              <Button
                variant="ghost"
                size="sm"
                class="h-7 w-8 rounded p-0 text-xs font-normal text-muted-foreground"
                onclick={() => startEdit(item.id, i + 1)}
              >
                {i + 1}
              </Button>
            {/if}

            <a
              href="/projects/{data.project.id}/players/{item.id}"
              class="flex-1 font-semibold hover:underline">{item.name}</a
            >

            {#if isAlgorithmic && item.computed_rating !== null}
              <span class="text-xs font-semibold text-primary tabular-nums">
                {formatRating(item)}
              </span>
              {#if delta !== null && delta !== 0}
                <span
                  class="min-w-[28px] text-right text-xs tabular-nums
                    {delta > 0 ? 'text-green-600' : 'text-red-500'}"
                >
                  {delta > 0 ? `↑${delta}` : `↓${Math.abs(delta)}`}
                </span>
              {/if}
            {:else if s}
              <span class="text-xs text-muted-foreground">{wlRecord(s)}</span>
              <span class="min-w-[36px] text-right text-xs font-semibold"
                >{winRate(s.wins.length, s.losses.length)}</span
              >
            {/if}
          </div>
        {/each}
      </div>
    {:else}
      <div class="flex max-w-xl flex-col gap-1">
        {#each items as item, i (item.id)}
          {@const s = statsMap[item.id]}
          {@const delta = computedDelta(item.id, i)}
          <div
            class="flex items-center gap-3 rounded-md border bg-card px-3 py-2.5 text-sm"
          >
            <span class="w-8 text-center text-xs text-muted-foreground"
              >{i + 1}</span
            >
            <a
              href="/projects/{data.project.id}/players/{item.id}"
              class="flex-1 font-semibold hover:underline">{item.name}</a
            >
            {#if isAlgorithmic && item.computed_rating !== null}
              <span class="text-xs font-semibold text-primary tabular-nums">
                {formatRating(item)}
              </span>
              {#if delta !== null && delta !== 0}
                <span
                  class="min-w-[28px] text-right text-xs tabular-nums
                    {delta > 0 ? 'text-green-600' : 'text-red-500'}"
                >
                  {delta > 0 ? `↑${delta}` : `↓${Math.abs(delta)}`}
                </span>
              {/if}
            {:else if s}
              <span class="text-xs text-muted-foreground">{wlRecord(s)}</span>
              <span class="min-w-[36px] text-right text-xs font-semibold"
                >{winRate(s.wins.length, s.losses.length)}</span
              >
            {/if}
          </div>
        {/each}
      </div>
    {/if}

    {#if canEdit}
      <p class="text-xs text-muted-foreground">
        {#if isAlgorithmic}
          Drag ⠿ to reorder · "Sync to algorithm" resets order to computed
          ratings · Save to persist
        {:else}
          Click the rank number to edit · Drag ⠿ to reorder · Click Save to
          persist
        {/if}
      </p>
    {/if}
  </div>
{/if}
