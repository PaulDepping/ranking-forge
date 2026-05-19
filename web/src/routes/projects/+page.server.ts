import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Project } from '$lib/types';
import { INTERNAL_API_URL } from '$env/dynamic/private';

export const load: PageServerLoad = async ({ fetch, cookies }) => {
	const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
	const res = await api.get('/projects');
	if (!res.ok) return { projects: [] as Project[] };
	const projects: Project[] = await res.json();
	return { projects };
};
