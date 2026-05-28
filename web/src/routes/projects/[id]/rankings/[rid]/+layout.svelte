<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import * as Tabs from "$lib/components/ui/tabs";
  import { Separator } from "$lib/components/ui/separator";

  let { children, data } = $props();

  const allTabs = [
    { label: "Players", href: "players", minRole: "editor" as const },
    { label: "Tournaments", href: "tournaments", minRole: null },
    { label: "Stats", href: "stats", minRole: null },
    { label: "H2H", href: "h2h", minRole: null },
    { label: "Ranking", href: "ranking", minRole: null },
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
</script>

<div class="space-y-4 {page.data.wide ? 'mx-auto max-w-5xl px-4' : ''}">
  <div class="px-4">
    <p class="text-sm text-muted-foreground">
      <a href="/projects/{data.project.id}" class="hover:text-foreground"
        >{data.project.name}</a
      >
      {" / "}
      {data.ranking.name}
    </p>
  </div>

  <Tabs.Root value={currentTab} onValueChange={(v) => v && goto(tabHref(v))}>
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
