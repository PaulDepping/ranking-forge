import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Player, PlayerStats, TournamentAttendance } from '$lib/types';
import { env } from '$env/dynamic/private';

export const load: PageServerLoad = async ({ fetch, params, cookies }) => {
	const api = makeApi(fetch, env.INTERNAL_API_URL, cookies.get('session_id'));

	const [statsRes, tournamentsRes, playersRes] = await Promise.all([
		api.get(`/projects/${params.id}/stats/${params.player_id}`),
		api.get(`/projects/${params.id}/players/${params.player_id}/tournaments`),
		api.get(`/projects/${params.id}/players`)
	]);

	if (!statsRes.ok) {
		if (statsRes.status === 404) {
			error(404, { message: 'not_found' });
		}
		error(statsRes.status, { message: 'error' });
	}

	const stats: PlayerStats = await statsRes.json();

	if (!tournamentsRes.ok) {
		error(tournamentsRes.status, 'Failed to load tournament history');
	}

	const tournaments: TournamentAttendance[] = await tournamentsRes.json();

	const players: Player[] = playersRes.ok ? await playersRes.json() : [];
	const trackedPlayerIds = new Set(players.map((p) => p.id));

	return { stats, tournaments, trackedPlayerIds, projectId: params.id };
};
