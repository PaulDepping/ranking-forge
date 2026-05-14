import { render, screen } from '@testing-library/svelte';
import SetDetailModal from './SetDetailModal.svelte';
import type { SetRecord } from '$lib/types';

const baseSet: SetRecord = {
	opponent_id: 'p2',
	opponent_name: 'Bob',
	upset_factor: 2,
	winner_score: 3,
	loser_score: 1,
	tournament_name: 'Genesis 9',
	tournament_slug: 'tournament/genesis-9',
	event_name: 'Melee Singles',
	round_name: 'Winners Finals',
	completed_at: '2024-01-20T18:00:00Z',
	is_dq: false,
	vod_url: null,
	startgg_set_id: 12345,
	winner_seed: 1,
	loser_seed: 12,
};

describe('SetDetailModal', () => {
	it('renders nothing when set is null', () => {
		render(SetDetailModal, {
			props: { set: null, isWin: false, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.queryByText('Genesis 9')).not.toBeInTheDocument();
	});

	it('shows player names in title', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText('Alice vs Bob')).toBeInTheDocument();
	});

	it('shows Win with score from winner perspective', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText(/Win · 3–1/)).toBeInTheDocument();
	});

	it('shows Loss with score from loser perspective', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: false, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText(/Loss · 1–3/)).toBeInTheDocument();
	});

	it('shows tournament, event and round', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText('Genesis 9')).toBeInTheDocument();
		expect(screen.getByText('Melee Singles')).toBeInTheDocument();
		expect(screen.getByText('Winners Finals')).toBeInTheDocument();
	});

	it('shows upset factor as integer', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.getByText('2')).toBeInTheDocument();
	});

	it('shows start.gg link when tournament_slug is present', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		const link = screen.getByRole('link', { name: /View on start\.gg/ });
		expect(link).toHaveAttribute(
			'href',
			'https://www.start.gg/tournament/genesis-9'
		);
	});

	it('hides VOD link when vod_url is null', () => {
		render(SetDetailModal, {
			props: { set: baseSet, isWin: true, currentPlayerName: 'Alice', onClose: () => {} }
		});
		expect(screen.queryByRole('link', { name: /Watch VOD/ })).not.toBeInTheDocument();
	});

	it('shows VOD link when vod_url is present', () => {
		render(SetDetailModal, {
			props: {
				set: { ...baseSet, vod_url: 'https://youtube.com/watch?v=abc' },
				isWin: true,
				currentPlayerName: 'Alice',
				onClose: () => {}
			}
		});
		const link = screen.getByRole('link', { name: /Watch VOD/ });
		expect(link).toHaveAttribute('href', 'https://youtube.com/watch?v=abc');
	});
});
