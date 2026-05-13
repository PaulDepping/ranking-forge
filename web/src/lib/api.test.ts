import { describe, it, expect, vi } from 'vitest';
import { makeApi } from './api';

describe('makeApi', () => {
	it('sends GET with credentials:include', async () => {
		const mockFetch = vi.fn().mockResolvedValue(new Response('{}'));
		const api = makeApi(mockFetch, 'http://localhost:8080');

		await api.get('/projects');

		expect(mockFetch).toHaveBeenCalledWith(
			'http://localhost:8080/projects',
			expect.objectContaining({ method: 'GET', credentials: 'include' })
		);
	});

	it('sends POST with JSON body and Content-Type header', async () => {
		const mockFetch = vi.fn().mockResolvedValue(new Response('{}'));
		const api = makeApi(mockFetch, 'http://localhost:8080');

		await api.post('/projects', { name: 'Test' });

		expect(mockFetch).toHaveBeenCalledWith('http://localhost:8080/projects', {
			method: 'POST',
			credentials: 'include',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ name: 'Test' })
		});
	});

	it('sends POST without body or Content-Type when no body given', async () => {
		const mockFetch = vi.fn().mockResolvedValue(new Response('{}'));
		const api = makeApi(mockFetch, 'http://localhost:8080');

		await api.post('/auth/logout');

		expect(mockFetch).toHaveBeenCalledWith('http://localhost:8080/auth/logout', {
			method: 'POST',
			credentials: 'include',
			headers: {},
			body: undefined
		});
	});

	it('sends PATCH with JSON body', async () => {
		const mockFetch = vi.fn().mockResolvedValue(new Response('{}'));
		const api = makeApi(mockFetch, 'http://localhost:8080');

		await api.patch('/projects/1/events/2', { included: false });

		expect(mockFetch).toHaveBeenCalledWith('http://localhost:8080/projects/1/events/2', {
			method: 'PATCH',
			credentials: 'include',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ included: false })
		});
	});

	it('sends DELETE with no body or Content-Type', async () => {
		const mockFetch = vi.fn().mockResolvedValue(new Response(null, { status: 200 }));
		const api = makeApi(mockFetch, 'http://localhost:8080');

		await api.delete('/projects/1');

		expect(mockFetch).toHaveBeenCalledWith('http://localhost:8080/projects/1', {
			method: 'DELETE',
			credentials: 'include',
			headers: {},
			body: undefined
		});
	});

	it('prepends the base URL to every path', async () => {
		const mockFetch = vi.fn().mockResolvedValue(new Response('{}'));
		const api = makeApi(mockFetch, 'https://api.example.com');

		await api.get('/auth/me');

		expect(mockFetch).toHaveBeenCalledWith(
			'https://api.example.com/auth/me',
			expect.anything()
		);
	});

	it('returns the raw fetch response', async () => {
		const mockResponse = new Response(JSON.stringify({ id: '1' }), { status: 200 });
		const mockFetch = vi.fn().mockResolvedValue(mockResponse);
		const api = makeApi(mockFetch, 'http://localhost:8080');

		const result = await api.get('/projects/1');

		expect(result).toBe(mockResponse);
	});
});
