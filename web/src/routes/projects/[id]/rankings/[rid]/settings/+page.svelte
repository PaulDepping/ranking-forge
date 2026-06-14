<script lang="ts">
  import { untrack } from "svelte";
  import { enhance } from "$app/forms";
  import { makeApi } from "$lib/api";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Separator } from "$lib/components/ui/separator";
  import * as AlertDialog from "$lib/components/ui/alert-dialog";

  let { data, form } = $props();

  let name = $state(untrack(() => data.ranking.name));
  let description = $state(untrack(() => data.ranking.description ?? ""));
  let published = $state(untrack(() => data.ranking.published));
  $effect(() => {
    name = data.ranking.name;
    description = data.ranking.description ?? "";
    published = data.ranking.published;
  });

  let deleteDialogOpen = $state(false);
  let deleteFormEl = $state<HTMLFormElement | null>(null);

  let recomputeStatus = $state<"idle" | "sending" | "sent" | "error">("idle");

  async function triggerRecompute() {
    recomputeStatus = "sending";
    const api = makeApi(fetch);
    const res = await api.recomputeRanking(data.project.id, data.ranking.id);
    if (res.ok) {
      recomputeStatus = "sent";
      setTimeout(() => (recomputeStatus = "idle"), 3000);
    } else {
      recomputeStatus = "error";
      setTimeout(() => (recomputeStatus = "idle"), 3000);
    }
  }

  const eloConfig = $derived(
    data.ranking.algorithm_config as {
      k_factor?: number;
      initial_rating?: number;
    },
  );
  let eloK = $state(untrack(() => String(eloConfig.k_factor ?? 32)));
  let eloInitial = $state(
    untrack(() => String(eloConfig.initial_rating ?? 1500)),
  );

  const g2Config = $derived(
    data.ranking.algorithm_config as {
      tau?: number;
      initial_rd?: number;
      initial_volatility?: number;
    },
  );
  let g2Tau = $state(untrack(() => String(g2Config.tau ?? 0.5)));
  let g2Rd = $state(untrack(() => String(g2Config.initial_rd ?? 350)));
  let g2Sigma = $state(
    untrack(() => String(g2Config.initial_volatility ?? 0.06)),
  );

  function algorithmLabel(a: string | null): string {
    if (!a) return "Manual";
    if (a === "elo") return "Elo";
    if (a === "glicko2") return "Glicko-2";
    return a;
  }

  function algorithmWikiUrl(a: string | null): string | null {
    if (a === "elo") return "https://en.wikipedia.org/wiki/Elo_rating_system";
    if (a === "glicko2")
      return "https://en.wikipedia.org/wiki/Glicko_rating_system";
    return null;
  }
</script>

