<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { makeApi } from "$lib/api";
  import { invalidateAll } from "$app/navigation";

  let { projectId }: { projectId: string } = $props();

  let name = $state("");
  let submitting = $state(false);
  let error = $state<string | null>(null);

  async function submit() {
    const trimmed = name.trim();
    if (!trimmed) return;
    submitting = true;
    error = null;
    const api = makeApi(fetch);
    const res = await api.post(`/projects/${projectId}/players`, {
      name: trimmed,
    });
    submitting = false;
    if (res.ok) {
      name = "";
      await invalidateAll();
    } else {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to add player" }));
      error = err.message;
    }
  }
</script>

<div class="space-y-4">
  <div class="space-y-2">
    <Label for="player-name">Player name</Label>
    <Input
      id="player-name"
      bind:value={name}
      placeholder="e.g. Mang0"
      disabled={submitting}
    />
    {#if error}
      <p class="text-sm text-destructive">{error}</p>
    {/if}
    <p class="text-xs text-muted-foreground">
      Creates a player with no start.gg account. You can link one later.
    </p>
  </div>
  <Button onclick={submit} disabled={submitting || !name.trim()} class="w-full">
    {submitting ? "Adding…" : "Add player"}
  </Button>
</div>
