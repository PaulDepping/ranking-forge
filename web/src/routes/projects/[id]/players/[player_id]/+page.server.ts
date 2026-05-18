import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { PlayerStats, TournamentAttendance } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, params, cookies }) => {
	const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));

	const [statsRes, tournamentsRes] = await Promise.all([
		api.get(`/projects/${params.id}/stats/${params.player_id}`),
		api.get(`/projects/${params.id}/players/${params.player_id}/tournaments`)
	]);

	if (!statsRes.ok) {
		if (statsRes.status === 404) {
			error(404, { message: 'not_found' });
		}
		error(statsRes.status, { message: 'error' });
	}

	const stats: PlayerStats = await statsRes.json();

	let tournaments: TournamentAttendance[] = [];
	if (tournamentsRes.ok) {
		tournaments = await tournamentsRes.json();
	}

	return { stats, tournaments };
};
