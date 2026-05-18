import type { Handle } from '@sveltejs/kit';
import { redirect } from '@sveltejs/kit';
import { INTERNAL_API_URL } from '$env/static/private';

export const handle: Handle = async ({ event, resolve }) => {
	const { pathname } = event.url;

	const sessionId = event.cookies.get('session_id');
	const res = await event.fetch(`${INTERNAL_API_URL}/auth/me`, {
		headers: sessionId ? { Cookie: `session_id=${sessionId}` } : {}
	});
	if (res.ok) {
		event.locals.user = await res.json();
	} else {
		event.locals.user = null;
		const isPublic =
			['/login', '/register'].includes(pathname) ||
			/^\/projects\/[^/]/.test(pathname) ||
			/^\/invite\//.test(pathname);
		if (!isPublic) {
			redirect(303, '/login');
		}
	}

	return resolve(event);
};
