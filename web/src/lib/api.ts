export function makeApi(fetchFn: typeof fetch, baseUrl: string) {
	async function req(method: string, path: string, body?: unknown): Promise<Response> {
		return fetchFn(baseUrl + path, {
			method,
			credentials: 'include',
			headers: body !== undefined ? { 'Content-Type': 'application/json' } : {},
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
