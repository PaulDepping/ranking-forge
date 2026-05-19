import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { INTERNAL_API_URL } from '$env/dynamic/private';

export const load: PageServerLoad = ({ locals }) => {
	if (locals.user) redirect(303, '/projects');
};

export const actions: Actions = {
	default: async ({ fetch, request, cookies }) => {
		const data = await request.formData();
		const username = data.get('username') as string;
		const password = data.get('password') as string;

		const res = await fetch(`${INTERNAL_API_URL}/auth/login`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ username, password })
		});

		if (!res.ok) {
			const body = await res.json().catch(() => ({ message: 'Login failed' }));
			return fail(res.status, { error: body.message ?? 'Login failed' });
		}

		const setCookie = res.headers.get('set-cookie');
		const match = setCookie?.match(/session_id=([^;]+)/);
		if (match) {
			cookies.set('session_id', match[1], {
				path: '/',
				httpOnly: true,
				sameSite: 'strict',
				maxAge: 60 * 60 * 24 * 30
			});
		}

		redirect(303, '/projects');
	}
};
