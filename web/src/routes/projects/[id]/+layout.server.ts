import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Project } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: LayoutServerLoad = async ({ fetch, params, cookies }) => {
	const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
	const res = await api.get(`/projects/${params.id}`);
	if (!res.ok) redirect(303, '/projects');
	const project: Project = await res.json();
	return { project };
};
