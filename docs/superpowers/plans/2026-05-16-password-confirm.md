# Password Confirmation on Registration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Confirm password" field to the registration form; the server action rejects submissions where the two passwords differ.

**Architecture:** Server-action validation only — the SvelteKit action reads both `password` and `confirm_password` from the form body and returns `fail(400, { error: 'Passwords do not match' })` before calling the API. No changes to the backend.

**Tech Stack:** SvelteKit 5 (runes), TypeScript, shadcn-svelte `Input`/`Label`, Playwright (e2e tests)

---

### Task 1: Write failing e2e tests

**Files:**
- Modify: `web/tests/auth.test.ts`

- [ ] **Step 1: Append three new tests to `web/tests/auth.test.ts`**

```typescript
test('register page shows Confirm password field', async ({ page }) => {
	await page.goto('/register');
	await expect(page.getByLabel('Confirm password')).toBeVisible();
});

test('shows error when passwords do not match', async ({ page }) => {
	await page.goto('/register');
	await page.getByLabel('Username').fill('newuser');
	await page.getByLabel('Password').fill('password123');
	await page.getByLabel('Confirm password').fill('different123');
	await page.getByRole('button', { name: 'Create account' }).click();
	await expect(page.getByText('Passwords do not match')).toBeVisible();
	await expect(page).toHaveURL('/register');
});

test('registration succeeds when passwords match', async ({ page }) => {
	await page.goto('/register');
	await page.getByLabel('Username').fill('newuser');
	await page.getByLabel('Password').fill('password123');
	await page.getByLabel('Confirm password').fill('password123');
	await page.getByRole('button', { name: 'Create account' }).click();
	await expect(page.getByText('Passwords do not match')).not.toBeVisible();
});
```

- [ ] **Step 2: Run the new tests to confirm they fail**

```bash
cd web && npm run test:e2e -- --grep "register"
```

Expected: 2–3 failures — "Confirm password field not found", mismatch error not shown.

---

### Task 2: Add the Confirm password field to the form

**Files:**
- Modify: `web/src/routes/register/+page.svelte`

- [ ] **Step 1: Add the Confirm password field below the existing password field**

In `web/src/routes/register/+page.svelte`, replace:

```svelte
			<div class="space-y-2">
				<Label for="password">Password</Label>
				<Input id="password" name="password" type="password" required minlength={8} autocomplete="new-password" />
			</div>
			<Button type="submit" class="w-full">Create account</Button>
```

with:

```svelte
			<div class="space-y-2">
				<Label for="password">Password</Label>
				<Input id="password" name="password" type="password" required minlength={8} autocomplete="new-password" />
			</div>
			<div class="space-y-2">
				<Label for="confirm_password">Confirm password</Label>
				<Input id="confirm_password" name="confirm_password" type="password" required minlength={8} autocomplete="new-password" />
			</div>
			<Button type="submit" class="w-full">Create account</Button>
```

- [ ] **Step 2: Run the "Confirm password field" test to confirm it now passes**

```bash
cd web && npm run test:e2e -- --grep "Confirm password field"
```

Expected: PASS.

---

### Task 3: Add server-side mismatch validation

**Files:**
- Modify: `web/src/routes/register/+page.server.ts`

- [ ] **Step 1: Read `confirm_password` and return early if passwords differ**

In `web/src/routes/register/+page.server.ts`, replace:

```typescript
		const data = await request.formData();
		const username = data.get('username') as string;
		const password = data.get('password') as string;

		const res = await fetch(`${INTERNAL_API_URL}/auth/register`, {
```

with:

```typescript
		const data = await request.formData();
		const username = data.get('username') as string;
		const password = data.get('password') as string;
		const confirmPassword = data.get('confirm_password') as string;

		if (password !== confirmPassword) {
			return fail(400, { error: 'Passwords do not match' });
		}

		const res = await fetch(`${INTERNAL_API_URL}/auth/register`, {
```

(`fail` is already imported at the top of the file.)

- [ ] **Step 2: Run all register-related tests**

```bash
cd web && npm run test:e2e -- --grep "register"
```

Expected: all 5 register tests PASS.

- [ ] **Step 3: Run the full e2e suite to check for regressions**

```bash
cd web && npm run test:e2e
```

Expected: all tests PASS.

---

### Task 4: Commit

- [ ] **Step 1: Commit all changes**

```bash
git add web/src/routes/register/+page.svelte web/src/routes/register/+page.server.ts web/tests/auth.test.ts
git commit -m "feat(web): require password confirmation on registration"
```
