import http from 'http';

const MOCK_USER = { id: 'user-1', email: 'testuser@test.com', display_name: 'testuser', created_at: '2026-01-01T00:00:00Z' };

const MOCK_PROJECTS = [
	{
		id: 'proj-1',
		name: 'SSBM Power Ranking',
		game_id: 1,
		game_name: 'Super Smash Bros. Melee',
		created_at: '2026-01-01T00:00:00Z',
		published: false,
		user_role: 'owner'
	}
];

const MOCK_VIEWER_PROJECT = {
	id: 'proj-viewer',
	name: 'SSBM Power Ranking',
	game_id: 1,
	game_name: 'Super Smash Bros. Melee',
	created_at: '2026-01-01T00:00:00Z',
	published: true,
	user_role: 'viewer'
};

const MOCK_PLAYERS = [
	{ id: 'player-1', project_id: 'proj-1', name: 'Alice', rank_position: 1, created_at: '2026-01-01T00:00:00Z', accounts: [] },
	{ id: 'player-2', project_id: 'proj-1', name: 'Bob', rank_position: 2, created_at: '2026-01-01T00:00:00Z', accounts: [] },
	{ id: 'player-3', project_id: 'proj-1', name: 'Charlie', rank_position: 3, created_at: '2026-01-01T00:00:00Z', accounts: [] }
];

const MOCK_ENTRANTS = {
	all_participants: [
		{ startgg_user_id: 1001, handle: 'mang0', name: 'Mang0' },
		{ startgg_user_id: 1002, handle: 'armada', name: 'Armada' },
		{ startgg_user_id: 9999, handle: 'spectator', name: 'Spectator' }
	],
	events: [
		{
			id: 101,
			name: 'Melee Singles',
			entrants: [
				{ startgg_user_id: 1001, handle: 'mang0', name: 'Mang0', seed: 1, placement: 2 },
				{ startgg_user_id: 1002, handle: 'armada', name: 'Armada', seed: 2, placement: 1 }
			]
		},
		{
			id: 102,
			name: 'Doubles',
			entrants: [
				{ startgg_user_id: 1001, handle: 'mang0', name: 'Mang0', seed: 1, placement: 1 }
			]
		}
	]
};

const MOCK_TOURNAMENTS = [
	{
		id: 't1', startgg_id: 1, name: 'Genesis 10', slug: 'tournament/genesis-10',
		city: 'San Jose', addr_state: 'CA', country_code: 'US',
		venue_name: null, online: false,
		start_at: '2025-01-12T00:00:00Z', end_at: null,
		events: [
			{
				id: 'e1', startgg_id: 1, name: 'Melee Singles',
				game_name: null, num_entrants: 256, start_at: null,
				included: true, event_type: 1,
				bracket_types: ['DOUBLE_ELIMINATION'],
			}
		]
	}
];

const MOCK_FAILED_JOB = {
	id: 'job-1',
	status: 'failed',
	error: 'start.gg API error: rate limit exceeded',
	progress: null,
	after_date: '2026-01-01',
	before_date: '2026-03-31',
	created_at: '2026-05-01T10:00:00Z',
	updated_at: '2026-05-01T10:01:00Z'
};

const MOCK_PENDING_JOB = {
	id: 'job-2',
	status: 'pending',
	error: null,
	progress: null,
	after_date: '2026-01-01',
	before_date: '2026-03-31',
	created_at: '2026-05-01T10:05:00Z',
	updated_at: '2026-05-01T10:05:00Z'
};

const MOCK_H2H = [
	{ player_id: 'player-1', opponent_id: 'player-2', wins: 3, losses: 1 },
	{ player_id: 'player-2', opponent_id: 'player-1', wins: 1, losses: 3 },
	{ player_id: 'player-1', opponent_id: 'player-3', wins: 2, losses: 0 },
	{ player_id: 'player-3', opponent_id: 'player-1', wins: 0, losses: 2 },
	{ player_id: 'player-2', opponent_id: 'player-3', wins: 2, losses: 1 },
	{ player_id: 'player-3', opponent_id: 'player-2', wins: 1, losses: 2 }
];

const MOCK_MEMBERS = [
	{ project_id: 'proj-1', user_id: 'user-2', display_name: 'editor_user', email: 'editor@test.com', role: 'editor', joined_at: '2026-01-01T00:00:00Z' }
];

/** @type {Array<{id: string, project_id: string, role: string, created_by: string, expires_at: string | null, revoked_at: string | null, created_at: string}>} */
const MOCK_INVITE_LINKS = [];

const MOCK_SET_BASE = {
	winner_score: 3, loser_score: 1,
	tournament_name: 'Test Tournament', tournament_slug: 'tournament/test-2024',
	event_name: 'Melee Singles', round_name: 'Winners Quarters',
	completed_at: '2024-01-20T18:00:00Z', is_dq: false,
	vod_url: null, startgg_set_id: 1001, winner_seed: 3, loser_seed: 7,
};

