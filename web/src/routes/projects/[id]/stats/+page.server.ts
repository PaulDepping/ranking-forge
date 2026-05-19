import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { PlayerStats } from '$lib/types';
import { INTERNAL_API_URL } from '$env/dynamic/private';

export const load: PageServerLoad = async ({ fetch, params, cookies }) => {
	const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
	const res = await api.get(`/projects/${params.id}/stats`);
	const stats: PlayerStats[] = res.ok ? await res.json() : [];
	return { stats };
};
