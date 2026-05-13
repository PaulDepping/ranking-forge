<script lang="ts">
	import { page } from '$app/state';
	import { Separator } from '$lib/components/ui/separator';

	let { children, data } = $props();

	const tabs = [
		{ label: 'Players', href: 'players' },
		{ label: 'Import', href: 'import' },
		{ label: 'Tournaments', href: 'tournaments' },
		{ label: 'Stats', href: 'stats' },
		{ label: 'H2H', href: 'h2h' }
	];

	function tabHref(slug: string) {
		return `/projects/${data.project.id}/${slug}`;
	}

	function isActive(slug: string) {
		return page.url.pathname.startsWith(`/projects/${data.project.id}/${slug}`);
	}
</script>

<div class="space-y-4">
	<div>
		<a href="/projects" class="text-sm text-muted-foreground hover:text-foreground">← Projects</a>
		<h1 class="mt-1 text-2xl font-bold">{data.project.name}</h1>
		{#if data.project.game_name}
			<p class="text-sm text-muted-foreground">{data.project.game_name}</p>
		{/if}
	</div>

	<nav class="flex gap-1">
		{#each tabs as tab (tab.href)}
			<a
				href={tabHref(tab.href)}
				class="rounded-md px-3 py-1.5 text-sm font-medium transition-colors
					{isActive(tab.href)
						? 'bg-primary text-primary-foreground'
						: 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'}"
			>{tab.label}</a>
		{/each}
	</nav>

	<Separator />

	{@render children()}
</div>
