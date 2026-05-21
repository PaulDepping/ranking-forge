<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Badge } from "$lib/components/ui/badge";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { ScrollArea } from "$lib/components/ui/scroll-area";
  import * as Tabs from "$lib/components/ui/tabs";
  import * as ToggleGroup from "$lib/components/ui/toggle-group";
  import { makeApi } from "$lib/api";
  import { invalidateAll } from "$app/navigation";
  import type {
    Player,
    TournamentData,
    TournamentParticipant,
    TournamentEntrantOrdered,
  } from "$lib/types";
  import { toOrdinal } from "$lib/utils";

  let {
    projectId,
    players,
    onClose,
  }: { projectId: string; players: Player[]; onClose: () => void } = $props();

  let tournamentInput = $state("");
  let loading = $state(false);
  let fetchError = $state<string | null>(null);
  let tournamentData = $state<TournamentData | null>(null);
  let activeTab = $state("all");
  let sortMode = $state<"placement" | "seed">("placement");
  let search = $state("");
  let selected = $state(new Set<number>());
  let submitting = $state(false);
  let addError = $state<string | null>(null);

  const alreadyAddedIds = $derived(
    new Set(players.flatMap((p) => p.accounts.map((a) => a.startgg_user_id))),
  );

  // Flat lookup of every known entrant by startgg_user_id (for add-selected)
  const allEntrantMap = $derived.by(() => {
    const map = new Map<
      number,
      { startgg_user_id: number; handle: string; name: string }
    >();
    if (!tournamentData) return map;
    for (const p of tournamentData.all_participants)
      map.set(p.startgg_user_id, p);
    for (const ev of tournamentData.events) {
      for (const e of ev.entrants) {
        if (!map.has(e.startgg_user_id)) map.set(e.startgg_user_id, e);
      }
    }
    return map;
  });

  type DisplayEntrant = (TournamentParticipant | TournamentEntrantOrdered) & {
    seed?: number | null;
    placement?: number | null;
  };

  const visibleEntrants = $derived.by((): DisplayEntrant[] => {
    if (!tournamentData) return [];
    if (activeTab === "all") {
      return [...tournamentData.all_participants].sort((a, b) =>
        a.name.localeCompare(b.name),
      );
    }
    const ev = tournamentData.events.find((e) => String(e.id) === activeTab);
    if (!ev) return [];
    return [...ev.entrants].sort((a, b) => {
      const va = sortMode === "placement" ? a.placement : a.seed;
      const vb = sortMode === "placement" ? b.placement : b.seed;
      if (va == null && vb == null) return 0;
      if (va == null) return 1;
      if (vb == null) return -1;
      return va - vb;
    });
  });

  const filteredEntrants = $derived(
    visibleEntrants.filter((e) => {
      const q = search.toLowerCase();
      return (
        e.name.toLowerCase().includes(q) || e.handle.toLowerCase().includes(q)
      );
    }),
  );

  const selectedCount = $derived(selected.size);
  const alreadyAddedCount = $derived(
    filteredEntrants.filter((e) => alreadyAddedIds.has(e.startgg_user_id))
      .length,
  );
  const selectableFiltered = $derived(
    filteredEntrants.filter((e) => !alreadyAddedIds.has(e.startgg_user_id)),
  );
  const allSelected = $derived(
    selectableFiltered.length > 0 &&
      selectableFiltered.every((e) => selected.has(e.startgg_user_id)),
  );

  function toggleAll(checked: boolean) {
    const next = new Set(selected);
    if (checked) {
      for (const e of selectableFiltered) next.add(e.startgg_user_id);
    } else {
      for (const e of selectableFiltered) next.delete(e.startgg_user_id);
    }
    selected = next;
  }

  function toggleEntrant(id: number) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
  }

  function formatRank(e: DisplayEntrant): string {
    const n = sortMode === "placement" ? e.placement : e.seed;
    if (n == null) return "—";
    if (sortMode === "seed") return `#${n}`;
    return toOrdinal(n);
  }

  async function fetchTournamentData() {
    if (!tournamentInput.trim()) return;
    loading = true;
    fetchError = null;
    tournamentData = null;
    activeTab = "all";
    selected = new Set();
    search = "";
    const api = makeApi(fetch);
    const res = await api.get(
      `/projects/${projectId}/tournament-entrants?tournament=${encodeURIComponent(tournamentInput.trim())}`,
    );
    loading = false;
    if (res.ok) {
      tournamentData = await res.json();
    } else {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to fetch entrants" }));
      fetchError = err.message;
    }
  }

  async function addSelected() {
    const entries = [...selected]
      .map((id) => allEntrantMap.get(id))
      .filter((e): e is NonNullable<typeof e> => e != null)
      .map((e) => ({
        name: e.name,
        startgg_user_id: e.startgg_user_id,
        handle: e.handle,
      }));
    if (!entries.length) return;
    submitting = true;
    addError = null;
    const api = makeApi(fetch);
    const res = await api.post(`/projects/${projectId}/players/bulk`, {
      players: entries,
    });
    submitting = false;
    if (res.ok) {
      await invalidateAll();
      onClose();
    } else {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to add players" }));
      addError = err.message;
    }
  }
