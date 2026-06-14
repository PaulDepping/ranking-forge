<script lang="ts">
  import type { PageData } from "./$types";
  import * as Card from "$lib/components/ui/card";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import type { Ranking } from "$lib/types";

  let { data }: { data: PageData } = $props();
  const isEditor = $derived(
    data.project.user_role === "owner" || data.project.user_role === "editor",
  );

  function algorithmLabel(r: Ranking): string {
    if (!r.algorithm) return "Manual";
    if (r.algorithm === "elo") return "Elo";
    if (r.algorithm === "glicko2") return "Glicko-2";
    return r.algorithm;
  }
</script>

<div class="container mx-auto max-w-3xl py-8 px-4">
  <div class="mb-6 flex items-center justify-between">
    <h2 class="text-xl font-semibold">Rankings</h2>
    {#if isEditor}
      <Button href="/projects/{data.project.id}/rankings/new" size="sm"
        >New ranking</Button
      >
    {/if}
  </div>

  {#if data.rankings.length === 0}
    <p class="text-muted-foreground">
      No rankings yet.{#if isEditor}
        Create one to get started.{/if}
    </p>
  {:else}
    <div class="flex flex-col gap-3">
      {#each data.rankings as ranking (ranking.id)}
        <a href="/projects/{data.project.id}/rankings/{ranking.id}/stats">
          <Card.Root class="cursor-pointer transition-colors hover:bg-muted/50">
            <Card.Header>
              <div class="flex items-center justify-between">
                <Card.Title>{ranking.name}</Card.Title>
                {#if ranking.published}
                  <Badge variant="secondary">Public</Badge>
                {:else}
                  <Badge variant="outline">Private</Badge>
                {/if}
              </div>
              <div
                class="flex items-center gap-2 text-sm text-muted-foreground"
              >
                <span>{algorithmLabel(ranking)}</span>
                {#if (ranking as any).player_count !== undefined}
                  <span>·</span>
                  <span>{(ranking as any).player_count} players</span>
                {/if}
              </div>
              {#if ranking.description}
                <Card.Description>{ranking.description}</Card.Description>
              {/if}
            </Card.Header>
          </Card.Root>
        </a>
      {/each}
    </div>
  {/if}
</div>
