import { fail } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Player } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, params, cookies }) => {
	const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
	const res = await api.get(`/projects/${params.id}/players`);
	const players: Player[] = res.ok ? await res.json() : [];
	return { players };
};

export const actions: Actions = {
	addPlayer: async ({ fetch, request, params, cookies }) => {
		const data = await request.formData();
		const name = (data.get('name') as string)?.trim();
		if (!name) return fail(422, { addError: 'Player name is required' });

		const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.post(`/projects/${params.id}/players`, { name });
		if (!res.ok) {
			const err = await res.json().catch(() => ({ message: 'Failed to add player' }));
			return fail(res.status, { addError: err.message });
		}
	},

	deletePlayer: async ({ fetch, request, params, cookies }) => {
		const data = await request.formData();
		const pid = data.get('pid') as string;
		const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.delete(`/projects/${params.id}/players/${pid}`);
		if (!res.ok) return fail(res.status, { deleteError: 'Failed to delete player' });
	},

	renamePlayer: async ({ fetch, request, params, cookies }) => {
		const data = await request.formData();
		const pid = data.get('pid') as string;
		const name = (data.get('name') as string)?.trim();
		if (!name) return fail(422, { renameError: 'Name is required', renamePid: pid });

		const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.patch(`/projects/${params.id}/players/${pid}`, { name });
		if (!res.ok) {
			const err = await res.json().catch(() => ({ message: 'Failed to rename player' }));
			return fail(res.status, { renameError: err.message, renamePid: pid });
		}
	},

	linkAccount: async ({ fetch, request, params, cookies }) => {
		const data = await request.formData();
		const pid = data.get('pid') as string;
		const handle = (data.get('handle') as string)?.trim();
		if (!handle) return fail(422, { linkError: 'Handle is required', linkPid: pid });

		const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.post(`/projects/${params.id}/players/${pid}/accounts`, { handle });
		if (!res.ok) {
			const err = await res.json().catch(() => ({ message: 'Failed to link account' }));
			return fail(res.status, { linkError: err.message, linkPid: pid });
		}
	},

	unlinkAccount: async ({ fetch, request, params, cookies }) => {
		const data = await request.formData();
		const pid = data.get('pid') as string;
		const aid = data.get('aid') as string;
		const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.delete(`/projects/${params.id}/players/${pid}/accounts/${aid}`);
		if (!res.ok) return fail(res.status, { deleteError: 'Failed to unlink account' });
	}
};
