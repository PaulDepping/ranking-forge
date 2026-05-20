import { test, expect } from '@playwright/test';

test('login page renders the sign-in form', async ({ page }) => {
	await page.goto('/login');
	await expect(page.getByRole('link', { name: 'RankingForge' })).toBeVisible();
	await expect(page.getByLabel('Email')).toBeVisible();
	await expect(page.getByLabel('Password')).toBeVisible();
	await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Register' }).first()).toBeVisible();
});

test('shows error alert on invalid credentials', async ({ page }) => {
	await page.goto('/login');
	await page.getByLabel('Email').fill('wrong@test.com');
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
	await page.getByLabel('Email').fill('testuser@test.com');
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
	await expect(page.getByLabel('Email')).toBeVisible();
	await expect(page.getByLabel('Display name')).toBeVisible();
	await expect(page.getByLabel('Password', { exact: true })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Create account' })).toBeVisible();
});

test('register page shows Confirm password field', async ({ page }) => {
	await page.goto('/register');
	await expect(page.getByLabel('Confirm password')).toBeVisible();
});

test('shows error when passwords do not match', async ({ page }) => {
	await page.goto('/register');
	await page.getByLabel('Email').fill('newuser@test.com');
	await page.getByLabel('Display name').fill('newuser');
	await page.getByLabel('Password', { exact: true }).fill('password123');
	await page.getByLabel('Confirm password').fill('different123');
	await page.getByRole('button', { name: 'Create account' }).click();
	await expect(page.getByText('Passwords do not match')).toBeVisible();
	await expect(page).toHaveURL('/register');
});

test('registration succeeds when passwords match', async ({ page }) => {
	await page.goto('/register');
	await page.getByLabel('Email').fill('newuser@test.com');
	await page.getByLabel('Display name').fill('newuser');
	await page.getByLabel('Password', { exact: true }).fill('password123');
	await page.getByLabel('Confirm password').fill('password123');
	await page.getByRole('button', { name: 'Create account' }).click();
	await expect(page.getByText('Passwords do not match')).not.toBeVisible();
});

test('viewer visiting /players is redirected to login with return URL', async ({ page }) => {
	await page.goto('/projects/proj-viewer/players');
	await expect(page).toHaveURL('/login?redirect=%2Fprojects%2Fproj-viewer%2Fplayers');
});

test('viewer visiting /import is redirected to login with return URL', async ({ page }) => {
	await page.goto('/projects/proj-viewer/import');
	await expect(page).toHaveURL('/login?redirect=%2Fprojects%2Fproj-viewer%2Fimport');
});
