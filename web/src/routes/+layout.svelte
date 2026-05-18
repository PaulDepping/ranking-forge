<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { goto } from '$app/navigation';
	import { ModeWatcher } from 'mode-watcher';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Tooltip from '$lib/components/ui/tooltip';

	let { children, data } = $props();

	async function logout() {
		await fetch(`${PUBLIC_API_URL}/auth/logout`, { method: 'POST', credentials: 'include' });
		await goto('/login');
	}
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<ModeWatcher />

<Tooltip.Provider>
	{#if data.user}
		<header class="border-b border-border bg-card">
			<div class="mx-auto flex max-w-5xl items-center justify-between px-4 py-3">
				<a href="/projects" class="font-semibold text-foreground hover:text-primary">RankingForge</a>
				<div class="flex items-center gap-4">
					<span class="text-sm text-muted-foreground">{data.user.username}</span>
					<ThemeToggle />
					<Button variant="ghost" size="sm" onclick={logout}>Logout</Button>
				</div>
			</div>
		</header>
	{/if}

	<main class="mx-auto max-w-5xl px-4 py-8">
		{@render children()}
	</main>
</Tooltip.Provider>
