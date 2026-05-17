import { fail, redirect } from '@sveltejs/kit';
import type { Actions } from './$types';
import { makeApi } from '$lib/api';
import { INTERNAL_API_URL } from '$env/static/private';

export const actions: Actions = {
	rename: async ({ fetch, params, cookies, request }) => {
		const data = await request.formData();
		const name = (data.get('name') as string ?? '').trim();
		if (!name) return fail(400, { renameError: 'Name is required' });
		if ([...name].length > 100) return fail(400, { renameError: 'Name must be at most 100 characters' });
		const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.patch(`/projects/${params.id}`, { name });
		if (!res.ok) {
			const body = await res.json().catch(() => ({ message: 'Rename failed' }));
			return fail(res.status, { renameError: body.message });
		}
		const project = await res.json();
		return { project };
	},

	delete: async ({ fetch, params, cookies }) => {
		const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.delete(`/projects/${params.id}`);
		if (!res.ok) {
			const body = await res.json().catch(() => ({ message: 'Delete failed' }));
			return fail(res.status, { deleteError: body.message });
		}
		redirect(303, '/projects');
	}
};
