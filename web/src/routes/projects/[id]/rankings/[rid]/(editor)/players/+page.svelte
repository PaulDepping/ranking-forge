<script lang="ts">
  import type { PageData } from "./$types";
  import { Button } from "$lib/components/ui/button";
  import { Badge } from "$lib/components/ui/badge";
  import { makeApi } from "$lib/api";
  import { invalidateAll } from "$app/navigation";

  let { data }: { data: PageData } = $props();
  const api = makeApi(fetch);

  const rankingPlayerIds = $derived(
    new Set(data.rankingPlayers.map((rp) => rp.player_id)),
  );

  async function addPlayer(playerId: string) {
    await api.addRankingPlayer(data.project.id, data.ranking.id, playerId);
    await invalidateAll();
  }

  async function removePlayer(playerId: string) {
    await api.removeRankingPlayer(data.project.id, data.ranking.id, playerId);
    await invalidateAll();
  }
</script>

<div class="container mx-auto py-8 max-w-4xl">
  <h2 class="text-xl font-semibold mb-4">Players in "{data.ranking.name}"</h2>

  <div class="grid grid-cols-2 gap-6">
    <div>
      <h3 class="font-medium mb-2 text-muted-foreground">Project pool</h3>
      <div class="flex flex-col gap-2">
        {#each data.pool as player}
          <div
            class="flex items-center justify-between border rounded px-3 py-2"
          >
            <span>{player.name}</span>
            {#if rankingPlayerIds.has(player.id)}
              <Badge variant="secondary">In ranking</Badge>
            {:else}
              <Button size="sm" onclick={() => addPlayer(player.id)}>Add</Button
              >
            {/if}
          </div>
        {/each}
      </div>
    </div>

    <div>
      <h3 class="font-medium mb-2 text-muted-foreground">In this ranking</h3>
      <div class="flex flex-col gap-2">
        {#each data.rankingPlayers as rp}
          <div
            class="flex items-center justify-between border rounded px-3 py-2"
          >
            <div>
              <span>{rp.name}</span>
              {#if rp.notes}
                <p class="text-xs text-muted-foreground">{rp.notes}</p>
              {/if}
            </div>
            <Button
              size="sm"
              variant="destructive"
              onclick={() => removePlayer(rp.player_id)}>Remove</Button
            >
          </div>
        {/each}
      </div>
    </div>
  </div>
</div>
