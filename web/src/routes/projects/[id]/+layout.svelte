<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { Separator } from "$lib/components/ui/separator";
  import * as Tabs from "$lib/components/ui/tabs";

  let { children, data } = $props();

  const allTabs = [
    { label: "Players", href: "players", minRole: "editor" as const },
    { label: "Import", href: "import", minRole: "editor" as const },
    { label: "Tournaments", href: "tournaments", minRole: null },
    { label: "Stats", href: "stats", minRole: null },
    { label: "H2H", href: "h2h", minRole: null },
    { label: "Ranking", href: "ranking", minRole: null },
    { label: "Settings", href: "settings", minRole: "owner" as const },
  ];

  const tabs = $derived(
    allTabs.filter((t) => {
      const role = data.project.user_role;
      if (t.minRole === null) return true;
      if (t.minRole === "editor") return role === "editor" || role === "owner";
      if (t.minRole === "owner") return role === "owner";
      return false;
    }),
  );

  function tabHref(slug: string) {
    return `/projects/${data.project.id}/${slug}`;
  }

  const currentTab = $derived(
    tabs.find((t) => page.url.pathname.startsWith(tabHref(t.href)))?.href ??
      tabs[0].href,
  );
</script>

<div class="space-y-4">
  <div class="space-y-4 {page.data.wide ? 'mx-auto max-w-5xl' : ''}">
    <div>
      <a
        href="/projects"
        class="text-sm text-muted-foreground hover:text-foreground">← Projects</a
      >
      <h1 class="mt-1 text-2xl font-bold">{data.project.name}</h1>
      {#if data.project.game_name}
        <p class="text-sm text-muted-foreground">{data.project.game_name}</p>
      {/if}
    </div>

    <Tabs.Root value={currentTab} onValueChange={(v) => v && goto(tabHref(v))}>
      <Tabs.List>
        {#each tabs as tab (tab.href)}
          <Tabs.Trigger value={tab.href}>{tab.label}</Tabs.Trigger>
        {/each}
      </Tabs.List>
    </Tabs.Root>
  </div>

  <Separator />

  {@render children()}
</div>
