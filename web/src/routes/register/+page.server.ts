import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = ({ locals }) => {
	if (locals.user) redirect(303, '/projects');
};

export const actions: Actions = {
	default: async ({ fetch, request }) => {
		const data = await request.formData();
		const username = data.get('username') as string;
		const password = data.get('password') as string;

		const res = await fetch(`${INTERNAL_API_URL}/auth/register`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ username, password })
		});

		if (!res.ok) {
			const body = await res.json().catch(() => ({ message: 'Registration failed' }));
			return fail(res.status, { error: body.message ?? 'Registration failed' });
		}

		redirect(303, '/projects');
	}
};
