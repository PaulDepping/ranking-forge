<script lang="ts">
  import { page } from "$app/state";

  let { children, data } = $props();
</script>

<div class="space-y-4">
  {#if !page.data.user}
    <div class="border-b bg-muted px-4 py-2 text-sm text-muted-foreground">
      You're viewing a shared project · <a
        href="/register"
        class="underline hover:text-foreground">Sign up</a
      > to build your own rankings
    </div>
  {/if}

  <div class="space-y-4 {page.data.wide ? 'mx-auto max-w-5xl px-4' : ''}">
    <div>
      <a
        href={page.data.user ? "/projects" : "/"}
        class="text-sm text-muted-foreground hover:text-foreground"
        >{page.data.user ? "← Projects" : "← Home"}</a
      >
      <h1 class="mt-1 text-2xl font-bold">{data.project.name}</h1>
      {#if data.project.game_name}
        <p class="text-sm text-muted-foreground">{data.project.game_name}</p>
      {/if}
    </div>
  </div>

  {@render children()}
</div>
