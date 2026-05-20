<script lang="ts">
  import { enhance } from "$app/forms";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Alert } from "$lib/components/ui/alert";

  let { form, data } = $props();
</script>

<div class="flex min-h-[60vh] items-center justify-center">
  <div class="w-full max-w-sm space-y-6">
    <div class="space-y-1 text-center">
      <p class="text-lg font-semibold">Sign in to your account</p>
    </div>

    {#if form?.error}
      <Alert variant="destructive">{form.error}</Alert>
    {/if}

    <form method="POST" use:enhance class="space-y-4">
      <input type="hidden" name="redirect" value={data.redirectTo} />
      <div class="space-y-2">
        <Label for="email">Email</Label>
        <Input
          id="email"
          name="email"
          type="email"
          required
          autocomplete="email"
        />
      </div>
      <div class="space-y-2">
        <Label for="password">Password</Label>
        <Input
          id="password"
          name="password"
          type="password"
          required
          autocomplete="current-password"
        />
      </div>
      <Button type="submit" class="w-full">Sign in</Button>
    </form>

    <p class="text-center text-sm text-muted-foreground">
      No account? <a href="/register" class="underline hover:text-foreground"
        >Register</a
      >
    </p>
  </div>
</div>