<div class="container mx-auto max-w-lg space-y-8 px-4 py-8">
  <!-- General -->
  <div class="space-y-4">
    <h2 class="text-lg font-semibold">General</h2>
    <form method="POST" action="?/save" use:enhance class="space-y-3">
      <div class="flex flex-col gap-1.5">
        <Label for="name">Name</Label>
        <Input id="name" name="name" bind:value={name} required />
      </div>
      <div class="flex flex-col gap-1.5">
        <Label for="description"
          >Description <span class="text-muted-foreground text-sm"
            >(optional)</span
          ></Label
        >
        <Input id="description" name="description" bind:value={description} />
      </div>
      {#if form?.saveError}
        <p class="text-sm text-destructive">{form.saveError}</p>
      {/if}
      {#if form?.saved}
        <p class="text-sm text-green-600">Saved.</p>
      {/if}
      <Button type="submit">Save</Button>
    </form>
  </div>

  <Separator />

  <!-- Publishing -->
  <div class="space-y-3">
    <h2 class="text-lg font-semibold">Publishing</h2>
    <div class="flex items-center justify-between rounded-md border p-3">
      <div>
        <p class="text-sm font-medium">Public</p>
        <p class="text-xs text-muted-foreground">
          Anyone with the link can view stats, H2H, and ranking
        </p>
      </div>
      <form method="POST" action="?/save" use:enhance>
        <input type="hidden" name="name" value={data.ranking.name} />
        <input
          type="hidden"
          name="published"
          value={published ? "false" : "true"}
        />
        <Button
          type="submit"
          variant={published ? "default" : "outline"}
          size="sm"
        >
          {published ? "Public" : "Private"}
        </Button>
      </form>
    </div>
  </div>

  <Separator />

  <!-- Algorithm -->
  <div class="space-y-3">
    <h2 class="text-lg font-semibold">Algorithm</h2>
    <div class="flex items-center gap-2">
      <span class="text-sm font-semibold"
        >{algorithmLabel(data.ranking.algorithm)}</span
      >
      {#if algorithmWikiUrl(data.ranking.algorithm)}
        <a
          href={algorithmWikiUrl(data.ranking.algorithm)!}
          target="_blank"
          rel="noopener noreferrer"
          class="text-xs text-primary hover:underline">Wikipedia ↗</a
        >
      {/if}
    </div>
    <p class="text-xs text-muted-foreground">
      Set at creation. Create a new ranking to use a different algorithm.
    </p>

    {#if data.ranking.algorithm === "elo"}
      <form
        method="POST"
        action="?/saveAlgorithmConfig"
        use:enhance
        class="space-y-3"
      >
        <input type="hidden" name="algorithm" value="elo" />
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">K-factor</div>
            <div class="text-xs text-muted-foreground">
              Points at stake per set. 32 is standard; lower (16) = slow
              changes, higher (64) = fast.
            </div>
          </div>
          <Input
            type="number"
            name="elo_k"
            bind:value={eloK}
            min="1"
            max="256"
            class="w-20 text-right"
          />
        </div>
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">Initial rating</div>
            <div class="text-xs text-muted-foreground">
              Starting rating for all players. 1500 is the universal convention.
            </div>
          </div>
          <Input
            type="number"
            name="elo_initial"
            bind:value={eloInitial}
            min="1"
            class="w-20 text-right"
          />
        </div>
        {#if form?.algoError}
          <p class="text-sm text-destructive">{form.algoError}</p>
        {/if}
        {#if form?.algoSaved}
          <p class="text-sm text-green-600">Saved. Recompute queued.</p>
        {/if}
        <Button type="submit">Save &amp; recompute</Button>
      </form>
    {:else if data.ranking.algorithm === "glicko2"}
      <form
        method="POST"
        action="?/saveAlgorithmConfig"
        use:enhance
        class="space-y-3"
      >
        <input type="hidden" name="algorithm" value="glicko2" />
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">τ (tau)</div>
            <div class="text-xs text-muted-foreground">
              Controls volatility change rate. Glickman recommends 0.3–1.2;
              lower = more stable.
            </div>
          </div>
          <Input
            type="number"
            name="g2_tau"
            bind:value={g2Tau}
            min="0.1"
            max="2"
            step="0.1"
            class="w-20 text-right"
          />
        </div>
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">Initial RD</div>
            <div class="text-xs text-muted-foreground">
              Rating deviation for a new player. Shrinks as they play more sets.
            </div>
          </div>
          <Input
            type="number"
            name="g2_rd"
            bind:value={g2Rd}
            min="50"
            max="700"
            class="w-20 text-right"
          />
        </div>
        <div class="flex items-center justify-between gap-4">
          <div>
            <div class="text-xs font-semibold">Initial volatility (σ)</div>
            <div class="text-xs text-muted-foreground">
              Expected rating fluctuation for a new player. Glickman recommends
              0.06.
            </div>
          </div>
          <Input
            type="number"
            name="g2_sigma"
            bind:value={g2Sigma}
            min="0.01"
            max="1"
            step="0.01"
            class="w-20 text-right"
          />
        </div>
        {#if form?.algoError}
          <p class="text-sm text-destructive">{form.algoError}</p>
        {/if}
        {#if form?.algoSaved}
          <p class="text-sm text-green-600">Saved. Recompute queued.</p>
        {/if}
        <Button type="submit">Save &amp; recompute</Button>
      </form>
    {/if}
  </div>

  <Separator />

  <!-- Recompute (algorithmic rankings only) -->
  {#if data.ranking.algorithm}
    <div class="space-y-2">
      <h2 class="text-lg font-semibold">Recompute</h2>
      <p class="text-sm text-muted-foreground">
        Manually trigger a recalculation. This happens automatically after
        imports and event inclusion changes.
      </p>
      <Button
        variant="outline"
        onclick={triggerRecompute}
        disabled={recomputeStatus === "sending"}
      >
        {#if recomputeStatus === "sending"}
          Sending…
        {:else if recomputeStatus === "sent"}
          Queued ✓
        {:else if recomputeStatus === "error"}
          Failed
        {:else}
          Recompute now
        {/if}
      </Button>
    </div>

    <Separator />
  {/if}

  <!-- Danger zone -->
  <div class="space-y-3">
    <h2 class="text-lg font-semibold text-destructive">Danger zone</h2>
    <div
      class="flex items-center justify-between rounded-md border border-destructive/40 p-4"
    >
      <div>
        <p class="font-medium">Delete this ranking</p>
        <p class="text-sm text-muted-foreground">
          Removes all players, event inclusion, and computed stats.
        </p>
      </div>
      <form
        method="POST"
        action="?/delete"
        use:enhance
        bind:this={deleteFormEl}
        class="ml-4"
      >
        <Button
          type="button"
          variant="destructive"
          size="sm"
          onclick={() => (deleteDialogOpen = true)}
        >
          Delete
        </Button>
      </form>
    </div>
    {#if form?.deleteError}
      <p class="text-sm text-destructive">{form.deleteError}</p>
    {/if}
  </div>
</div>

<AlertDialog.Root bind:open={deleteDialogOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Delete "{data.ranking.name}"?</AlertDialog.Title>
      <AlertDialog.Description>
        Removes all players, event inclusion, and computed stats for this
        ranking. This cannot be undone.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action
        class="bg-destructive text-destructive-foreground hover:bg-destructive/90"
        onclick={() => {
          deleteDialogOpen = false;
          deleteFormEl?.requestSubmit();
        }}
      >
        Delete ranking
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