const MOCK_PLAYER_STATS = {
	player_id: 'player-1',
	name: 'Alice',
	wins: [{ ...MOCK_SET_BASE, opponent_id: 'player-2', opponent_name: 'Bob', upset_factor: 3 }],
	losses: [{ ...MOCK_SET_BASE, opponent_id: 'player-3', opponent_name: 'Charlie', upset_factor: 1 }],
};

const MOCK_PLAYER_TOURNAMENTS = [
	{
		tournament_name: 'Genesis 9',
		tournament_slug: 'tournament/genesis-9',
		event_name: 'Melee Singles',
		placement: 1,
		num_entrants: 486,
		start_at: '2024-01-12T00:00:00Z',
		location: 'San Jose, CA',
	},
	{
		tournament_name: 'CEO 2024',
		tournament_slug: 'tournament/ceo-2024',
		event_name: 'Melee Singles',
		placement: 5,
		num_entrants: 312,
		start_at: '2024-06-14T00:00:00Z',
		location: 'Kissimmee, FL',
	},
];

const MOCK_STATS = [
	{
		player_id: 'player-1',
		name: 'Alice',
		wins: [{ ...MOCK_SET_BASE, opponent_id: 'player-2', opponent_name: 'Bob', upset_factor: 2 }],
		losses: [{ ...MOCK_SET_BASE, opponent_id: 'player-3', opponent_name: 'Charlie', upset_factor: 1 }]
	},
	{
		player_id: 'player-2',
		name: 'Bob',
		wins: [],
		losses: [{ ...MOCK_SET_BASE, opponent_id: 'player-1', opponent_name: 'Alice', upset_factor: 2 }]
	},
	{
		player_id: 'player-3',
		name: 'Charlie',
		wins: [{ ...MOCK_SET_BASE, opponent_id: 'player-1', opponent_name: 'Alice', upset_factor: 1 }],
		losses: []
	}
];

/**
 * @param {import('http').IncomingMessage} req
 * @param {string} name
 * @param {string} value
 */
function hasCookie(req, name, value) {
	const header = req.headers.cookie || '';
	return header.split(';').some((c) => c.trim() === `${name}=${value}`);
}

/**
 * @param {import('http').ServerResponse} res
 * @param {number} status
 * @param {unknown} body
 */
function respond(res, status, body) {
	res.writeHead(status, { 'Content-Type': 'application/json' });
	res.end(JSON.stringify(body));
}

/**
 * @param {import('http').IncomingMessage} req
 * @returns {Promise<Record<string, unknown> | null>}
 */
