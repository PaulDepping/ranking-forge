import { fail, redirect } from '@sveltejs/kit';
import type { Actions } from './$types';
import { makeApi } from '$lib/api';
import { INTERNAL_API_URL } from '$env/static/private';

export const actions: Actions = {
	default: async ({ fetch, request, cookies }) => {
		const data = await request.formData();
		const name = (data.get('name') as string)?.trim();
		const game_id_raw = data.get('game_id') as string | null;
		const game_name = (data.get('game_name') as string | null) || null;

		if (!name) return fail(422, { error: 'Project name is required' });

		const body: Record<string, unknown> = { name };
		if (game_id_raw) body.game_id = parseInt(game_id_raw, 10);
		if (game_name) body.game_name = game_name;

		const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.post('/projects', body);

		if (!res.ok) {
			const err = await res.json().catch(() => ({ message: 'Failed to create project' }));
			return fail(res.status, { error: err.message });
		}

		const project = await res.json();
		redirect(303, `/projects/${project.id}/players`);
	}
};
