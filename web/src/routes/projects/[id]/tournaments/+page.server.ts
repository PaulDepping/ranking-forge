import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Tournament } from '$lib/types';
import { env } from '$env/dynamic/private';

export const load: PageServerLoad = async ({ fetch, params, cookies }) => {
	const api = makeApi(fetch, env.INTERNAL_API_URL, cookies.get('session_id'));
	const res = await api.get(`/projects/${params.id}/tournaments`);
	const tournaments: Tournament[] = res.ok ? await res.json() : [];
	return { tournaments };
};
