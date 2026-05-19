import { test, expect } from '@playwright/test';

const authedTest = test.extend({
	page: async ({ page }, use) => {
		await page.context().addCookies([
			{ name: 'session_id', value: 'test-session', domain: 'localhost', path: '/' }
		]);
		await use(page);
	}
});

// --- Unauthenticated ---

test('landing page renders without redirect', async ({ page }) => {
	await page.goto('/');
	await expect(page).toHaveURL('/');
});

test('landing page shows hero heading', async ({ page }) => {
	await page.goto('/');
	await expect(
		page.getByRole('heading', { name: 'The data behind your power rankings.' })
	).toBeVisible();
});

test('landing page shows Get started and Sign in CTAs when logged out', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Get started' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Sign in' }).first()).toBeVisible();
});

test('header shows Sign in and Register nav buttons when logged out', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Register' }).first()).toBeVisible();
});

test('header does not show Projects link when logged out', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Projects', exact: true })).not.toBeVisible();
});

test('landing page shows all four feature cards', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByText('Import from start.gg')).toBeVisible();
	await expect(page.getByText('Curate your events')).toBeVisible();
	await expect(page.getByText('Stats at a glance')).toBeVisible();
	await expect(page.getByText('Collaborate with your panel')).toBeVisible();
});

test('landing page shows How it works section', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'How it works' })).toBeVisible();
	await expect(page.getByText('Create a project')).toBeVisible();
	await expect(page.getByText('Import & curate')).toBeVisible();
	await expect(page.getByText('Build your ranking')).toBeVisible();
});

test('landing page shows footer with creator and GitHub link', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByText('Created by King')).toBeVisible();
	await expect(page.getByRole('link', { name: 'Source on GitHub' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Open source under AGPL v3' })).toBeVisible();
});

test('header always shows RankingForge brand link', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'RankingForge' })).toBeVisible();
});

// --- Authenticated ---

authedTest('landing page shows Go to your projects CTA when logged in', async ({ page }) => {
	await page.goto('/');
	await expect(page).toHaveURL('/');
	await expect(page.getByRole('link', { name: 'Go to your projects' })).toBeVisible();
});

authedTest('landing page does not show Get started CTA when logged in', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Get started' })).not.toBeVisible();
});

authedTest('header shows Projects link when logged in', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Projects', exact: true })).toBeVisible();
});

authedTest('header shows username when logged in', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByText('testuser')).toBeVisible();
});
