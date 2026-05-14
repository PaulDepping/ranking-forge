export function makeApi(fetchFn: typeof fetch, baseUrl: string, sessionId?: string | null) {
	async function req(method: string, path: string, body?: unknown): Promise<Response> {
		const headers: Record<string, string> = {};
		if (sessionId) headers['Cookie'] = `session_id=${sessionId}`;
		if (body !== undefined) headers['Content-Type'] = 'application/json';
		return fetchFn(baseUrl + path, {
			method,
			credentials: 'include',
			headers,
			body: body !== undefined ? JSON.stringify(body) : undefined
		});
	}

	return {
		get: (path: string) => req('GET', path),
		post: (path: string, body?: unknown) => req('POST', path, body),
		patch: (path: string, body: unknown) => req('PATCH', path, body),
		delete: (path: string) => req('DELETE', path)
	};
}
