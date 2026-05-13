<script lang="ts">
	import { enhance } from '$app/forms';
	import { Button } from '$lib/components/ui/button';
	import { Card, CardHeader, CardTitle, CardDescription, CardFooter } from '$lib/components/ui/card';

	let { data } = $props();
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<h1 class="text-2xl font-bold">Projects</h1>
		<Button href="/projects/new">New project</Button>
	</div>

	{#if data.projects.length === 0}
		<p class="text-muted-foreground">No projects yet. Create one to get started.</p>
	{:else}
		<div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
			{#each data.projects as project (project.id)}
				<Card>
					<CardHeader>
						<CardTitle>
							<a href="/projects/{project.id}/players" class="hover:underline">{project.name}</a>
						</CardTitle>
						{#if project.game_name}
							<CardDescription>{project.game_name}</CardDescription>
						{/if}
					</CardHeader>
					<CardFooter class="flex items-center justify-between">
						<span class="text-xs text-muted-foreground">
							{new Date(project.created_at).toLocaleDateString()}
						</span>
						<form method="POST" action="?/delete" use:enhance>
							<input type="hidden" name="id" value={project.id} />
							<Button
								type="submit"
								variant="ghost"
								size="sm"
								class="text-destructive hover:text-destructive"
								onclick={(e: MouseEvent) => { if (!confirm('Delete this project?')) e.preventDefault(); }}
							>Delete</Button>
						</form>
					</CardFooter>
				</Card>
			{/each}
		</div>
	{/if}
</div>
