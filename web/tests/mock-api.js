import http from 'http';

const MOCK_USER = { id: 'user-1', username: 'testuser', created_at: '2026-01-01T00:00:00Z' };

const MOCK_PROJECTS = [
	{
		id: 'proj-1',
		name: 'SSBM Power Ranking',
		game_id: 1,
		game_name: 'Super Smash Bros. Melee',
		created_at: '2026-01-01T00:00:00Z'
	}
];

const MOCK_PLAYERS = [
	{ id: 'player-1', project_id: 'proj-1', name: 'Alice', created_at: '2026-01-01T00:00:00Z', accounts: [] },
	{ id: 'player-2', project_id: 'proj-1', name: 'Bob', created_at: '2026-01-01T00:00:00Z', accounts: [] },
	{ id: 'player-3', project_id: 'proj-1', name: 'Charlie', created_at: '2026-01-01T00:00:00Z', accounts: [] }
];

const MOCK_H2H = [
	{ player_id: 'player-1', opponent_id: 'player-2', wins: 3, losses: 1 },
	{ player_id: 'player-2', opponent_id: 'player-1', wins: 1, losses: 3 },
	{ player_id: 'player-1', opponent_id: 'player-3', wins: 2, losses: 0 },
	{ player_id: 'player-3', opponent_id: 'player-1', wins: 0, losses: 2 },
	{ player_id: 'player-2', opponent_id: 'player-3', wins: 2, losses: 1 },
	{ player_id: 'player-3', opponent_id: 'player-2', wins: 1, losses: 2 }
];

const MOCK_STATS = [
	{
		player_id: 'player-1',
		name: 'Alice',
		wins: [{ opponent_id: 'player-2', opponent_name: 'Bob', upset_factor: 2.0 }],
		losses: [{ opponent_id: 'player-3', opponent_name: 'Charlie', upset_factor: 0.5 }]
	},
	{
		player_id: 'player-2',
		name: 'Bob',
		wins: [],
		losses: [{ opponent_id: 'player-1', opponent_name: 'Alice', upset_factor: 2.0 }]
	},
	{
		player_id: 'player-3',
		name: 'Charlie',
		wins: [{ opponent_id: 'player-1', opponent_name: 'Alice', upset_factor: 0.5 }],
		losses: []
	}
];

function hasCookie(req, name, value) {
	const header = req.headers.cookie || '';
	return header.split(';').some((c) => c.trim() === `${name}=${value}`);
}

function respond(res, status, body) {
	res.writeHead(status, { 'Content-Type': 'application/json' });
	res.end(JSON.stringify(body));
}

function readBody(req) {
	return new Promise((resolve) => {
		let data = '';
		req.on('data', (chunk) => (data += chunk));
		req.on('end', () => {
			try {
				resolve(data ? JSON.parse(data) : null);
			} catch {
				resolve(null);
			}
		});
	});
}

function createMockServer() {
	return http.createServer(async (req, res) => {
		res.setHeader('Access-Control-Allow-Origin', 'http://localhost:5174');
		res.setHeader('Access-Control-Allow-Methods', 'GET, POST, PATCH, DELETE, OPTIONS');
		res.setHeader('Access-Control-Allow-Headers', 'Content-Type');
		res.setHeader('Access-Control-Allow-Credentials', 'true');

		if (req.method === 'OPTIONS') {
			res.writeHead(204);
			res.end();
			return;
		}

		const url = new URL(req.url, 'http://localhost');
		const path = url.pathname;
		const isAuthenticated = hasCookie(req, 'session_id', 'test-session');

		if (path === '/auth/me') {
			if (isAuthenticated) respond(res, 200, MOCK_USER);
			else respond(res, 401, { message: 'Unauthorized' });
			return;
		}

		if (path === '/auth/login' && req.method === 'POST') {
			const body = await readBody(req);
			if (body?.username === 'testuser' && body?.password === 'testpass') {
				res.setHeader('Set-Cookie', 'session_id=test-session; HttpOnly; Path=/; SameSite=Strict');
				respond(res, 200, MOCK_USER);
			} else {
				respond(res, 401, { message: 'Invalid username or password' });
			}
			return;
		}

		if (path === '/auth/register' && req.method === 'POST') {
			res.setHeader('Set-Cookie', 'session_id=test-session; HttpOnly; Path=/; SameSite=Strict');
			respond(res, 201, MOCK_USER);
			return;
		}

		if (path === '/auth/logout' && req.method === 'POST') {
			res.setHeader('Set-Cookie', 'session_id=; Max-Age=0; Path=/');
			respond(res, 200, {});
			return;
		}

		if (path === '/projects' && req.method === 'GET') {
			respond(res, 200, MOCK_PROJECTS);
			return;
		}

		if (path === '/projects' && req.method === 'POST') {
			const body = await readBody(req);
			respond(res, 201, { ...MOCK_PROJECTS[0], name: body?.name ?? 'New Project' });
			return;
		}

		if (path === '/games') {
			const q = url.searchParams.get('q') ?? '';
			respond(res, 200, [
				{ id: 1, name: 'melee', display_name: 'Super Smash Bros. Melee' },
				{ id: 2, name: 'ultimate', display_name: 'Super Smash Bros. Ultimate' }
			].filter((g) => g.display_name.toLowerCase().includes(q.toLowerCase())));
			return;
		}

		const projectMatch = path.match(/^\/projects\/([^/]+)$/);
		if (projectMatch && req.method === 'GET') {
			respond(res, 200, MOCK_PROJECTS[0]);
			return;
		}

		const playersMatch = path.match(/^\/projects\/([^/]+)\/players$/);
		if (playersMatch && req.method === 'GET') {
			respond(res, 200, MOCK_PLAYERS);
			return;
		}

		const importMatch = path.match(/^\/projects\/([^/]+)\/import$/);
		if (importMatch && req.method === 'GET') {
			respond(res, 200, null);
			return;
		}

		const tournamentsMatch = path.match(/^\/projects\/([^/]+)\/tournaments$/);
		if (tournamentsMatch && req.method === 'GET') {
			respond(res, 200, []);
			return;
		}

		const statsMatch = path.match(/^\/projects\/([^/]+)\/stats$/);
		if (statsMatch && req.method === 'GET') {
			respond(res, 200, MOCK_STATS);
			return;
		}

		const h2hMatch = path.match(/^\/projects\/([^/]+)\/head-to-head$/);
		if (h2hMatch && req.method === 'GET') {
			respond(res, 200, MOCK_H2H);
			return;
		}

		respond(res, 404, { message: 'Not found' });
	});
}

const server = createMockServer();
server.listen(9999, () => {
	process.stdout.write('Mock API listening on :9999\n');
});
