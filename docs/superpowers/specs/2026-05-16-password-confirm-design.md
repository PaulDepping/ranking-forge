# Password Confirmation on Registration

**Date:** 2026-05-16
**Status:** Approved

## Problem

The registration form accepts a password but has no confirmation step. A user who mistypes their password during registration has no way to recover it, since there is no password reset flow.

## Goal

Require the user to type their password twice on the registration form. If the two entries don't match, show an error after submit and do not proceed with registration.

## Approach

Server action validation (Option A). The confirmation field is read in the SvelteKit server action alongside the original password. If they differ, the action returns a `fail(400)` response with a user-facing error message. The API backend (`/auth/register`) is never called in the mismatch case and receives no changes.

## Changes

### `web/src/routes/register/+page.svelte`

Add a "Confirm password" input field immediately below the existing password field:

- `<Label for="confirm_password">Confirm password</Label>`
- `<Input id="confirm_password" name="confirm_password" type="password" required minlength={8} autocomplete="new-password" />`

No changes to error display — the existing `{#if form?.error}` alert already handles this.

### `web/src/routes/register/+page.server.ts`

In the `default` action, after reading `password` from `formData`:

1. Read `confirm_password` from `formData`.
2. If `password !== confirm_password`, return `fail(400, { error: 'Passwords do not match' })`.
3. Proceed with the existing API call only if passwords match.

The `confirm_password` value is used only for comparison and is never forwarded to the API.

## Out of scope

- Real-time / inline validation (intentionally deferred — user wants post-submit feedback only)
- Password strength requirements
- Backend changes
