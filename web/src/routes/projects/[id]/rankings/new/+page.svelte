<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import type { ActionData } from "./$types";

  let { form }: { form: ActionData } = $props();
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
    {#if form?.error}
      <p class="text-sm text-destructive">{form.error}</p>
    {/if}
    <Button type="submit">Create ranking</Button>
  </form>
</div>
