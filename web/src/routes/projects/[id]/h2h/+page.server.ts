import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { HeadToHeadEntry, Player } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, params, cookies }) => {
	const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
	const [h2hRes, playersRes] = await Promise.all([
		api.get(`/projects/${params.id}/head-to-head`),
		api.get(`/projects/${params.id}/players`)
	]);
	const h2h: HeadToHeadEntry[] = h2hRes.ok ? await h2hRes.json() : [];
	const players: Player[] = playersRes.ok ? await playersRes.json() : [];
	return { h2h, players };
};
