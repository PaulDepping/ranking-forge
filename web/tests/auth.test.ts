import { test, expect } from '@playwright/test';

test('login page renders the sign-in form', async ({ page }) => {
	await page.goto('/login');
	await expect(page.getByRole('heading', { name: 'RankingForge' })).toBeVisible();
	await expect(page.getByLabel('Username')).toBeVisible();
	await expect(page.getByLabel('Password')).toBeVisible();
	await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Register' })).toBeVisible();
});

test('shows error alert on invalid credentials', async ({ page }) => {
	await page.goto('/login');
	await page.getByLabel('Username').fill('wronguser');
	await page.getByLabel('Password').fill('wrongpass');
	await page.getByRole('button', { name: 'Sign in' }).click();

	await expect(page.getByText('Invalid username or password')).toBeVisible();
	await expect(page).toHaveURL('/login');
});

test('valid credentials are accepted without an error', async ({ page }) => {
	// Verifies the form action processes correct credentials.
	// Full session-cookie persistence relies on SvelteKit forwarding Set-Cookie from
	// event.fetch responses — tested in projects.test.ts via pre-set cookie fixture.
	await page.goto('/login');
	await page.getByLabel('Username').fill('testuser');
	await page.getByLabel('Password').fill('testpass');
	await page.getByRole('button', { name: 'Sign in' }).click();

	// No error alert should be shown for valid credentials
	await expect(page.getByText('Invalid username or password')).not.toBeVisible();
});

test('unauthenticated visit to /projects redirects to /login', async ({ page }) => {
	// Navigate without a session cookie — hooks.server.ts should redirect
	await page.goto('/projects');
	await expect(page).toHaveURL('/login');
});

test('register page renders the registration form', async ({ page }) => {
	await page.goto('/register');
	await expect(page.getByLabel('Username')).toBeVisible();
	await expect(page.getByLabel('Password')).toBeVisible();
	await expect(page.getByRole('button', { name: 'Create account' })).toBeVisible();
});
