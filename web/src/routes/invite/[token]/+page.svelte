<script lang="ts">
	import { enhance } from '$app/forms';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';

	let { data, form } = $props();
</script>

<div class="flex min-h-[60vh] items-center justify-center">
	<Card.Root class="w-full max-w-sm">
		<Card.Header>
			<Card.Title>Project invitation</Card.Title>
			<Card.Description>You've been invited to collaborate on a project.</Card.Description>
		</Card.Header>
		<Card.Content class="space-y-4">
			{#if form?.error}
				<p class="text-sm text-destructive">{form.error}</p>
			{/if}
			<form method="POST" action="?/accept" use:enhance>
				<Button type="submit" class="w-full">Accept invitation</Button>
			</form>
			<p class="text-center text-sm text-muted-foreground">
				Don't have an account? <a href="/register" class="underline">Create one</a>
			</p>
		</Card.Content>
	</Card.Root>
</div>
