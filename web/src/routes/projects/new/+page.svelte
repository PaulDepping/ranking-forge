<script lang="ts">
	import { enhance } from '$app/forms';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Alert } from '$lib/components/ui/alert';
	import * as Popover from '$lib/components/ui/popover';
	import * as Command from '$lib/components/ui/command';
	import { PUBLIC_API_URL } from '$env/static/public';
	import type { Game } from '$lib/types';

	let { form } = $props();

	let gameQuery = $state('');
	let gameResults = $state<Game[]>([]);
	let selectedGame = $state<Game | null>(null);
	let searching = $state(false);
	let gameSearchOpen = $state(false);
	let searchTimeout: ReturnType<typeof setTimeout>;

	function onCommandInput(value: string) {
		gameQuery = value;
		selectedGame = null;
		clearTimeout(searchTimeout);
		if (value.length < 2) { gameResults = []; return; }
		searching = true;
		searchTimeout = setTimeout(async () => {
			const res = await fetch(`${PUBLIC_API_URL}/games?q=${encodeURIComponent(value)}`, { credentials: 'include' });
			gameResults = res.ok ? await res.json() : [];
			searching = false;
		}, 300);
	}

	function selectGame(g: Game) {
		selectedGame = g;
		gameQuery = g.display_name ?? g.name;
		gameResults = [];
		gameSearchOpen = false;
	}
</script>

<div class="max-w-md space-y-6">
	<h1 class="text-2xl font-bold">New project</h1>

	{#if form?.error}
		<Alert variant="destructive">{form.error}</Alert>
	{/if}

	<form method="POST" use:enhance class="space-y-4">
		<div class="space-y-2">
			<Label for="name">Project name</Label>
			<Input id="name" name="name" required placeholder="e.g. NY Smash PR 2025" />
		</div>

		<div class="space-y-2">
			<Label for="game-search">Game (optional)</Label>
			<Popover.Root bind:open={gameSearchOpen}>
				<Popover.Trigger class="w-full flex h-9 items-center justify-start rounded-md border border-input bg-transparent px-3 text-sm text-left">
					{#if selectedGame}
						{selectedGame.display_name ?? selectedGame.name}
					{:else}
						<span class="text-muted-foreground">Search start.gg games…</span>
					{/if}
				</Popover.Trigger>
				<Popover.Content class="p-0 w-80" align="start">
					<Command.Root shouldFilter={false}>
						<Command.Input
							placeholder="Search start.gg games…"
							value={gameQuery}
							oninput={(e) => onCommandInput((e.target as HTMLInputElement).value)}
						/>
						<Command.List>
							{#if searching}
								<Command.Empty>Searching…</Command.Empty>
							{:else if gameQuery.length >= 2 && gameResults.length === 0}
								<Command.Empty>No games found.</Command.Empty>
							{:else}
								{#each gameResults as g (g.id)}
									<Command.Item
										value={g.id.toString()}
										onSelect={() => selectGame(g)}
									>
										{g.display_name ?? g.name}
									</Command.Item>
								{/each}
							{/if}
						</Command.List>
					</Command.Root>
				</Popover.Content>
			</Popover.Root>
		</div>

		<input type="hidden" name="game_id" value={selectedGame?.id ?? ''} />
		<input type="hidden" name="game_name" value={selectedGame ? (selectedGame.display_name ?? selectedGame.name) : ''} />

		<div class="flex gap-2">
			<Button type="submit">Create</Button>
			<Button variant="ghost" href="/projects">Cancel</Button>
		</div>
	</form>
</div>
