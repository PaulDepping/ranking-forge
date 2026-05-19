<script lang="ts">
	import { enhance } from '$app/forms';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import * as Card from '$lib/components/ui/card';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';

	let { data, form } = $props();
</script>

<div class="mx-auto max-w-2xl space-y-6">
	<h1 class="text-2xl font-semibold">Account settings</h1>

	<!-- Profile card -->
	<Card.Root>
		<Card.Header>
			<Card.Title>Profile</Card.Title>
			<Card.Description>Update your display name and email address.</Card.Description>
		</Card.Header>
		<form method="POST" action="?/updateProfile" use:enhance>
			<Card.Content class="space-y-4">
				{#if form?.profileError}
					<p class="text-sm text-destructive">{form.profileError}</p>
				{/if}
				{#if form?.profileSuccess}
					<p class="text-sm text-green-600">Profile updated.</p>
				{/if}
				<div class="space-y-2">
					<Label for="display_name">Display name</Label>
					<Input
						id="display_name"
						name="display_name"
						value={data.user.display_name}
						maxlength={50}
						autocomplete="nickname"
					/>
				</div>
				<div class="space-y-2">
					<Label for="email">Email</Label>
					<Input
						id="email"
						name="email"
						type="email"
						value={data.user.email}
						autocomplete="email"
					/>
				</div>
			</Card.Content>
			<Card.Footer class="flex justify-end">
				<Button type="submit">Save changes</Button>
			</Card.Footer>
		</form>
	</Card.Root>

	<!-- Password card -->
	<Card.Root>
		<Card.Header>
			<Card.Title>Password</Card.Title>
			<Card.Description>Use a strong, unique password.</Card.Description>
		</Card.Header>
		<form method="POST" action="?/updatePassword" use:enhance>
			<Card.Content class="space-y-4">
				{#if form?.passwordError}
					<p class="text-sm text-destructive">{form.passwordError}</p>
				{/if}
				{#if form?.passwordSuccess}
					<p class="text-sm text-green-600">Password changed.</p>
				{/if}
				<div class="space-y-2">
					<Label for="current_password">Current password</Label>
					<Input
						id="current_password"
						name="current_password"
						type="password"
						required
						autocomplete="current-password"
					/>
				</div>
				<div class="space-y-2">
					<Label for="new_password">New password</Label>
					<Input
						id="new_password"
						name="new_password"
						type="password"
						required
						minlength={8}
						maxlength={128}
						autocomplete="new-password"
					/>
				</div>
				<div class="space-y-2">
					<Label for="confirm_password">Confirm new password</Label>
					<Input
						id="confirm_password"
						name="confirm_password"
						type="password"
						required
						minlength={8}
						maxlength={128}
						autocomplete="new-password"
					/>
				</div>
			</Card.Content>
			<Card.Footer class="flex justify-end">
				<Button type="submit">Change password</Button>
			</Card.Footer>
		</form>
	</Card.Root>

	<!-- Delete account card -->
	<Card.Root class="border-destructive">
		<Card.Header>
			<Card.Title class="text-destructive">Delete account</Card.Title>
			<Card.Description>
				Permanently deletes your account and all projects you own. This cannot be undone.
			</Card.Description>
		</Card.Header>
		<Card.Footer>
			{#if form?.deleteError}
				<p class="text-sm text-destructive mr-auto">{form.deleteError}</p>
			{/if}
			<AlertDialog.Root>
				<AlertDialog.Trigger>
					{#snippet child({ props })}
						<Button variant="destructive" {...props}>Delete account</Button>
					{/snippet}
				</AlertDialog.Trigger>
				<AlertDialog.Content>
					<AlertDialog.Header>
						<AlertDialog.Title>Are you absolutely sure?</AlertDialog.Title>
						<AlertDialog.Description>
							This will permanently delete your account, all your projects, and all associated data.
							This action cannot be undone.
						</AlertDialog.Description>
					</AlertDialog.Header>
					<AlertDialog.Footer>
						<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
						<form method="POST" action="?/deleteAccount">
							<AlertDialog.Action type="submit" class="bg-destructive text-destructive-foreground hover:bg-destructive/90">
								Delete account
							</AlertDialog.Action>
						</form>
					</AlertDialog.Footer>
				</AlertDialog.Content>
			</AlertDialog.Root>
		</Card.Footer>
	</Card.Root>
</div>
