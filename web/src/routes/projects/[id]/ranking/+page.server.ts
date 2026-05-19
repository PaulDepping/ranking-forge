import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Player, PlayerStats } from '$lib/types';
import { env } from '$env/dynamic/private';

export const load: PageServerLoad = async ({ fetch, params, cookies }) => {
	const api = makeApi(fetch, env.INTERNAL_API_URL, cookies.get('session_id'));
	const [playersRes, statsRes] = await Promise.all([
		api.get(`/projects/${params.id}/players`),
		api.get(`/projects/${params.id}/stats`)
	]);
	const players: Player[] = playersRes.ok ? await playersRes.json() : [];
	const stats: PlayerStats[] = statsRes.ok ? await statsRes.json() : [];
	return { players, stats };
};
