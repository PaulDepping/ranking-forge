import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Project } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch }) => {
	const api = makeApi(fetch, INTERNAL_API_URL);
	const res = await api.get('/projects');
	if (!res.ok) return { projects: [] as Project[] };
	const projects: Project[] = await res.json();
	return { projects };
};

export const actions: Actions = {
	delete: async ({ fetch, request }) => {
		const data = await request.formData();
		const id = data.get('id') as string;
		const api = makeApi(fetch, INTERNAL_API_URL);
		const res = await api.delete(`/projects/${id}`);
		if (!res.ok) {
			const body = await res.json().catch(() => ({ message: 'Delete failed' }));
			return fail(res.status, { error: body.message });
		}
	}
};
