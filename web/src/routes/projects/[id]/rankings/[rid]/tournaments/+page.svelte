<script lang="ts">
  import { untrack } from "svelte";
  import { Badge } from "$lib/components/ui/badge";
  import { Button } from "$lib/components/ui/button";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import * as Collapsible from "$lib/components/ui/collapsible";
  import * as Popover from "$lib/components/ui/popover";
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Select from "$lib/components/ui/select";
  import DateRangePicker from "$lib/components/DateRangePicker.svelte";
  import type { DateRange } from "bits-ui";
  import { makeApi } from "$lib/api";
  import type { Tournament, TournamentEvent } from "$lib/types";
  import * as Card from "$lib/components/ui/card";
  import * as Empty from "$lib/components/ui/empty";
  import { formatDate } from "$lib/utils";
  import { invalidateAll } from "$app/navigation";

  let { data } = $props();

  const canEdit = $derived(
    data.project.user_role === "editor" || data.project.user_role === "owner",
  );

  // Local copy for optimistic toggle updates; synced when server data changes
  let tournaments = $state(untrack(() => [...data.tournaments]));
  $effect(() => {
    tournaments = [...data.tournaments];
  });

  // Filter state
  let filterOpen = $state(false);
  let search = $state("");
  let venueFilter = $state<"all" | "online" | "offline">("all");
  let minEntrants = $state<number | null>(null);
  let maxEntrants = $state<number | null>(null);
  let dateRange = $state<DateRange | undefined>(undefined);
  const dateFromStr = $derived(dateRange?.start?.toString() ?? "");
  const dateToStr = $derived(dateRange?.end?.toString() ?? "");
  let eventType = $state<"all" | "singles" | "teams">("all");

  // Bracket type filter
  type BracketTypeState = "neutral" | "required" | "excluded";

  const BRACKET_TYPES = [
    "DOUBLE_ELIMINATION",
    "SINGLE_ELIMINATION",
    "ROUND_ROBIN",
    "MATCHMAKING",
    "SWISS",
    "EXHIBITION",
    "RACE",
    "CIRCUIT",
    "CUSTOM_SCHEDULE",
    "ELIMINATION_ROUNDS",
  ] as const;

  // First 5 are common; last 5 are rare (separated by a divider in the UI)
  const COMMON_BRACKET_TYPES = BRACKET_TYPES.slice(0, 5);
  const RARE_BRACKET_TYPES = BRACKET_TYPES.slice(5);

  const BRACKET_TYPE_LABELS: Record<string, string> = {
    DOUBLE_ELIMINATION: "Double Elim",
    SINGLE_ELIMINATION: "Single Elim",
    ROUND_ROBIN: "Round Robin",
    MATCHMAKING: "Matchmaking",
    SWISS: "Swiss",
    EXHIBITION: "Exhibition",
    RACE: "Race",
    CIRCUIT: "Circuit",
    CUSTOM_SCHEDULE: "Custom Schedule",
    ELIMINATION_ROUNDS: "Elim. Rounds",
  };

  let bracketFilter = $state<Record<string, BracketTypeState>>(
    Object.fromEntries(
      BRACKET_TYPES.map((t) => [t, "neutral" as BracketTypeState]),
    ),
  );
  let bracketPopoverOpen = $state(false);
  let bracketDialogOpen = $state(false);

  const bracketReqCount = $derived(
    Object.values(bracketFilter).filter((s) => s === "required").length,
  );
  const bracketExclCount = $derived(
    Object.values(bracketFilter).filter((s) => s === "excluded").length,
  );
  const bracketTriggerLabel = $derived(
    bracketReqCount === 0 && bracketExclCount === 0
      ? "Brackets ▾"
      : [
          bracketReqCount > 0 ? `${bracketReqCount} req` : "",
          bracketExclCount > 0 ? `${bracketExclCount} excl` : "",
        ]
          .filter(Boolean)
          .join(" · ") + " ▾",
  );

  const rareActiveCount = $derived(
    RARE_BRACKET_TYPES.filter(
      (t) => bracketFilter[t] === "required" || bracketFilter[t] === "excluded",
    ).length,
  );

  function setBracketState(type: string, clicked: BracketTypeState) {
    // Clicking the already-active state resets to neutral
    bracketFilter = {
      ...bracketFilter,
      [type]: bracketFilter[type] === clicked ? "neutral" : clicked,
    };
  }

  function resetBracketFilter() {
    bracketFilter = Object.fromEntries(
      BRACKET_TYPES.map((t) => [t, "neutral" as BracketTypeState]),
    );
  }

  function resetAllFilters() {
    search = "";
    venueFilter = "all";
    minEntrants = null;
    maxEntrants = null;
    dateRange = undefined;
    eventType = "all";
    resetBracketFilter();
  }

  async function toggleEvent(
    projectId: string,
    rankingId: string,
    eventId: string,
    included: boolean,
  ) {
    const api = makeApi(fetch);
    const res = await api.patch(
      `/projects/${projectId}/rankings/${rankingId}/events/${eventId}`,
      { included },
    );
    if (!res.ok) return;

    const updated = await res.json();
    tournaments = tournaments.map((t) => ({
      ...t,
      events: t.events.map((e) =>
        e.id === updated.event_id ? { ...e, included: updated.included } : e,
      ),
    }));
  }

  function tournamentVisible(t: Tournament): boolean {
    if (venueFilter === "online" && !t.online) return false;
    if (venueFilter === "offline" && t.online) return false;
    if (dateFromStr && t.start_at && t.start_at.slice(0, 10) < dateFromStr)
      return false;
    if (dateToStr && t.start_at && t.start_at.slice(0, 10) > dateToStr)
      return false;
    return true;
  }

  function eventVisible(e: TournamentEvent, t: Tournament): boolean {
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      const nameMatch = e.name.toLowerCase().includes(q);
      const tournamentMatch = t.name.toLowerCase().includes(q);
      if (!nameMatch && !tournamentMatch) return false;
    }
    const min = minEntrants ?? 0;
    const max = maxEntrants ?? 0;
    if (min > 0 && (e.num_entrants ?? Infinity) < min) return false;
    if (max > 0 && (e.num_entrants ?? 0) > max) return false;
    if (eventType === "singles" && e.event_type !== null && e.event_type !== 1)
      return false;
    if (eventType === "teams" && e.event_type !== null && e.event_type !== 2)
      return false;

    const required = Object.entries(bracketFilter)
      .filter(([, s]) => s === "required")
      .map(([t]) => t);
    const excluded = Object.entries(bracketFilter)
      .filter(([, s]) => s === "excluded")
      .map(([t]) => t);

    if (required.length > 0 || excluded.length > 0) {
      if (e.bracket_types.length === 0) return true;
      for (const r of required) {
        if (!e.bracket_types.includes(r)) return false;
      }
      for (const x of excluded) {
        if (e.bracket_types.includes(x)) return false;
      }
    }

    return true;
  }

  const visibleTournaments = $derived(
    tournaments
      .filter((t) => tournamentVisible(t))
      .map((t) => ({
        ...t,
        events: t.events.filter((e) => eventVisible(e, t)),
      }))
      .filter((t) => t.events.length > 0),
  );

  const totalEventCount = $derived(
    tournaments.reduce((n, t) => n + t.events.length, 0),
  );
  const visibleEventCount = $derived(
    visibleTournaments.reduce((n, t) => n + t.events.length, 0),
  );

  const venueLabel = $derived(
    (
      {
        all: "Venue: All",
        online: "Online only",
        offline: "Offline only",
      } as const
    )[venueFilter],
  );
  const eventTypeLabel = $derived(
    ({ all: "All types", singles: "Singles", teams: "Teams" } as const)[
      eventType
    ],
  );

  async function bulkSetIncluded(included: boolean) {
    const toChange = visibleTournaments
      .flatMap((t) => t.events)
      .filter((e) => e.included !== included);

    if (toChange.length === 0) return;

    // Optimistic update so checkboxes reflect the change immediately
    const idSet = new Set(toChange.map((e) => e.id));
    tournaments = tournaments.map((t) => ({
      ...t,
      events: t.events.map((e) => (idSet.has(e.id) ? { ...e, included } : e)),
    }));

    await Promise.all(
      toChange.map((e) =>
        toggleEvent(data.project.id, data.ranking.id, e.id, included),
      ),
    );
  }

  function handleToggle(
    projectId: string,
    rankingId: string,
    event: TournamentEvent,
  ) {
    tournaments = tournaments.map((t) => ({
      ...t,
      events: t.events.map((e) =>
        e.id === event.id ? { ...e, included: !e.included } : e,
      ),
    }));
    toggleEvent(projectId, rankingId, event.id, !event.included);
  }

  // Delete tournament
  let deleteTournamentId = $state<string | null>(null);
  let deleteDialogOpen = $state(false);

  function confirmDelete(tournamentId: string) {
    deleteTournamentId = tournamentId;
    deleteDialogOpen = true;
  }

  async function deleteTournament() {
    if (!deleteTournamentId) return;
    const api = makeApi(fetch);
    const res = await api.delete(
      `/projects/${data.project.id}/tournaments/${deleteTournamentId}`,
    );
    deleteDialogOpen = false;
    deleteTournamentId = null;
    if (res.ok) {
      await invalidateAll();
    }
  }
