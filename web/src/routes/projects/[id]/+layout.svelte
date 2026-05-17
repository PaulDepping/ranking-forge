<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { Separator } from '$lib/components/ui/separator';
	import * as Tabs from '$lib/components/ui/tabs';

	let { children, data } = $props();

	const tabs = [
		{ label: 'Players', href: 'players' },
		{ label: 'Import', href: 'import' },
		{ label: 'Tournaments', href: 'tournaments' },
		{ label: 'Stats', href: 'stats' },
		{ label: 'H2H', href: 'h2h' },
		{ label: 'Settings', href: 'settings' }
	];

	function tabHref(slug: string) {
		return `/projects/${data.project.id}/${slug}`;
	}

	const currentTab = $derived(
		tabs.find(t => page.url.pathname.startsWith(tabHref(t.href)))?.href ?? tabs[0].href
	);
</script>

<div class="space-y-4">
	<div>
		<a href="/projects" class="text-sm text-muted-foreground hover:text-foreground">← Projects</a>
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

	<Separator />

	{@render children()}
</div>
