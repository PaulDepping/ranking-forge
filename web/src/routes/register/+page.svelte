<script lang="ts">
	import { enhance } from '$app/forms';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Alert } from '$lib/components/ui/alert';

	let { form } = $props();
</script>

<div class="flex min-h-[60vh] items-center justify-center">
	<div class="w-full max-w-sm space-y-6">
		<div class="space-y-1 text-center">
			<p class="text-lg font-semibold">Create an account</p>
		</div>

		{#if form?.error}
			<Alert variant="destructive">{form.error}</Alert>
		{/if}

		<form method="POST" use:enhance class="space-y-4">
			<div class="space-y-2">
				<Label for="email">Email</Label>
				<Input id="email" name="email" type="email" required autocomplete="email" />
			</div>
			<div class="space-y-2">
				<Label for="display_name">Display name</Label>
				<Input id="display_name" name="display_name" required maxlength={50} autocomplete="nickname" />
			</div>
			<div class="space-y-2">
				<Label for="password">Password</Label>
				<Input id="password" name="password" type="password" required minlength={8} autocomplete="new-password" />
			</div>
			<div class="space-y-2">
				<Label for="confirm_password">Confirm password</Label>
				<Input id="confirm_password" name="confirm_password" type="password" required minlength={8} autocomplete="new-password" />
			</div>
			<Button type="submit" class="w-full">Create account</Button>
		</form>

		<p class="text-center text-sm text-muted-foreground">
			Already have an account? <a href="/login" class="underline hover:text-foreground">Sign in</a>
		</p>
	</div>
</div>
