import { render, screen } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import Page from './+page.svelte';

const players = [
	{ id: 'p1', name: 'Alice', project_id: 'proj-1', created_at: '2026-01-01T00:00:00Z', accounts: [] },
	{ id: 'p2', name: 'Bob', project_id: 'proj-1', created_at: '2026-01-01T00:00:00Z', accounts: [] },
	{ id: 'p3', name: 'Charlie', project_id: 'proj-1', created_at: '2026-01-01T00:00:00Z', accounts: [] }
];

const h2h = [
	{ player_id: 'p1', opponent_id: 'p2', wins: 3, losses: 1 },
	{ player_id: 'p2', opponent_id: 'p1', wins: 1, losses: 3 },
	{ player_id: 'p1', opponent_id: 'p3', wins: 2, losses: 0 },
	{ player_id: 'p3', opponent_id: 'p1', wins: 0, losses: 2 },
	{ player_id: 'p2', opponent_id: 'p3', wins: 2, losses: 1 },
	{ player_id: 'p3', opponent_id: 'p2', wins: 1, losses: 2 }
];

describe('H2H page', () => {
	it('renders player names in header row', () => {
		render(Page, { data: { players, h2h } });
		expect(screen.getAllByText('Alice').length).toBeGreaterThan(0);
		expect(screen.getAllByText('Bob').length).toBeGreaterThan(0);
		expect(screen.getAllByText('Charlie').length).toBeGreaterThan(0);
	});

	it('renders win–loss records between players', () => {
		render(Page, { data: { players, h2h } });
		// Alice vs Bob: 3–1 (and Bob vs Alice: 1–3)
		expect(screen.getByText('3–1')).toBeInTheDocument();
		expect(screen.getByText('1–3')).toBeInTheDocument();
		// Alice vs Charlie: 2–0
		expect(screen.getByText('2–0')).toBeInTheDocument();
		expect(screen.getByText('0–2')).toBeInTheDocument();
	});

	it('shows empty message when h2h data is absent', () => {
		render(Page, { data: { players: players.slice(0, 1), h2h: [] } });
		expect(
			screen.getByText('No head-to-head data yet. Import tournaments first.')
		).toBeInTheDocument();
		expect(screen.queryByRole('table')).not.toBeInTheDocument();
	});

	it('shows table footer note when data is present', () => {
		render(Page, { data: { players, h2h } });
		expect(screen.getByText("Row player's record vs. column player")).toBeInTheDocument();
	});

	it('renders a dash for same-player diagonal cells', () => {
		render(Page, { data: { players, h2h } });
		const dashCells = screen.getAllByText('—');
		// One dash per player (3 players → 3 diagonal cells)
		expect(dashCells.length).toBe(players.length);
	});
});