</script>

<div class="space-y-3">
  <div class="flex gap-2">
    <div class="flex-1">
      <Label for="tournament-input" class="sr-only"
        >Tournament URL or slug</Label
      >
      <Input
        id="tournament-input"
        bind:value={tournamentInput}
        placeholder="genesis-9 or start.gg/tournament/genesis-9"
        disabled={loading}
      />
    </div>
    <Button
      onclick={fetchTournamentData}
      disabled={loading || !tournamentInput.trim()}
    >
      {loading ? "Fetching…" : "Fetch"}
    </Button>
  </div>

  {#if fetchError}
    <p class="text-sm text-destructive">{fetchError}</p>
  {/if}

  {#if tournamentData}
    <!-- Event tabs -->
    <Tabs.Root bind:value={activeTab}>
      <div class="overflow-x-auto">
        <Tabs.List class="w-max min-w-full">
          <Tabs.Trigger value="all">All</Tabs.Trigger>
          {#each tournamentData.events as ev (ev.id)}
            <Tabs.Trigger value={String(ev.id)}>{ev.name}</Tabs.Trigger>
          {/each}
        </Tabs.List>
      </div>
    </Tabs.Root>

    <!-- Search + sort toggle row -->
    <div class="flex gap-2 items-center">
      <Input
        bind:value={search}
        placeholder="Search entrants…"
        class="flex-1"
      />
      {#if activeTab !== "all"}
        <ToggleGroup.Root
          type="single"
          value={sortMode}
          onValueChange={(v) => {
            if (v === "placement" || v === "seed") sortMode = v;
          }}
          class="flex-shrink-0"
        >
          <ToggleGroup.Item value="placement" class="h-8 text-xs"
            >Placement</ToggleGroup.Item
          >
          <ToggleGroup.Item value="seed" class="h-8 text-xs"
            >Seed</ToggleGroup.Item
          >
        </ToggleGroup.Root>
      {/if}
    </div>

    <!-- Select all -->
    <div class="flex items-center gap-2">
      <Checkbox
        id="select-all"
        checked={allSelected}
        onCheckedChange={toggleAll}
      />
      <Label for="select-all" class="cursor-pointer text-sm font-normal"
        >Select all</Label
      >
    </div>

    <ScrollArea class="h-52 rounded-md border">
      <div class="divide-y">
        {#each filteredEntrants as entrant (entrant.startgg_user_id)}
          {@const isAdded = alreadyAddedIds.has(entrant.startgg_user_id)}
          <div
            class="flex items-center gap-3 px-3 py-2 text-sm"
            class:opacity-50={isAdded}
          >
            <Checkbox
              id="entrant-{entrant.startgg_user_id}"
              checked={selected.has(entrant.startgg_user_id)}
              disabled={isAdded}
              onCheckedChange={() =>
                !isAdded && toggleEntrant(entrant.startgg_user_id)}
            />
            {#if activeTab !== "all"}
              <span
                class="w-8 text-right text-xs text-muted-foreground flex-shrink-0"
              >
                {formatRank(entrant)}
              </span>
            {/if}
            <Label
              for="entrant-{entrant.startgg_user_id}"
              class="flex flex-1 items-center gap-2 {isAdded
                ? 'cursor-default'
                : 'cursor-pointer'}"
            >
              <span class="font-medium">{entrant.name}</span>
              <span class="text-muted-foreground">{entrant.handle}</span>
            </Label>
            {#if isAdded}
              <Badge variant="secondary" class="text-xs">already added</Badge>
            {/if}
          </div>
        {/each}
      </div>
    </ScrollArea>

    {#if addError}<p class="text-sm text-destructive">{addError}</p>{/if}
    <div class="flex items-center justify-between">
      <span class="text-sm text-muted-foreground">
        {selectedCount} selected · {alreadyAddedCount} already added
      </span>
      <Button
        onclick={addSelected}
        disabled={selectedCount === 0 || submitting}
      >
        {submitting
          ? "Adding…"
          : `Add ${selectedCount} player${selectedCount === 1 ? "" : "s"}`}
      </Button>
    </div>
  {/if}
</div>