/** @param {any} req @returns {Promise<any>} */
function readBody(req) {
	return new Promise((resolve) => {
		let data = '';
		req.on('data', (/** @type {Buffer} */ chunk) => (data += chunk));
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

		const url = new URL(req.url ?? '/', 'http://localhost');
		const path = url.pathname;
		const isAuthenticated = hasCookie(req, 'session_id', 'test-session');

		if (path === '/auth/me') {
			if (isAuthenticated) respond(res, 200, MOCK_USER);
			else respond(res, 401, { message: 'Unauthorized' });
			return;
		}

		if (path === '/auth/login' && req.method === 'POST') {
			const body = await readBody(req);
			if (body?.email === 'testuser@test.com' && body?.password === 'testpass') {
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
			const projectId = projectMatch[1];
			if (projectId === 'proj-viewer' || projectId === 'proj-viewer-tournaments') {
				respond(res, 200, { ...MOCK_VIEWER_PROJECT, id: projectId });
			} else {
				respond(res, 200, MOCK_PROJECTS[0]);
			}
			return;
		}

		const playersMatch = path.match(/^\/projects\/([^/]+)\/players$/);
		if (playersMatch && req.method === 'GET') {
			respond(res, 200, MOCK_PLAYERS);
			return;
		}

		if (playersMatch && req.method === 'POST') {
			const body = await readBody(req);
			respond(res, 201, {
				id: 'player-new',
				project_id: playersMatch[1],
				name: body?.name ?? 'New Player',
				created_at: '2026-01-01T00:00:00Z',
				accounts: []
			});
			return;
		}

		const tournamentEntrantsMatch = path.match(/^\/projects\/([^/]+)\/tournament-entrants$/);
		if (tournamentEntrantsMatch && req.method === 'GET') {
			respond(res, 200, MOCK_ENTRANTS);
			return;
		}

		const playersBulkMatch = path.match(/^\/projects\/([^/]+)\/players\/bulk$/);
		if (playersBulkMatch && req.method === 'POST') {
			const body = await readBody(req);
			const results = (body?.players ?? []).map((/** @type {any} */ p) => ({
				name: p.name,
				handle: p.handle,
				status: 'created'
			}));
			respond(res, 200, results);
			return;
		}

		const playersByHandlesMatch = path.match(/^\/projects\/([^/]+)\/players\/by-handles$/);
		if (playersByHandlesMatch && req.method === 'POST') {
			const body = await readBody(req);
			const results = (body?.handles ?? []).map((/** @type {string} */ h) => ({
				handle: h,
				name: 'Test Player',
				status: 'created'
			}));
			respond(res, 200, results);
			return;
		}

		const playerTournamentsMatch = path.match(/^\/projects\/([^/]+)\/players\/([^/]+)\/tournaments$/);
		if (playerTournamentsMatch && req.method === 'GET') {
			respond(res, 200, MOCK_PLAYER_TOURNAMENTS);
			return;
		}

		const playerPatchMatch = path.match(/^\/projects\/([^/]+)\/players\/([^/]+)$/);
		if (playerPatchMatch && req.method === 'PATCH') {
			const body = await readBody(req);
			respond(res, 200, {
				...MOCK_PLAYERS[0],
				id: playerPatchMatch[2],
				name: body?.name ?? 'Renamed'
			});
			return;
		}

		const rankingMatch = path.match(/^\/projects\/([^/]+)\/ranking$/);
		if (rankingMatch && req.method === 'PUT') {
			respond(res, 200, {});
			return;
		}

		const importMatch = path.match(/^\/projects\/([^/]+)\/import$/);
		if (importMatch) {
			const projectId = importMatch[1];
			if (req.method === 'GET') {
				respond(res, 200, projectId === 'proj-failed' ? MOCK_FAILED_JOB : null);
				return;
			}
			if (req.method === 'POST') {
				respond(res, 202, MOCK_PENDING_JOB);
				return;
			}
		}

		const tournamentsMatch = path.match(/^\/projects\/([^/]+)\/tournaments$/);
		if (tournamentsMatch && req.method === 'GET') {
			const projectId = tournamentsMatch[1];
			const hasTournaments = projectId === 'proj-tournaments' || projectId === 'proj-viewer-tournaments';
			respond(res, 200, hasTournaments ? MOCK_TOURNAMENTS : []);
			return;
		}

		const playerStatsMatch = path.match(/^\/projects\/([^/]+)\/stats\/([^/]+)$/);
		if (playerStatsMatch && req.method === 'GET') {
			respond(res, 200, MOCK_PLAYER_STATS);
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

		const h2hSetsMatch = path.match(/^\/projects\/([^/]+)\/head-to-head\/([^/]+)\/([^/]+)\/sets$/);
		if (h2hSetsMatch && req.method === 'GET') {
			respond(res, 200, [
				{ ...MOCK_SET_BASE, opponent_id: h2hSetsMatch[3], opponent_name: 'Bob', upset_factor: 2, is_win: true },
			]);
			return;
		}

		const membersMatch = path.match(/^\/projects\/([^/]+)\/members$/);
		if (membersMatch) {
			if (req.method === 'GET') { respond(res, 200, MOCK_MEMBERS); return; }
			if (req.method === 'POST') { respond(res, 204, null); return; }
		}

		const transferMatch = path.match(/^\/projects\/([^/]+)\/members\/transfer-ownership$/);
		if (transferMatch && req.method === 'POST') {
			respond(res, 204, null);
			return;
		}

		const memberMatch = path.match(/^\/projects\/([^/]+)\/members\/([^/]+)$/);
		if (memberMatch) {
			if (req.method === 'PATCH') { respond(res, 204, null); return; }
			if (req.method === 'DELETE') { respond(res, 204, null); return; }
		}

		const inviteLinksMatch = path.match(/^\/projects\/([^/]+)\/invite-links$/);
		if (inviteLinksMatch) {
			if (req.method === 'GET') { respond(res, 200, MOCK_INVITE_LINKS); return; }
			if (req.method === 'POST') {
				const body = await readBody(req);
				respond(res, 201, { id: 'link-new', project_id: inviteLinksMatch[1], role: body?.role ?? 'editor', created_by: 'user-1', expires_at: body?.expires_at ?? null, revoked_at: null, created_at: new Date().toISOString() });
				return;
			}
		}

		const inviteLinkMatch = path.match(/^\/projects\/([^/]+)\/invite-links\/([^/]+)$/);
		if (inviteLinkMatch && req.method === 'DELETE') {
			respond(res, 204, null);
			return;
		}

		const inviteAcceptMatch = path.match(/^\/invite\/([^/]+)\/accept$/);
		if (inviteAcceptMatch && req.method === 'POST') {
			respond(res, 200, { project_id: 'proj-1' });
			return;
		}

		if (path === '/account/profile' && req.method === 'PATCH') {
			respond(res, 204, null);
			return;
		}

		if (path === '/account/password' && req.method === 'PATCH') {
			respond(res, 204, null);
			return;
		}

		if (path === '/account' && req.method === 'DELETE') {
			res.setHeader('Set-Cookie', 'session_id=; Max-Age=0; Path=/');
			respond(res, 204, null);
			return;
		}

		respond(res, 404, { message: 'Not found' });
	});
}

const server = createMockServer();
server.listen(9999, () => {
	process.stdout.write('Mock API listening on :9999\n');
});
