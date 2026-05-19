<script lang="ts">
	import { buttonVariants } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';

	let { data } = $props();

	const features = [
		{
			title: 'Import from start.gg',
			description: 'Fetch your tournament history automatically — just provide the player slugs.'
		},
		{
			title: 'Curate your events',
			description:
				"Manually exclude tournaments that shouldn't count. You stay in control of what goes into the ranking."
		},
		{
			title: 'Stats at a glance',
			description:
				"Per-player win/loss breakdowns and head-to-head tables, ready to reference when you're building your list."
		},
		{
			title: 'Collaborate with your panel',
			description:
				'Invite other panelists to work on a ranking together — multiple people can contribute to the same project.'
		}
	];

	const steps = [
		{
			title: 'Create a project',
			description:
				'Add the players you want to rank using their start.gg slugs. Invite your fellow panelists to collaborate.'
		},
		{
			title: 'Import & curate',
			description:
				"We fetch the tournament history automatically. Deselect any events that shouldn't count toward the ranking."
		},
		{
			title: 'Build your ranking',
			description:
				"Use the win/loss breakdowns and head-to-head tables to inform your panel's decisions."
		}
	];
</script>

<!-- Hero -->
<section class="py-20 text-center">
	<h1 class="text-4xl font-bold tracking-tight sm:text-5xl">
		The data behind your power rankings.
	</h1>
	<p class="mx-auto mt-4 max-w-2xl text-lg text-muted-foreground">
		Pull your tournament data from start.gg, curate which events count, and get the stats to back
		up your ranking decisions.
	</p>
	<div class="mt-8 flex justify-center gap-3">
		{#if data.user}
			<a href="/projects" class={buttonVariants()}>Go to your projects</a>
		{:else}
			<a href="/register" class={buttonVariants()}>Get started</a>
			<a href="/login" class={buttonVariants({ variant: 'outline' })}>Sign in</a>
		{/if}
	</div>
</section>

<!-- Features -->
<section class="py-12">
	<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
		{#each features as feature}
			<Card.Root>
				<Card.Header>
					<Card.Title>{feature.title}</Card.Title>
				</Card.Header>
				<Card.Content>
					<p class="text-muted-foreground">{feature.description}</p>
				</Card.Content>
			</Card.Root>
		{/each}
	</div>
</section>

<!-- How it works -->
<section class="py-12">
	<h2 class="mb-8 text-center text-2xl font-bold">How it works</h2>
	<div class="grid grid-cols-1 gap-8 md:grid-cols-3">
		{#each steps as step, i}
			<div class="text-center">
				<Badge class="mb-3">{i + 1}</Badge>
				<h3 class="mb-2 font-semibold">{step.title}</h3>
				<p class="text-sm text-muted-foreground">{step.description}</p>
			</div>
		{/each}
	</div>
</section>

<!-- Footer -->
<Separator class="mt-12" />
<footer class="py-6 text-center text-sm text-muted-foreground">
	Created by King ·
	<a
		href="https://github.com/PaulDepping/ranking-forge"
		class="underline hover:text-foreground"
		target="_blank"
		rel="noopener noreferrer">Source on GitHub</a
	> ·
	<a
		href="https://www.gnu.org/licenses/agpl-3.0.html"
		class="underline hover:text-foreground"
		target="_blank"
		rel="noopener noreferrer">Open source under AGPL v3</a
	>
</footer>
