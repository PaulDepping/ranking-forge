<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { Separator } from "$lib/components/ui/separator";
  import * as Tabs from "$lib/components/ui/tabs";

  let { children, data } = $props();

  const allTabs = [
    { label: "Rankings", href: "", minRole: null },
    { label: "Players", href: "players", minRole: "editor" as const },
    { label: "Import", href: "import", minRole: "editor" as const },
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
    if (slug === "") return `/projects/${data.project.id}`;
    return `/projects/${data.project.id}/${slug}`;
  }

  const currentTab = $derived(
    tabs.find((t) => {
      if (t.href === "") {
        return page.url.pathname === `/projects/${data.project.id}`;
      }
      return page.url.pathname.startsWith(tabHref(t.href));
    })?.href ?? tabs[0].href,
  );
</script>

<div class="space-y-4 {page.data.wide ? 'mx-auto max-w-5xl px-4' : ''}">
  <Tabs.Root value={currentTab} onValueChange={(v) => v !== undefined && goto(tabHref(v))}>
    <Tabs.List>
      {#each tabs as tab (tab.href)}
        <Tabs.Trigger value={tab.href}>{tab.label}</Tabs.Trigger>
      {/each}
    </Tabs.List>
  </Tabs.Root>

  <Separator />

  {@render children()}
</div>
