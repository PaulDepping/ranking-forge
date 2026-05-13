<script lang="ts">
	import { enhance } from '$app/forms';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Alert } from '$lib/components/ui/alert';
	import { PUBLIC_API_URL } from '$env/static/public';
	import type { Game } from '$lib/types';

	let { form } = $props();

	let gameQuery = $state('');
	let gameResults = $state<Game[]>([]);
	let selectedGame = $state<Game | null>(null);
	let searching = $state(false);
	let searchTimeout: ReturnType<typeof setTimeout>;

	function onGameInput(e: Event) {
		const q = (e.target as HTMLInputElement).value;
		gameQuery = q;
		selectedGame = null;
		clearTimeout(searchTimeout);
		if (q.length < 2) { gameResults = []; return; }
		searching = true;
		searchTimeout = setTimeout(async () => {
			const res = await fetch(`${PUBLIC_API_URL}/games?q=${encodeURIComponent(q)}`, { credentials: 'include' });
			gameResults = res.ok ? await res.json() : [];
			searching = false;
		}, 300);
	}

	function selectGame(g: Game) {
		selectedGame = g;
		gameQuery = g.display_name ?? g.name;
		gameResults = [];
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
			<div class="relative">
				<Input
					id="game-search"
					value={gameQuery}
					oninput={onGameInput}
					placeholder="Search start.gg games…"
					autocomplete="off"
				/>
				{#if gameResults.length > 0}
					<ul class="absolute z-10 mt-1 w-full rounded-md border border-border bg-popover shadow-lg">
						{#each gameResults as g (g.id)}
							<li>
								<button
									type="button"
									class="w-full px-3 py-2 text-left text-sm hover:bg-accent"
									onclick={() => selectGame(g)}
								>{g.display_name ?? g.name}</button>
							</li>
						{/each}
					</ul>
				{/if}
			</div>
			{#if searching}
				<p class="text-xs text-muted-foreground">Searching…</p>
			{/if}
		</div>

		<input type="hidden" name="game_id" value={selectedGame?.id ?? ''} />
		<input type="hidden" name="game_name" value={selectedGame ? (selectedGame.display_name ?? selectedGame.name) : ''} />

		<div class="flex gap-2">
			<Button type="submit">Create</Button>
			<Button variant="ghost" href="/projects">Cancel</Button>
		</div>
	</form>
</div>
