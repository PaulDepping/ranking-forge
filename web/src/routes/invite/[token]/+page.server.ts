import { redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import { env } from '$env/dynamic/private';

export const load: PageServerLoad = async ({ params }) => {
	return { token: params.token };
};

export const actions: Actions = {
	accept: async ({ fetch, params, cookies }) => {
		const api = makeApi(fetch, env.INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.post(`/invite/${params.token}/accept`);
		if (!res.ok) {
			if (res.status === 401) {
				redirect(303, `/login?next=/invite/${params.token}`);
			}
			const body = await res.json().catch(() => ({ message: 'Failed to accept invite' }));
			return { error: body.message };
		}
		const data = await res.json();
		redirect(303, `/projects/${data.project_id}`);
	}
};
