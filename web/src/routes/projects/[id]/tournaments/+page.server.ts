import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Tournament } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, params }) => {
	const api = makeApi(fetch, INTERNAL_API_URL);
	const res = await api.get(`/projects/${params.id}/tournaments`);
	const tournaments: Tournament[] = res.ok ? await res.json() : [];
	return { tournaments };
};
