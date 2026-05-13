import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Job } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, params }) => {
	const api = makeApi(fetch, INTERNAL_API_URL);
	const res = await api.get(`/projects/${params.id}/import`);
	const job: Job | null = res.ok ? await res.json() : null;
	return { job };
};

export const actions: Actions = {
	default: async ({ fetch, params }) => {
		const api = makeApi(fetch, INTERNAL_API_URL);
		const res = await api.post(`/projects/${params.id}/import`);
		if (!res.ok) {
			const err = await res.json().catch(() => ({ message: 'Failed to start import' }));
			return fail(res.status, { error: err.message });
		}
		const job: Job = await res.json();
		return { job };
	}
};