</script>

{#snippet bracketRow(bt: string)}
  <div class="grid grid-cols-[1fr_28px_28px_28px] gap-1 items-center py-0.5">
    <span class="text-xs">{BRACKET_TYPE_LABELS[bt]}</span>
    {#each ["neutral", "required", "excluded"] as const as s}
      <Button
        type="button"
        size="icon"
        variant="ghost"
        class="h-5 w-5 mx-auto rounded border text-xs font-bold flex items-center justify-center
					{bracketFilter[bt] === s
          ? s === 'required'
            ? 'border-green-500 bg-green-950 text-green-400 hover:bg-green-950'
            : s === 'excluded'
              ? 'border-red-500 bg-red-950 text-red-400 hover:bg-red-950'
              : 'border-indigo-500 bg-indigo-950 text-indigo-400 hover:bg-indigo-950'
          : 'border-border bg-muted/30 text-transparent hover:text-muted-foreground'}"
        onclick={() => setBracketState(bt, s)}
      >
        {s === "neutral" ? "–" : s === "required" ? "✓" : "✕"}
      </Button>
    {/each}
  </div>
{/snippet}

<div class="space-y-4">
  <h2 class="text-lg font-semibold">Tournaments</h2>

  {#if tournaments.length === 0}
    <Empty.Root>
      <Empty.Header>
        <Empty.Title>No tournaments yet</Empty.Title>
        <Empty.Description
          >Run an import to pull in tournaments from start.gg.</Empty.Description
        >
      </Empty.Header>
    </Empty.Root>
  {:else}
    <Collapsible.Root bind:open={filterOpen}>
      <!-- Status line + toggle -->
      <div
        class="flex items-center justify-between text-sm text-muted-foreground"
      >
        <span>
          Showing <strong>{visibleTournaments.length}</strong> of {tournaments.length}
          tournaments · <strong>{visibleEventCount}</strong> of {totalEventCount}
          events
        </span>
        <Collapsible.Trigger>
          {#snippet child({ props })}
            <Button {...props} variant="outline" size="sm">
              ⚙ {canEdit ? "Filters & Actions" : "Filters"}
              {filterOpen ? "▲" : "▼"}
            </Button>
          {/snippet}
        </Collapsible.Trigger>
      </div>

      <Collapsible.Content>
        <!-- Collapsible filter panel -->
        <div
          class="rounded-md border border-border bg-muted/30 p-4 space-y-3 mt-2"
        >
          <!-- Header: label + clear button -->
          <div class="flex items-center justify-between">
            <span
              class="text-xs font-medium text-muted-foreground uppercase tracking-wide"
              >Filters</span
            >
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onclick={resetAllFilters}>Clear filters</Button
            >
          </div>

          <!-- Row 1: search + venue -->
          <div class="flex flex-wrap gap-2">
            <Input
              type="text"
              placeholder="Search tournament or event name…"
              bind:value={search}
              class="flex-1 min-w-48"
            />
            <Select.Root type="single" bind:value={venueFilter}>
              <Select.Trigger class="w-36">{venueLabel}</Select.Trigger>
              <Select.Content>
                <Select.Item value="all">Venue: All</Select.Item>
                <Select.Item value="online">Online only</Select.Item>
                <Select.Item value="offline">Offline only</Select.Item>
              </Select.Content>
            </Select.Root>
          </div>

          <!-- Row 2: entrant range + date range -->
          <div class="flex flex-wrap gap-2 items-center">
            <div class="flex items-center gap-1.5">
              <span class="text-xs text-muted-foreground whitespace-nowrap"
                >Entrants</span
              >
              <Input
                type="number"
                min="0"
                placeholder="min"
                bind:value={minEntrants}
                class="w-20"
              />
              <span class="text-muted-foreground">–</span>
              <Input
                type="number"
                min="0"
                placeholder="max"
                bind:value={maxEntrants}
                class="w-20"
              />
            </div>
            <DateRangePicker
              value={dateRange}
              onSelect={(r) => {
                dateRange = r;
              }}
              placeholder="All time"
            />
          </div>

          <!-- Row 3: event type + bracket filter -->
          <div class="flex flex-wrap gap-4 items-center">
            <div class="flex items-center gap-2">
              <span class="text-xs text-muted-foreground whitespace-nowrap"
                >Event type</span
              >
              <Select.Root type="single" bind:value={eventType}>
                <Select.Trigger class="w-28">{eventTypeLabel}</Select.Trigger>
                <Select.Content>
                  <Select.Item value="all">All types</Select.Item>
                  <Select.Item value="singles">Singles</Select.Item>
                  <Select.Item value="teams">Teams</Select.Item>
                </Select.Content>
              </Select.Root>
            </div>

            <Popover.Root bind:open={bracketPopoverOpen}>
              <Popover.Trigger>
                {#snippet child({ props })}
                  <Button
                    {...props}
                    variant="outline"
                    size="sm"
                    class={bracketReqCount > 0 || bracketExclCount > 0
                      ? "border-primary text-primary"
                      : ""}
                  >
                    {bracketTriggerLabel}
                  </Button>
                {/snippet}
              </Popover.Trigger>
              <Popover.Content class="w-64 p-3 gap-1" align="start">
                <div class="flex justify-between items-center mb-2">
                  <span
                    class="text-xs text-muted-foreground uppercase tracking-wide"
                    >Bracket Types</span
                  >
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    onclick={resetBracketFilter}>Reset</Button
                  >
                </div>

                <!-- Column headers -->
                <div class="grid grid-cols-[1fr_28px_28px_28px] gap-1 mb-1">
                  <span></span>
                  <span class="text-xs text-muted-foreground text-center"
                    >–</span
                  >
                  <span class="text-xs text-muted-foreground text-center"
                    >✓</span
                  >
                  <span class="text-xs text-muted-foreground text-center"
                    >✕</span
                  >
                </div>

                <!-- Common bracket types -->
                {#each COMMON_BRACKET_TYPES as bt}
                  {@render bracketRow(bt)}
                {/each}

                <div class="border-t border-border my-1.5"></div>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  class="w-full text-xs {rareActiveCount > 0
                    ? 'text-green-400'
                    : 'text-muted-foreground'}"
                  onclick={() => {
                    bracketPopoverOpen = false;
                    bracketDialogOpen = true;
                  }}
                >
                  All bracket types…{#if rareActiveCount > 0}<Badge
                      class="ml-1.5 bg-green-900 text-green-400 border-0 text-[9px] px-1.5"
                      >{rareActiveCount} active</Badge
                    >{/if}
                </Button>

                <!-- Legend -->
                <div
                  class="mt-2 pt-2 border-t border-border flex gap-3 flex-wrap"
                >
                  <span class="text-[10px] text-muted-foreground"
                    ><span class="text-indigo-400">–</span> don't care</span
                  >
                  <span class="text-[10px] text-muted-foreground"
                    ><span class="text-green-400">✓</span> required</span
                  >
                  <span class="text-[10px] text-muted-foreground"
                    ><span class="text-red-400">✕</span> excluded</span
                  >
                </div>
              </Popover.Content>
            </Popover.Root>
          </div>

          {#if canEdit}
            <!-- Divider + bulk actions -->
            <div
              class="flex items-center justify-between border-t border-border pt-3"
            >
              <span class="text-xs text-muted-foreground">
                Bulk actions apply to {visibleEventCount} visible event{visibleEventCount !==
                1
                  ? "s"
                  : ""}
              </span>
              <div class="flex gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onclick={() => bulkSetIncluded(true)}
                >
                  ✓ Include all visible
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  class="border-destructive text-destructive hover:bg-destructive/10"
                  onclick={() => bulkSetIncluded(false)}
                >
                  ✕ Exclude all visible
                </Button>
              </div>
            </div>
          {/if}
        </div>
      </Collapsible.Content>
    </Collapsible.Root>

    <Dialog.Root bind:open={bracketDialogOpen}>
      <Dialog.Content class="sm:max-w-sm p-4 gap-3">
        <Dialog.Header>
          <Dialog.Title>Bracket Types</Dialog.Title>
        </Dialog.Header>

        <div>
          <!-- Column headers -->
          <div class="grid grid-cols-[1fr_28px_28px_28px] gap-1 mb-1">
            <span></span>
            <span class="text-xs text-muted-foreground text-center">–</span>
            <span class="text-xs text-muted-foreground text-center">✓</span>
            <span class="text-xs text-muted-foreground text-center">✕</span>
          </div>

          <!-- Common types -->
          {#each COMMON_BRACKET_TYPES as bt}
            {@render bracketRow(bt)}
          {/each}

          <!-- Rare types -->
          <p
            class="text-[10px] uppercase tracking-wide text-muted-foreground mt-3 mb-1"
          >
            Rare formats
          </p>
          {#each RARE_BRACKET_TYPES as bt}
            {@render bracketRow(bt)}
          {/each}

          <!-- Legend -->
          <div class="flex gap-3 flex-wrap border-t border-border pt-2 mt-2">
            <span class="text-[10px] text-muted-foreground"
              ><span class="text-indigo-400">–</span> don't care</span
            >
            <span class="text-[10px] text-muted-foreground"
              ><span class="text-green-400">✓</span> required</span
            >
            <span class="text-[10px] text-muted-foreground"
              ><span class="text-red-400">✕</span> excluded</span
            >
          </div>
        </div>

        <Dialog.Footer>
          <Button
            variant="outline"
            size="sm"
            onclick={() => {
              bracketDialogOpen = false;
            }}
          >
            Done
          </Button>
        </Dialog.Footer>
      </Dialog.Content>
    </Dialog.Root>

    <!-- Delete tournament confirmation dialog -->
    <Dialog.Root bind:open={deleteDialogOpen}>
      <Dialog.Content class="sm:max-w-sm">
        <Dialog.Header>
          <Dialog.Title>Delete tournament?</Dialog.Title>
          <Dialog.Description
            >This will remove the tournament and all its events from this
            ranking. This action cannot be undone.</Dialog.Description
          >
        </Dialog.Header>
        <Dialog.Footer>
          <Button
            variant="outline"
            onclick={() => {
              deleteDialogOpen = false;
              deleteTournamentId = null;
            }}>Cancel</Button
          >
          <Button variant="destructive" onclick={deleteTournament}
            >Delete</Button
          >
        </Dialog.Footer>
      </Dialog.Content>
    </Dialog.Root>

    <!-- Tournament list — iterate visibleTournaments -->
    {#if visibleTournaments.length === 0}
      <Empty.Root>
        <Empty.Header>
          <Empty.Title>No results</Empty.Title>
          <Empty.Description
            >No tournaments match the current filters.</Empty.Description
          >
        </Empty.Header>
      </Empty.Root>
    {/if}
    <div class="space-y-3">
      {#each visibleTournaments as tournament (tournament.id)}
        <Card.Root class="py-0 gap-0">
          <Card.Header class="p-3">
            <Card.Title class="text-sm font-medium"
              >{tournament.name}</Card.Title
            >
            <Card.Description class="text-xs">
              {[tournament.city, tournament.addr_state, tournament.country_code]
                .filter(Boolean)
                .join(", ")}
              {tournament.online ? "(Online)" : ""}
              {tournament.start_at
                ? "· " + formatDate(tournament.start_at)
                : ""}
            </Card.Description>
            <Card.Action>
              <div class="flex items-center gap-2">
                <Badge variant="outline">
                  {tournament.events.length} event{tournament.events.length !==
                  1
                    ? "s"
                    : ""}
                </Badge>
                {#if canEdit}
                  <Button
                    variant="destructive"
                    size="sm"
                    onclick={() => confirmDelete(tournament.id)}
                  >
                    Delete
                  </Button>
                {/if}
              </div>
            </Card.Action>
          </Card.Header>
          <Card.Content class="p-0">
            <div class="divide-y divide-border border-t border-border">
              {#each tournament.events as event (event.id)}
                <Label
                  class="flex items-center justify-between px-4 py-2
									{canEdit ? 'cursor-pointer hover:bg-accent/50' : ''}"
                >
                  <div>
                    <span class="text-sm">{event.name}</span>
                    {#if event.num_entrants}
                      <span class="ml-2 text-xs text-muted-foreground"
                        >{event.num_entrants} entrants</span
                      >
                    {/if}
                  </div>
                  {#if canEdit}
                    <Checkbox
                      checked={event.included}
                      onCheckedChange={() =>
                        handleToggle(data.project.id, data.ranking.id, event)}
                    />
                  {/if}
                </Label>
              {/each}
            </div>
          </Card.Content>
        </Card.Root>
      {/each}
    </div>
  {/if}
</div>
