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
	await expect(page.getByRole('tab', { name: 'Players' })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'Import' })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'Tournaments' })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'Stats' })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'H2H' })).toBeVisible();
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
	// Player names appear as card headers (and also inside set rows, so use first())
	await expect(page.getByText('Alice').first()).toBeVisible();
	await expect(page.getByText('Bob').first()).toBeVisible();
	// W/L summary visible in Alice's card (1 win, 1 loss)
	await expect(page.getByText('W 1 · L 1 · 50%')).toBeVisible();
	// Set row with integer UF
	await expect(page.getByRole('button', { name: /Bob · UF 2/ }).first()).toBeVisible();
});

test('import page shows trigger button', async ({ page }) => {
	await page.goto('/projects/proj-1/import');
	await expect(page.getByRole('button', { name: 'Start import' })).toBeVisible();
});

test('tournaments page shows empty state before import', async ({ page }) => {
	await page.goto('/projects/proj-1/tournaments');
	await expect(page.getByText('No tournaments yet')).toBeVisible();
	await expect(
		page.getByText('Run an import to pull in tournaments from start.gg.')
	).toBeVisible();
});

test('import page shows retry button when last import failed', async ({ page }) => {
	await page.goto('/projects/proj-failed/import');
	await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
	// Main form submit button should not be "Retry" — it should say Re-import
	await expect(page.getByRole('button', { name: 'Re-import' })).toBeVisible();
});

test('retry button transitions import status to pending', async ({ page }) => {
	await page.goto('/projects/proj-failed/import');
	await page.waitForLoadState('networkidle');
	await page.getByRole('button', { name: 'Retry' }).click();
	await expect(page.getByText('pending')).toBeVisible();
});

test('tournaments filter panel has Clear filters button that resets search', async ({ page }) => {
	await page.goto('/projects/proj-tournaments/tournaments');
	await page.waitForLoadState('networkidle');
	// Open filter panel
	await page.getByRole('button', { name: /Filters & Actions/ }).click();
	// Type in the search box
	await page.getByPlaceholder('Search tournament or event name…').fill('melee');
	await expect(page.getByPlaceholder('Search tournament or event name…')).toHaveValue('melee');
	// Click "Clear filters"
	await page.getByRole('button', { name: 'Clear filters' }).click();
	// Search should be cleared
	await expect(page.getByPlaceholder('Search tournament or event name…')).toHaveValue('');
});

test('players page shows Add players button and no inline name form', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await expect(page.getByRole('button', { name: 'Add players' })).toBeVisible();
	await expect(page.getByPlaceholder('Player name')).not.toBeVisible();
});

test('Add players dialog opens with three tabs', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await page.getByRole('button', { name: 'Add players' }).click();
	await expect(page.getByRole('tab', { name: 'From tournament' })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'By handle' })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'By name' })).toBeVisible();
});

test('By name tab adds a player and clears the input', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await page.getByRole('button', { name: 'Add players' }).click();
	await page.getByRole('tab', { name: 'By name' }).click();
	await page.getByLabel('Player name').fill('TestPlayer');
	await page.getByRole('button', { name: 'Add player' }).click();
	await expect(page.getByLabel('Player name')).toHaveValue('');
});

test('player row has Edit button; clicking it shows inline input', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await page.getByRole('button', { name: 'Edit' }).first().click();
	await expect(page.getByRole('textbox').first()).toBeVisible();
	await page.getByRole('button', { name: 'Cancel' }).click();
	await expect(page.getByText('Alice').first()).toBeVisible();
});
