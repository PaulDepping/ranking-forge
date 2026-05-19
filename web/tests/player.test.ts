import { test as base, expect } from '@playwright/test';

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

test('player detail page shows stats and tournament history', async ({ page }) => {
	await page.goto('/projects/proj-1/players/player-1');

	// Player name and summary
	await expect(page.getByRole('heading', { name: 'Alice' })).toBeVisible();
	await expect(page.getByText(/1 W/)).toBeVisible();
	await expect(page.getByText(/1 L/)).toBeVisible();

	// Wins section
	await expect(page.getByText(/Wins \(1\)/i)).toBeVisible();
	await expect(page.getByRole('button', { name: /Bob/ })).toBeVisible();

	// Losses section
	await expect(page.getByText(/Losses \(1\)/i)).toBeVisible();
	await expect(page.getByRole('button', { name: /Charlie/ })).toBeVisible();

	// Tournament history table
	await expect(page.getByText('Tournament history (2)')).toBeVisible();
	await expect(page.getByText('Genesis 9')).toBeVisible();
	await expect(page.getByText('1st')).toBeVisible();
	await expect(page.getByText('CEO 2024')).toBeVisible();
});

test('stats page player name links to detail page', async ({ page }) => {
	await page.goto('/projects/proj-1/stats');
	await page.getByRole('link', { name: 'Alice' }).first().click();
	await expect(page).toHaveURL(/\/projects\/proj-1\/players\/player-1/);
});

test('back button returns to previous in-app page', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await page.getByRole('link', { name: 'Alice' }).click();
	await expect(page).toHaveURL('/projects/proj-1/players/player-1');
	await page.getByRole('link', { name: '← Back' }).click();
	await expect(page).toHaveURL('/projects/proj-1/players');
});

test('back button falls back to players list on direct link', async ({ page }) => {
	await page.goto('/projects/proj-1/players/player-1');
	await expect(page.getByRole('link', { name: '← Back' })).toHaveAttribute(
		'href',
		'/projects/proj-1/players'
	);
	await page.getByRole('link', { name: '← Back' }).click();
	await expect(page).toHaveURL('/projects/proj-1/players');
});
