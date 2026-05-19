<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { env } from '$env/dynamic/public';
	import { afterNavigate, goto } from '$app/navigation';
	import { ModeWatcher } from 'mode-watcher';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import { Button, buttonVariants } from '$lib/components/ui/button';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import * as NavigationMenu from '$lib/components/ui/navigation-menu';
	import { navigationMenuTriggerStyle } from '$lib/components/ui/navigation-menu/navigation-menu-trigger.svelte';
	import { previousPage } from '$lib/stores/navigation';

	let { children, data } = $props();

	afterNavigate((navigation) => {
		previousPage.set(
			navigation.from ? navigation.from.url.pathname + navigation.from.url.search : null
		);
	});

	async function logout() {
		await fetch(`${env.PUBLIC_API_URL}/auth/logout`, { method: 'POST', credentials: 'include' });
		await goto('/login');
	}
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<ModeWatcher />

<Tooltip.Provider>
	<header class="border-b border-border bg-card">
		<div class="mx-auto flex max-w-5xl items-center px-4">
			<NavigationMenu.Root>
				<NavigationMenu.List>
					<NavigationMenu.Item>
						<NavigationMenu.Link>
							{#snippet child()}
								<a href="/" class={navigationMenuTriggerStyle()}>
									<span class="font-semibold">RankingForge</span>
								</a>
							{/snippet}
						</NavigationMenu.Link>
					</NavigationMenu.Item>
					{#if data.user}
						<NavigationMenu.Item>
							<NavigationMenu.Link>
								{#snippet child()}
									<a href="/projects" class={navigationMenuTriggerStyle()}>Projects</a>
								{/snippet}
							</NavigationMenu.Link>
						</NavigationMenu.Item>
					{/if}
				</NavigationMenu.List>
			</NavigationMenu.Root>
			<div class="ml-auto flex items-center gap-2">
				<ThemeToggle />
				{#if data.user}
					<a href="/account" class="text-sm text-muted-foreground hover:text-foreground">{data.user.display_name}</a>
					<Button variant="ghost" size="sm" onclick={logout}>Logout</Button>
				{:else}
					<a href="/login" class={buttonVariants({ variant: 'outline', size: 'sm' })}>Sign in</a>
					<a href="/register" class={buttonVariants({ variant: 'default', size: 'sm' })}>Register</a>
				{/if}
			</div>
		</div>
	</header>

	<main class="mx-auto max-w-5xl px-4 py-8">
		{@render children()}
	</main>
</Tooltip.Provider>
