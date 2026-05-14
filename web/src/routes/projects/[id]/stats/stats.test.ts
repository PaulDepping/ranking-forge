import { render, screen, fireEvent } from '@testing-library/svelte';
import Page from './+page.svelte';
import type { SetRecord } from '$lib/types';

const user = { id: 'u1', username: 'testuser', created_at: '2026-01-01T00:00:00Z' };
const project = { id: 'proj-1', name: 'Test Project', game_id: null, game_name: null, created_at: '2026-01-01T00:00:00Z' };

function makeSet(opponentName: string, uf: number): SetRecord {
	return {
		opponent_id: 'opp',
		opponent_name: opponentName,
		upset_factor: uf,
		winner_score: null,
		loser_score: null,
		tournament_name: 'Test Tournament',
		tournament_slug: 'tournament/test',
		event_name: 'Melee Singles',
		round_name: 'Round 1',
		completed_at: null,
		is_dq: false,
		vod_url: null,
		startgg_set_id: 1,
		winner_seed: null,
		loser_seed: null,
	};
}

const stats = [
	{
		player_id: 'p1',
		name: 'Alice',
		wins: [makeSet('Bob', 2), makeSet('Charlie', 1)],
		losses: [makeSet('Charlie', 0)],
	},
	{
		player_id: 'p2',
		name: 'Bob',
		wins: [makeSet('Charlie', 3)],
		losses: [makeSet('Alice', 2)],
	},
	{
		player_id: 'p3',
		name: 'Charlie',
		wins: [],
		losses: [makeSet('Alice', 1)],
	},
];

describe('Stats page', () => {
	it('renders player names', () => {
		render(Page, { data: { user, project, stats } });
		expect(screen.getByText('Alice')).toBeInTheDocument();
		expect(screen.getByText('Bob')).toBeInTheDocument();
		expect(screen.getByText('Charlie')).toBeInTheDocument();
	});

	it('shows W/L/% summary in each card header', () => {
		render(Page, { data: { user, project, stats } });
		// Alice: 2W 1L = 67%, Bob: 1W 1L = 50%, Charlie: 0W 1L = 0%
		expect(screen.getByText('W 2 · L 1 · 67%')).toBeInTheDocument();
		expect(screen.getByText('W 1 · L 1 · 50%')).toBeInTheDocument();
		expect(screen.getByText('W 0 · L 1 · 0%')).toBeInTheDocument();
	});

	it('shows win opponent names with integer UF', () => {
		render(Page, { data: { user, project, stats } });
		expect(screen.getByText(/Bob · UF 2/)).toBeInTheDocument();
		expect(screen.getByText(/Charlie · UF 1/)).toBeInTheDocument();
	});

	it('does not render any decimal UF values', () => {
		const statsWithDecimalUF = [
			{ player_id: 'p1', name: 'Alice', wins: [makeSet('Bob', 2)], losses: [] },
		];
		render(Page, { data: { user, project, stats: statsWithDecimalUF } });
		expect(screen.queryByText(/UF 2\.0/)).not.toBeInTheDocument();
		expect(screen.queryByText(/UF 2\.5/)).not.toBeInTheDocument();
	});

	it('does not show Agg. UF or accumulated upset factor', () => {
		render(Page, { data: { user, project, stats } });
		expect(screen.queryByText(/Agg\./i)).not.toBeInTheDocument();
		expect(screen.queryByText(/accumulated/i)).not.toBeInTheDocument();
	});

	it('shows empty state when stats is empty', () => {
		render(Page, { data: { user, project, stats: [] } });
		expect(
			screen.getByText('No stats yet. Import tournaments and include some events first.')
		).toBeInTheDocument();
		expect(screen.queryByRole('table')).not.toBeInTheDocument();
	});

	it('opens set detail modal when a win row is clicked', async () => {
		render(Page, { data: { user, project, stats } });
		const bobRow = screen.getByRole('button', { name: /Bob · UF 2/ });
		await fireEvent.click(bobRow);
		expect(screen.getByText('Alice vs Bob')).toBeInTheDocument();
		expect(screen.getByText(/Win/)).toBeInTheDocument();
	});

	it('opens set detail modal when a loss row is clicked', async () => {
		render(Page, { data: { user, project, stats } });
		// Alice's losses list contains "Charlie · UF 0"
		const lossRow = screen.getByRole('button', { name: /Charlie · UF 0/ });
		await fireEvent.click(lossRow);
		expect(screen.getByText('Alice vs Charlie')).toBeInTheDocument();
		expect(screen.getByText(/Loss/)).toBeInTheDocument();
	});
});
