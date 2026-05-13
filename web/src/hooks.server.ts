import type { Handle } from '@sveltejs/kit';
import { redirect } from '@sveltejs/kit';
import { INTERNAL_API_URL } from '$env/static/private';

export const handle: Handle = async ({ event, resolve }) => {
	const { pathname } = event.url;

	const res = await event.fetch(`${INTERNAL_API_URL}/auth/me`);
	if (res.ok) {
		event.locals.user = await res.json();
	} else {
		event.locals.user = null;
		const publicRoutes = ['/login', '/register'];
		if (!publicRoutes.includes(pathname)) {
			redirect(303, '/login');
		}
	}

	return resolve(event);
};
