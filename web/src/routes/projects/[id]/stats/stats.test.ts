import { render, screen, fireEvent, within } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import Page from './+page.svelte';

const stats = [
	{
		player_id: 'p1',
		name: 'Alice',
		wins: [
			{ opponent_id: 'p2', opponent_name: 'Bob', upset_factor: 2.0 },
			{ opponent_id: 'p3', opponent_name: 'Charlie', upset_factor: 1.5 }
		],
		losses: [{ opponent_id: 'p3', opponent_name: 'Charlie', upset_factor: 0.5 }]
	},
	{
		player_id: 'p2',
		name: 'Bob',
		wins: [{ opponent_id: 'p3', opponent_name: 'Charlie', upset_factor: 1.0 }],
		losses: [{ opponent_id: 'p1', opponent_name: 'Alice', upset_factor: 2.0 }]
	},
	{
		player_id: 'p3',
		name: 'Charlie',
		wins: [],
		losses: [{ opponent_id: 'p1', opponent_name: 'Alice', upset_factor: 1.5 }]
	}
];

describe('Stats page', () => {
	it('renders player names', () => {
		render(Page, { data: { stats } });
		expect(screen.getByText('Alice')).toBeInTheDocument();
		expect(screen.getByText('Bob')).toBeInTheDocument();
		expect(screen.getByText('Charlie')).toBeInTheDocument();
	});

	it('renders players in the order supplied (server already sorts by aggregate UF)', () => {
		render(Page, { data: { stats } });
		const rows = screen.getAllByRole('row');
		expect(rows[1]).toHaveTextContent('Alice');
		expect(rows[2]).toHaveTextContent('Bob');
		expect(rows[3]).toHaveTextContent('Charlie');
	});

	it('shows aggregate upset factor for each player', () => {
		render(Page, { data: { stats } });
		// Alice: 2.0 + 1.5 = 3.5 | Bob: 1.0 | Charlie: 0.0
		expect(screen.getByText('3.5')).toBeInTheDocument();
		expect(screen.getByText('1.0')).toBeInTheDocument();
		expect(screen.getByText('0.0')).toBeInTheDocument();
	});

	it('shows win and loss count buttons per player row', () => {
		render(Page, { data: { stats } });
		const rows = screen.getAllByRole('row');
		// Alice: 2 wins, 1 loss — uniquely named buttons
		expect(within(rows[1]).getByRole('button', { name: '2' })).toBeInTheDocument();
		expect(within(rows[1]).getByRole('button', { name: '1' })).toBeInTheDocument();
		// Charlie: 0 wins
		expect(within(rows[3]).getByRole('button', { name: '0' })).toBeInTheDocument();
	});

	it('shows empty state when there are no stats', () => {
		render(Page, { data: { stats: [] } });
		expect(
			screen.getByText('No stats yet. Import tournaments and include some events first.')
		).toBeInTheDocument();
		expect(screen.queryByRole('table')).not.toBeInTheDocument();
	});

	it('expands wins list when the wins button is clicked', async () => {
		render(Page, { data: { stats } });
		const aliceRow = screen.getAllByRole('row')[1];
		// Alice has 2 wins — the wins button is labeled "2"
		const winsBtn = within(aliceRow).getByRole('button', { name: '2' });

		await fireEvent.click(winsBtn);

		// UF values only appear in the expanded section, not in any header
		expect(screen.getByText('UF 2.0')).toBeInTheDocument();
		expect(screen.getByText('UF 1.5')).toBeInTheDocument();
	});

	it('collapses the expanded section on a second click', async () => {
		render(Page, { data: { stats } });
		const aliceRow = screen.getAllByRole('row')[1];
		const winsBtn = within(aliceRow).getByRole('button', { name: '2' });

		await fireEvent.click(winsBtn);
		expect(screen.getByText('UF 2.0')).toBeInTheDocument();

		await fireEvent.click(winsBtn);
		expect(screen.queryByText('UF 2.0')).not.toBeInTheDocument();
	});

	it('expands losses list when the losses button is clicked', async () => {
		render(Page, { data: { stats } });
		const aliceRow = screen.getAllByRole('row')[1];
		// Alice has 1 loss — the losses button is labeled "1" (unique within Alice's row)
		const lossesBtn = within(aliceRow).getByRole('button', { name: '1' });

		await fireEvent.click(lossesBtn);

		expect(screen.getByText('UF 0.5')).toBeInTheDocument();
	});
});
