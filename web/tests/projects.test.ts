import { test as base, expect } from '@playwright/test';

// Extend base test with a pre-authenticated browser context
const test = base.extend({
	page: async ({ page }, use) => {
		await page.context().addCookies([
			{
				name: 'session_id',
				value: 'test-session',
				domain: 'localhost',
				path: '/'
			}
		]);
		await use(page);
	}
});

test('projects list page shows imported projects', async ({ page }) => {
	await page.goto('/projects');
	await expect(page.getByText('SSBM Power Ranking')).toBeVisible();
	await expect(page.getByText('Super Smash Bros. Melee')).toBeVisible();
});

test('projects list has a link to create a new project', async ({ page }) => {
	await page.goto('/projects');
	await expect(page.getByRole('link', { name: 'New project' })).toBeVisible();
});

test('project layout shows tab navigation', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await expect(page.getByRole('link', { name: 'Players' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Import' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Tournaments' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Stats' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'H2H' })).toBeVisible();
});

test('h2h page renders the player grid', async ({ page }) => {
	await page.goto('/projects/proj-1/h2h');
	// Player names appear in headers and row labels
	await expect(page.getByText('Alice').first()).toBeVisible();
	await expect(page.getByText('Bob').first()).toBeVisible();
	// At least one W–L record from mock data
	await expect(page.getByText('3–1')).toBeVisible();
	await expect(page.getByText("Row player's record vs. column player")).toBeVisible();
});

test('stats page renders player rankings', async ({ page }) => {
	await page.goto('/projects/proj-1/stats');
	await expect(page.getByText('Alice')).toBeVisible();
	await expect(page.getByText('Bob')).toBeVisible();
	// Alice's aggregate upset factor from mock data = 2.0
	await expect(page.getByText('2.0')).toBeVisible();
});

test('import page shows trigger button', async ({ page }) => {
	await page.goto('/projects/proj-1/import');
	await expect(page.getByRole('button', { name: 'Start import' })).toBeVisible();
});

test('tournaments page shows empty state before import', async ({ page }) => {
	await page.goto('/projects/proj-1/tournaments');
	await expect(
		page.getByText('No tournaments imported yet. Run an import first.')
	).toBeVisible();
});
