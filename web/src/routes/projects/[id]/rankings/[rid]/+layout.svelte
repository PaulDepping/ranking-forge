<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import * as Tabs from "$lib/components/ui/tabs";
  import { Separator } from "$lib/components/ui/separator";
  import * as Popover from "$lib/components/ui/popover";

  let { children, data } = $props();

  const allTabs = [
    { label: "Players", href: "players", minRole: "editor" as const },
    { label: "Tournaments", href: "tournaments", minRole: null },
    { label: "Stats", href: "stats", minRole: null },
    { label: "H2H", href: "h2h", minRole: null },
    { label: "Ranking", href: "ranking", minRole: null },
    { label: "Settings", href: "settings", minRole: "editor" as const },
  ];

  const tabs = $derived(
    allTabs.filter((t) => {
      const role = data.project.user_role;
      if (t.minRole === null) return true;
      if (t.minRole === "editor") return role === "editor" || role === "owner";
      return false;
    }),
  );

  function tabHref(slug: string) {
    return `/projects/${data.project.id}/rankings/${data.ranking.id}/${slug}`;
  }

  const currentTab = $derived(
    tabs.find((t) => page.url.pathname.startsWith(tabHref(t.href)))?.href ??
      tabs[0]?.href,
  );

  let switcherOpen = $state(false);

  function switchRanking(rid: string) {
    switcherOpen = false;
    const tab = currentTab ?? "stats";
    goto(`/projects/${data.project.id}/rankings/${rid}/${tab}`);
  }
</script>

<div class="space-y-4 {page.data.wide ? 'mx-auto max-w-5xl px-4' : ''}">
  <div class="px-4">
    <div class="flex items-center gap-1 text-sm text-muted-foreground">
      <a href="/projects/{data.project.id}" class="hover:text-foreground"
        >{data.project.name}</a
      >
      <span>/</span>
      <Popover.Root bind:open={switcherOpen}>
        <Popover.Trigger>
          <button class="font-medium text-foreground hover:underline">
            {data.ranking.name} ▾
          </button>
        </Popover.Trigger>
        <Popover.Content class="w-56 p-1" align="start">
          {#each data.rankings as r (r.id)}
            <button
              class="w-full rounded px-3 py-1.5 text-left text-sm transition-colors
                {r.id === data.ranking.id
                ? 'font-semibold text-primary'
                : 'text-foreground hover:bg-muted'}"
              onclick={() => switchRanking(r.id)}
            >
              {r.name}
            </button>
          {/each}
          {#if data.project.user_role === "editor" || data.project.user_role === "owner"}
            <Separator class="my-1" />
            <a
              href="/projects/{data.project.id}/rankings/new"
              class="block rounded px-3 py-1.5 text-sm text-primary hover:bg-muted"
              onclick={() => (switcherOpen = false)}
            >
              + New ranking
            </a>
          {/if}
        </Popover.Content>
      </Popover.Root>
    </div>
  </div>

  <Tabs.Root value={currentTab} onValueChange={(v) => v !== undefined && goto(tabHref(v))}>
    <div class="px-4">
      <Tabs.List>
        {#each tabs as tab (tab.href)}
          <Tabs.Trigger value={tab.href}>{tab.label}</Tabs.Trigger>
        {/each}
      </Tabs.List>
    </div>
  </Tabs.Root>

  <Separator />

  {@render children()}
</div>
