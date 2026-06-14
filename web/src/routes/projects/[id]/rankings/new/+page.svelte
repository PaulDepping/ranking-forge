<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import type { ActionData } from "./$types";

  let { form }: { form: ActionData } = $props();

  let algorithm = $state<"" | "elo" | "glicko2">("");
</script>

<div class="container mx-auto max-w-md py-8 px-4">
  <h1 class="mb-6 text-2xl font-bold">New ranking</h1>
  <form method="POST" class="flex flex-col gap-4">
    <div class="flex flex-col gap-1.5">
      <Label for="name">Name</Label>
      <Input id="name" name="name" required placeholder="2025 Season" />
    </div>
    <div class="flex flex-col gap-1.5">
      <Label for="description"
        >Description <span class="text-muted-foreground">(optional)</span
        ></Label
      >
      <Input
        id="description"
        name="description"
        placeholder="Brief description"
      />
    </div>

    <div class="flex flex-col gap-2">
      <Label>Algorithm</Label>

      <!-- Manual card -->
      <label
        class="flex cursor-pointer items-start gap-3 rounded-md border p-3 transition-colors
          {algorithm === ''
          ? 'border-primary bg-muted/40'
          : 'border-border hover:bg-muted/20'}"
      >
        <input
          type="radio"
          name="algorithm"
          value=""
          bind:group={algorithm}
          class="mt-0.5 accent-primary"
        />
        <div>
          <div class="text-sm font-semibold">Manual</div>
          <div class="text-xs text-muted-foreground">
            You set the order by dragging players
          </div>
        </div>
      </label>

      <!-- Elo card -->
      <label
        class="flex cursor-pointer items-start gap-3 rounded-md border p-3 transition-colors
          {algorithm === 'elo'
          ? 'border-primary bg-muted/40'
          : 'border-border hover:bg-muted/20'}"
      >
        <input
          type="radio"
          name="algorithm"
          value="elo"
          bind:group={algorithm}
          class="mt-0.5 accent-primary"
        />
        <div class="w-full">
          <div class="flex items-baseline gap-2">
            <span class="text-sm font-semibold">Elo</span>
            <a
              href="https://en.wikipedia.org/wiki/Elo_rating_system"
              target="_blank"
              rel="noopener noreferrer"
              class="text-xs text-primary hover:underline"
              onclick={(e) => e.stopPropagation()}>Wikipedia ↗</a
            >
          </div>
          <div class="text-xs text-muted-foreground">
            Classic rating system — players gain or lose points based on results
            relative to opponent strength
          </div>
          {#if algorithm === "elo"}
            <div class="mt-3 flex flex-col gap-3 border-t pt-3">
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
                  value="32"
                  min="1"
                  max="256"
                  class="w-20 text-right"
                />
              </div>
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="text-xs font-semibold">Initial rating</div>
                  <div class="text-xs text-muted-foreground">
                    Starting rating for all players. 1500 is the universal
                    convention.
                  </div>
                </div>
                <Input
                  type="number"
                  name="elo_initial"
                  value="1500"
                  min="1"
                  class="w-20 text-right"
                />
              </div>
            </div>
          {/if}
        </div>
      </label>

      <!-- Glicko-2 card -->
      <label
        class="flex cursor-pointer items-start gap-3 rounded-md border p-3 transition-colors
          {algorithm === 'glicko2'
          ? 'border-primary bg-muted/40'
          : 'border-border hover:bg-muted/20'}"
      >
        <input
          type="radio"
          name="algorithm"
          value="glicko2"
          bind:group={algorithm}
          class="mt-0.5 accent-primary"
        />
        <div class="w-full">
          <div class="flex items-baseline gap-2">
            <span class="text-sm font-semibold">Glicko-2</span>
            <a
              href="https://en.wikipedia.org/wiki/Glicko_rating_system"
              target="_blank"
              rel="noopener noreferrer"
              class="text-xs text-primary hover:underline"
              onclick={(e) => e.stopPropagation()}>Wikipedia ↗</a
            >
          </div>
          <div class="text-xs text-muted-foreground">
            Extends Elo with a rating deviation (RD) — confidence interval on
            each player's true strength
          </div>
          {#if algorithm === "glicko2"}
            <div class="mt-3 flex flex-col gap-3 border-t pt-3">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="text-xs font-semibold">τ (tau)</div>
                  <div class="text-xs text-muted-foreground">
                    Controls volatility change rate. Glickman recommends
                    0.3–1.2; lower = more stable.
                  </div>
                </div>
                <Input
                  type="number"
                  name="g2_tau"
                  value="0.5"
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
                    Rating deviation for a new player. Shrinks as they play more
                    sets.
                  </div>
                </div>
                <Input
                  type="number"
                  name="g2_rd"
                  value="350"
                  min="50"
                  max="700"
                  class="w-20 text-right"
                />
              </div>
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="text-xs font-semibold">
                    Initial volatility (σ)
                  </div>
                  <div class="text-xs text-muted-foreground">
                    Expected rating fluctuation for a new player. Glickman
                    recommends 0.06.
                  </div>
                </div>
                <Input
                  type="number"
                  name="g2_sigma"
                  value="0.06"
                  min="0.01"
                  max="1"
                  step="0.01"
                  class="w-20 text-right"
                />
              </div>
            </div>
          {/if}
        </div>
      </label>
    </div>

    {#if form?.error}
      <p class="text-sm text-destructive">{form.error}</p>
    {/if}
    <Button type="submit">Create ranking</Button>
  </form>
</div>
