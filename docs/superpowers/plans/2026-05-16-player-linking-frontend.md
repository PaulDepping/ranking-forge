# Player Linking Frontend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Players page inline-form with a three-tab "Add players" dialog, add inline player rename, and update the link-account dialog to accept handles/URLs.

**Architecture:** All three dialog tabs (From tournament / By handle / By name) are standalone Svelte components making client-side API calls via `makeApi(fetch, PUBLIC_API_URL)`. The dialog shell (`AddPlayersDialog.svelte`) owns open/close state and composes the three tabs. Inline rename uses an existing SvelteKit form action. After any successful mutation, `invalidateAll()` refreshes the player list.

**Tech Stack:** SvelteKit + Svelte 5 runes, shadcn-svelte (Dialog, Tabs, Input, Textarea, Checkbox, ScrollArea, Badge, Button), Playwright e2e tests, Vitest.

---

## File Map

| Action | Path |
|---|---|
| Modify | `web/src/lib/types.ts` |
| Modify | `web/tests/mock-api.js` |
| Modify | `web/tests/projects.test.ts` |
| Modify | `web/src/routes/projects/[id]/players/+page.server.ts` |
| Create | `web/src/lib/components/NameTab.svelte` |
| Create | `web/src/lib/components/HandleTab.svelte` |
| Create | `web/src/lib/components/TournamentTab.svelte` |
| Create | `web/src/lib/components/AddPlayersDialog.svelte` |
| Modify | `web/src/routes/projects/[id]/players/+page.svelte` |

---

### Task 1: Update frontend types

**Files:**
- Modify: `web/src/lib/types.ts`

- [ ] **Step 1: Apply the changes**

In `web/src/lib/types.ts`, rename `Account.slug` to `Account.handle` and add three new interfaces:

```ts
export interface User {
	id: string;
	username: string;
	created_at: string;
}

export interface Project {
	id: string;
	name: string;
	game_id: number | null;
	game_name: string | null;
	created_at: string;
}

export interface Account {
	id: string;
	startgg_user_id: number;
	handle: string;
	display_name: string | null;
}

export interface Player {
	id: string;
	project_id: string;
	name: string;
	created_at: string;
	accounts: Account[];
}

export interface TournamentEntrant {
	startgg_user_id: number;
	handle: string;
	name: string;
}

export interface BulkAddResult {
	name: string;
	handle: string;
	status: 'created' | 'skipped';
}

export interface ByHandlesResult {
	handle: string;
	name: string | null;
	status: 'created' | 'skipped' | 'not_found';
}

export interface TournamentEvent {
	id: string;
	startgg_id: number;
	name: string;
	game_name: string | null;
	num_entrants: number | null;
	start_at: string | null;
	included: boolean;
	event_type: number | null;
	bracket_types: string[];
}

export interface Tournament {
	id: string;
	startgg_id: number;
	name: string;
	slug: string;
	city: string | null;
	addr_state: string | null;
	country_code: string | null;
	venue_name: string | null;
	online: boolean;
	start_at: string | null;
	end_at: string | null;
	events: TournamentEvent[];
}

export interface Job {
	id: string;
	status: 'pending' | 'running' | 'done' | 'failed';
	error: string | null;
	after_date: string | null;
	before_date: string | null;
	created_at: string;
	updated_at: string;
}

export interface SetRecord {
	opponent_id: string;
	opponent_name: string;
	upset_factor: number;
	winner_score: number | null;
	loser_score: number | null;
	tournament_name: string;
	tournament_slug: string;
	event_name: string;
	round_name: string | null;
	completed_at: string | null;
	is_dq: boolean;
	vod_url: string | null;
	startgg_set_id: number;
	winner_seed: number | null;
	loser_seed: number | null;
	phase_name: string | null;
	pool_identifier: string | null;
	winner_placement: number | null;
	loser_placement: number | null;
	location: string | null;
	num_entrants: number | null;
	event_slug: string | null;
}

export interface H2HSet extends SetRecord {
	is_win: boolean;
}

export interface PlayerStats {
	player_id: string;
	name: string;
	wins: SetRecord[];
	losses: SetRecord[];
}

export interface HeadToHeadEntry {
	player_id: string;
	opponent_id: string;
	wins: number;
	losses: number;
}

export interface Game {
	id: number;
	name: string;
	display_name: string | null;
}
```

- [ ] **Step 2: Verify TypeScript sees no errors**

```bash
cd /home/pd/private_projects/ranking_forge/web && npx tsc --noEmit 2>&1 | head -30
```

Expected: errors only in files that reference `account.slug` (those get fixed in Task 9). Zero errors in `types.ts` itself.

- [ ] **Step 3: Commit**

```bash
cd /home/pd/private_projects/ranking_forge && git add web/src/lib/types.ts
git commit -m "feat(web): rename Account.slug→handle, add TournamentEntrant/BulkAddResult/ByHandlesResult types"
```

---

### Task 2: Extend the mock API server

**Files:**
- Modify: `web/tests/mock-api.js`

The Playwright e2e tests hit a local mock server (`tests/mock-api.js`) instead of the real backend. We need handlers for the five new/changed endpoints before writing the tests.

- [ ] **Step 1: Add a `MOCK_ENTRANTS` constant** near the top of `web/tests/mock-api.js`, after `MOCK_PLAYERS`:

```js
const MOCK_ENTRANTS = [
	{ startgg_user_id: 1001, handle: 'mang0', name: 'Mang0' },
	{ startgg_user_id: 1002, handle: 'armada', name: 'Armada' }
];
```

- [ ] **Step 2: Add new route handlers** inside `createMockServer`, before the final `respond(res, 404, ...)` line. Insert them after the existing `playersMatch` block:

```js
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
```

> **Important — route order:** The `bulk` and `by-handles` patterns must appear before `playerPatchMatch` (`/players/([^/]+)`) so the literal path segments `bulk` and `by-handles` don't get captured as `:pid`.

- [ ] **Step 3: Verify the mock server starts cleanly**

```bash
cd /home/pd/private_projects/ranking_forge/web && node tests/mock-api.js &
sleep 1 && curl -s http://localhost:9999/projects/proj-1/tournament-entrants | head -c 200
kill %1
```

Expected output: `[{"startgg_user_id":1001,"handle":"mang0","name":"Mang0"},...]`

- [ ] **Step 4: Commit**

```bash
cd /home/pd/private_projects/ranking_forge && git add web/tests/mock-api.js
git commit -m "test(web): add mock API handlers for bulk, by-handles, tournament-entrants, rename"
```

---

### Task 3: Write failing e2e tests

**Files:**
- Modify: `web/tests/projects.test.ts`

Write the tests first so they fail, then implement the feature to make them pass.

- [ ] **Step 1: Add the new test cases** to `web/tests/projects.test.ts`, inside the existing authenticated `test` block (after the last existing test):

```ts
test('players page shows Add players button and no inline name form', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await expect(page.getByRole('button', { name: 'Add players' })).toBeVisible();
	await expect(page.getByPlaceholder('Player name')).not.toBeVisible();
});

test('Add players dialog opens with three tabs', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await page.getByRole('button', { name: 'Add players' }).click();
	await expect(page.getByRole('tab', { name: 'From tournament' })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'By handle' })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'By name' })).toBeVisible();
});

test('By name tab adds a player and clears the input', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await page.getByRole('button', { name: 'Add players' }).click();
	await page.getByRole('tab', { name: 'By name' }).click();
	await page.getByLabel('Player name').fill('TestPlayer');
	await page.getByRole('button', { name: 'Add player' }).click();
	await expect(page.getByLabel('Player name')).toHaveValue('');
});

test('player row has Edit button; clicking it shows inline input', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await page.getByRole('button', { name: 'Edit' }).first().click();
	await expect(page.getByRole('textbox').first()).toBeVisible();
	await page.getByRole('button', { name: 'Cancel' }).click();
	await expect(page.getByText('Alice').first()).toBeVisible();
});
```

- [ ] **Step 2: Run only the new tests to confirm they fail**

```bash
cd /home/pd/private_projects/ranking_forge/web && npx playwright test --grep "Add players|Edit button" 2>&1 | tail -20
```

Expected: all four new tests FAIL (page still has the old inline form, no dialog).

- [ ] **Step 3: Commit the failing tests**

```bash
cd /home/pd/private_projects/ranking_forge && git add web/tests/projects.test.ts
git commit -m "test(web): add failing e2e tests for Add players dialog and inline rename"
```

---

### Task 4: Update the server actions

**Files:**
- Modify: `web/src/routes/projects/[id]/players/+page.server.ts`

- [ ] **Step 1: Replace the file contents** with:

```ts
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
```

- [ ] **Step 2: Commit**

```bash
cd /home/pd/private_projects/ranking_forge && git add web/src/routes/projects/[id]/players/+page.server.ts
git commit -m "feat(web): add renamePlayer action, rename linkAccount slug→handle"
```

---

### Task 5: Create NameTab.svelte

**Files:**
- Create: `web/src/lib/components/NameTab.svelte`

- [ ] **Step 1: Create the file**

```svelte
<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { makeApi } from '$lib/api';
	import { invalidateAll } from '$app/navigation';

	let { projectId }: { projectId: string } = $props();

	let name = $state('');
	let submitting = $state(false);
	let error = $state<string | null>(null);

	async function submit() {
		const trimmed = name.trim();
		if (!trimmed) return;
		submitting = true;
		error = null;
		const api = makeApi(fetch, PUBLIC_API_URL);
		const res = await api.post(`/projects/${projectId}/players`, { name: trimmed });
		submitting = false;
		if (res.ok) {
			name = '';
			await invalidateAll();
		} else {
			const err = await res.json().catch(() => ({ message: 'Failed to add player' }));
			error = err.message;
		}
	}
</script>

<div class="space-y-4">
	<div class="space-y-2">
		<Label for="player-name">Player name</Label>
		<Input id="player-name" bind:value={name} placeholder="e.g. Mang0" disabled={submitting} />
		{#if error}
			<p class="text-sm text-destructive">{error}</p>
		{/if}
		<p class="text-xs text-muted-foreground">
			Creates a player with no start.gg account. You can link one later.
		</p>
	</div>
	<Button onclick={submit} disabled={submitting || !name.trim()} class="w-full">
		{submitting ? 'Adding…' : 'Add player'}
	</Button>
</div>
```

- [ ] **Step 2: Commit**

```bash
cd /home/pd/private_projects/ranking_forge && git add web/src/lib/components/NameTab.svelte
git commit -m "feat(web): add NameTab component for name-only player creation"
```

---

### Task 6: Create HandleTab.svelte

**Files:**
- Create: `web/src/lib/components/HandleTab.svelte`

The `Textarea` component lives at `web/src/lib/components/ui/textarea/` (files already exist on disk).

- [ ] **Step 1: Create the file**

```svelte
<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { Textarea } from '$lib/components/ui/textarea';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { makeApi } from '$lib/api';
	import { invalidateAll } from '$app/navigation';
	import type { ByHandlesResult } from '$lib/types';

	let { projectId, onClose }: { projectId: string; onClose: () => void } = $props();

	let input = $state('');
	let submitting = $state(false);
	let results = $state<ByHandlesResult[]>([]);

	async function submit() {
		const handles = input.split('\n').map((h) => h.trim()).filter(Boolean);
		if (!handles.length) return;
		submitting = true;
		const api = makeApi(fetch, PUBLIC_API_URL);
		const res = await api.post(`/projects/${projectId}/players/by-handles`, { handles });
		submitting = false;
		if (res.ok) {
			results = await res.json();
		}
	}

	function done() {
		const anyCreated = results.some((r) => r.status === 'created');
		results = [];
		input = '';
		if (anyCreated) invalidateAll();
		onClose();
	}
</script>

<div class="space-y-3">
	{#if results.length === 0}
		<div class="space-y-2">
			<Label for="handles-input">One handle per line — bare handle, full slug, or full URL</Label>
			<Textarea
				id="handles-input"
				bind:value={input}
				placeholder={'mang0\nhttps://start.gg/user/armada'}
				rows={5}
				disabled={submitting}
				class="font-mono text-sm"
			/>
		</div>
		<Button onclick={submit} disabled={submitting || !input.trim()} class="w-full">
			{submitting ? 'Adding…' : 'Add players'}
		</Button>
	{:else}
		<div class="divide-y rounded-md border">
			{#each results as result (result.handle)}
				<div class="flex items-center gap-3 px-3 py-2 text-sm">
					{#if result.status === 'created'}
						<span class="text-green-600">✓</span>
						<span class="font-medium">{result.name}</span>
						<Badge variant="outline" class="ml-auto border-green-200 bg-green-50 text-xs text-green-700">
							created
						</Badge>
					{:else if result.status === 'skipped'}
						<span class="text-muted-foreground">–</span>
						<span class="font-medium">{result.name}</span>
						<Badge variant="secondary" class="ml-auto text-xs">already added</Badge>
					{:else}
						<span class="text-destructive">✕</span>
						<span class="text-muted-foreground">{result.handle}</span>
						<Badge variant="destructive" class="ml-auto text-xs">not found</Badge>
					{/if}
				</div>
			{/each}
		</div>
		<Button onclick={done} class="w-full">Done</Button>
	{/if}
</div>
```

- [ ] **Step 2: Commit**

```bash
cd /home/pd/private_projects/ranking_forge && git add web/src/lib/components/HandleTab.svelte
git commit -m "feat(web): add HandleTab component for bulk handle-based player import"
```

---

### Task 7: Create TournamentTab.svelte

**Files:**
- Create: `web/src/lib/components/TournamentTab.svelte`

- [ ] **Step 1: Create the file**

```svelte
<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import { Checkbox } from '$lib/components/ui/checkbox';
	import * as ScrollArea from '$lib/components/ui/scroll-area';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { makeApi } from '$lib/api';
	import { invalidateAll } from '$app/navigation';
	import type { Player, TournamentEntrant } from '$lib/types';

	let {
		projectId,
		players,
		onClose
	}: { projectId: string; players: Player[]; onClose: () => void } = $props();

	let tournamentInput = $state('');
	let loading = $state(false);
	let fetchError = $state<string | null>(null);
	let entrants = $state<TournamentEntrant[]>([]);
	let search = $state('');
	let selected = $state(new Set<number>());
	let submitting = $state(false);

	const alreadyAddedIds = $derived(
		new Set(players.flatMap((p) => p.accounts.map((a) => a.startgg_user_id)))
	);

	const filteredEntrants = $derived(
		entrants.filter((e) => {
			const q = search.toLowerCase();
			return e.name.toLowerCase().includes(q) || e.handle.toLowerCase().includes(q);
		})
	);

	const selectedCount = $derived(selected.size);
	const alreadyAddedCount = $derived(
		entrants.filter((e) => alreadyAddedIds.has(e.startgg_user_id)).length
	);

	async function fetchEntrants() {
		if (!tournamentInput.trim()) return;
		loading = true;
		fetchError = null;
		entrants = [];
		selected = new Set();
		const api = makeApi(fetch, PUBLIC_API_URL);
		const res = await api.get(
			`/projects/${projectId}/tournament-entrants?tournament=${encodeURIComponent(tournamentInput.trim())}`
		);
		loading = false;
		if (res.ok) {
			entrants = await res.json();
		} else {
			const err = await res.json().catch(() => ({ message: 'Failed to fetch entrants' }));
			fetchError = err.message;
		}
	}

	function toggleEntrant(id: number) {
		const next = new Set(selected);
		if (next.has(id)) next.delete(id);
		else next.add(id);
		selected = next;
	}

	async function addSelected() {
		const entries = entrants
			.filter((e) => selected.has(e.startgg_user_id))
			.map((e) => ({ name: e.name, startgg_user_id: e.startgg_user_id, handle: e.handle }));
		if (!entries.length) return;
		submitting = true;
		const api = makeApi(fetch, PUBLIC_API_URL);
		const res = await api.post(`/projects/${projectId}/players/bulk`, { players: entries });
		submitting = false;
		if (res.ok) {
			await invalidateAll();
			onClose();
		}
	}
</script>

<div class="space-y-3">
	<div class="flex gap-2">
		<div class="flex-1">
			<Label for="tournament-input" class="sr-only">Tournament URL or handle</Label>
			<Input
				id="tournament-input"
				bind:value={tournamentInput}
				placeholder="genesis-9 or start.gg/tournament/genesis-9"
				disabled={loading}
			/>
		</div>
		<Button onclick={fetchEntrants} disabled={loading || !tournamentInput.trim()}>
			{loading ? 'Fetching…' : 'Fetch'}
		</Button>
	</div>

	{#if fetchError}
		<p class="text-sm text-destructive">{fetchError}</p>
	{/if}

	{#if entrants.length > 0}
		<Input bind:value={search} placeholder="Search entrants…" />
		<ScrollArea.Root class="h-52 rounded-md border">
			<ScrollArea.Viewport>
				<div class="divide-y">
					{#each filteredEntrants as entrant (entrant.startgg_user_id)}
						{@const isAdded = alreadyAddedIds.has(entrant.startgg_user_id)}
						<div class="flex items-center gap-3 px-3 py-2 text-sm" class:opacity-50={isAdded}>
							<Checkbox
								id="entrant-{entrant.startgg_user_id}"
								checked={selected.has(entrant.startgg_user_id)}
								disabled={isAdded}
								onCheckedChange={() => !isAdded && toggleEntrant(entrant.startgg_user_id)}
							/>
							<label
								for="entrant-{entrant.startgg_user_id}"
								class="flex flex-1 cursor-pointer items-center gap-2"
								class:cursor-default={isAdded}
							>
								<span class="font-medium">{entrant.name}</span>
								<span class="text-muted-foreground">{entrant.handle}</span>
							</label>
							{#if isAdded}
								<Badge variant="secondary" class="text-xs">already added</Badge>
							{/if}
						</div>
					{/each}
				</div>
			</ScrollArea.Viewport>
			<ScrollArea.Scrollbar orientation="vertical" />
		</ScrollArea.Root>
		<div class="flex items-center justify-between">
			<span class="text-sm text-muted-foreground">
				{selectedCount} selected · {alreadyAddedCount} already added
			</span>
			<Button onclick={addSelected} disabled={selectedCount === 0 || submitting}>
				{submitting ? 'Adding…' : `Add ${selectedCount} player${selectedCount === 1 ? '' : 's'}`}
			</Button>
		</div>
	{/if}
</div>
```

- [ ] **Step 2: Commit**

```bash
cd /home/pd/private_projects/ranking_forge && git add web/src/lib/components/TournamentTab.svelte
git commit -m "feat(web): add TournamentTab component for tournament-based bulk import"
```

---

### Task 8: Create AddPlayersDialog.svelte

**Files:**
- Create: `web/src/lib/components/AddPlayersDialog.svelte`

- [ ] **Step 1: Create the file**

```svelte
<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Tabs from '$lib/components/ui/tabs';
	import { Button } from '$lib/components/ui/button';
	import TournamentTab from './TournamentTab.svelte';
	import HandleTab from './HandleTab.svelte';
	import NameTab from './NameTab.svelte';
	import type { Player } from '$lib/types';

	let { projectId, players }: { projectId: string; players: Player[] } = $props();

	let open = $state(false);

	function close() {
		open = false;
	}
</script>

<Button onclick={() => (open = true)}>Add players</Button>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Add players</Dialog.Title>
		</Dialog.Header>
		<Tabs.Root value="tournament">
			<Tabs.List class="w-full">
				<Tabs.Trigger value="tournament" class="flex-1">From tournament</Tabs.Trigger>
				<Tabs.Trigger value="handle" class="flex-1">By handle</Tabs.Trigger>
				<Tabs.Trigger value="name" class="flex-1">By name</Tabs.Trigger>
			</Tabs.List>
			<Tabs.Content value="tournament" class="mt-4">
				<TournamentTab {projectId} {players} onClose={close} />
			</Tabs.Content>
			<Tabs.Content value="handle" class="mt-4">
				<HandleTab {projectId} onClose={close} />
			</Tabs.Content>
			<Tabs.Content value="name" class="mt-4">
				<NameTab {projectId} />
			</Tabs.Content>
		</Tabs.Root>
	</Dialog.Content>
</Dialog.Root>
```

- [ ] **Step 2: Commit**

```bash
cd /home/pd/private_projects/ranking_forge && git add web/src/lib/components/AddPlayersDialog.svelte
git commit -m "feat(web): add AddPlayersDialog shell with three-tab layout"
```

---

### Task 9: Update the Players page

**Files:**
- Modify: `web/src/routes/projects/[id]/players/+page.svelte`

This is the largest change: remove the inline form, add the dialog button, add inline rename, update the link-account dialog, and fix `account.slug` → `account.handle`.

- [ ] **Step 1: Replace the file contents** with:

```svelte
<script lang="ts">
	import { enhance } from '$app/forms';
	import { invalidateAll } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Badge } from '$lib/components/ui/badge';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Empty from '$lib/components/ui/empty';
	import AddPlayersDialog from '$lib/components/AddPlayersDialog.svelte';

	let { data, form } = $props();

	let linkDialogOpen = $state(false);
	let linkingPid = $state('');
	let linkingName = $state('');
	let editingPid = $state('');
	let editingName = $state('');

	function openLinkDialog(pid: string, name: string) {
		linkingPid = pid;
		linkingName = name;
		linkDialogOpen = true;
	}

	function startEdit(pid: string, name: string) {
		editingPid = pid;
		editingName = name;
	}

	function cancelEdit() {
		editingPid = '';
		editingName = '';
	}
</script>

<div class="space-y-6">
	<div class="flex items-center justify-between">
		<h2 class="text-lg font-semibold">Players</h2>
		<AddPlayersDialog projectId={data.project.id} players={data.players} />
	</div>

	{#if data.players.length === 0}
		<Empty.Root>
			<Empty.Header>
				<Empty.Title>No players yet</Empty.Title>
				<Empty.Description>Use "Add players" to get started.</Empty.Description>
			</Empty.Header>
		</Empty.Root>
	{:else}
		<div class="space-y-2">
			{#each data.players as player (player.id)}
				<div class="rounded-md border border-border p-3">
					{#if editingPid === player.id}
						<form
							method="POST"
							action="?/renamePlayer"
							use:enhance={() => {
								return async ({ result, update }) => {
									if (result.type === 'success') {
										cancelEdit();
										await invalidateAll();
									} else {
										await update();
									}
								};
							}}
						>
							<input type="hidden" name="pid" value={player.id} />
							<div class="flex items-center gap-2">
								<Input name="name" bind:value={editingName} class="flex-1" />
								<Button type="submit" size="sm">Save</Button>
								<Button type="button" variant="ghost" size="sm" onclick={cancelEdit}>Cancel</Button>
							</div>
							{#if form?.renameError && form.renamePid === player.id}
								<p class="mt-1 text-sm text-destructive">{form.renameError}</p>
							{/if}
						</form>
					{:else}
						<div class="flex items-start justify-between">
							<div class="space-y-1">
								<p class="font-medium">{player.name}</p>
								<div class="flex flex-wrap gap-1">
									{#each player.accounts as account (account.id)}
										<form
											method="POST"
											action="?/unlinkAccount"
											use:enhance={() => {
												return async ({ result, update }) => {
													if (result.type === 'success') {
														await invalidateAll();
													} else {
														await update();
													}
												};
											}}
											class="inline-flex"
										>
											<input type="hidden" name="pid" value={player.id} />
											<input type="hidden" name="aid" value={account.id} />
											<Badge variant="secondary" class="gap-1 pr-1">
												{account.display_name ?? account.handle}
												<button
													type="submit"
													class="ml-0.5 rounded-full hover:bg-muted"
													title="Remove">×</button
												>
											</Badge>
										</form>
									{/each}
									<button
										type="button"
										onclick={() => openLinkDialog(player.id, player.name)}
										class="text-xs text-muted-foreground underline hover:text-foreground"
									>+ add account</button>
								</div>
							</div>
							<div class="flex gap-1">
								<Button
									type="button"
									variant="ghost"
									size="sm"
									onclick={() => startEdit(player.id, player.name)}
								>Edit</Button>
								<form method="POST" action="?/deletePlayer" use:enhance>
									<input type="hidden" name="pid" value={player.id} />
									<Button
										type="submit"
										variant="ghost"
										size="sm"
										class="text-destructive hover:text-destructive"
										onclick={(e: MouseEvent) => {
											if (!confirm(`Remove ${player.name}?`)) e.preventDefault();
										}}
									>Remove</Button>
								</form>
							</div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/if}
</div>

<!-- Link account dialog -->
<Dialog.Root bind:open={linkDialogOpen}>
	<Dialog.Content>
		<Dialog.Header>
			<Dialog.Title>Link start.gg account</Dialog.Title>
			<Dialog.Description>Add a start.gg account for {linkingName}</Dialog.Description>
		</Dialog.Header>
		{#if form?.linkError && form.linkPid === linkingPid}
			<p class="text-sm text-destructive">{form.linkError}</p>
		{/if}
		<form
			method="POST"
			action="?/linkAccount"
			use:enhance={() => {
				return async ({ result, update }) => {
					if (result.type === 'success') {
						linkDialogOpen = false;
						await invalidateAll();
					} else {
						await update();
					}
				};
			}}
			class="space-y-4"
		>
			<input type="hidden" name="pid" value={linkingPid} />
			<div class="space-y-2">
				<Label for="handle">start.gg handle</Label>
				<Input id="handle" name="handle" placeholder="mang0" required />
				<p class="text-xs text-muted-foreground">Accepts bare handle, full slug, or full URL</p>
			</div>
			<div class="flex justify-end gap-2">
				<Button type="button" variant="ghost" onclick={() => (linkDialogOpen = false)}>Cancel</Button>
				<Button type="submit">Link</Button>
			</div>
		</form>
	</Dialog.Content>
</Dialog.Root>
```

- [ ] **Step 2: Verify project id is available**

The layout's load (`+layout.server.ts`) returns `{ project }`, so `data.project.id` is available in the page. The `AddPlayersDialog` line in Step 1 already uses `data.project.id`:

```svelte
<AddPlayersDialog projectId={data.project.id} players={data.players} />
```

No changes needed here — just confirming it's correct.

- [ ] **Step 3: Verify TypeScript**

```bash
cd /home/pd/private_projects/ranking_forge/web && npx tsc --noEmit 2>&1 | head -30
```

Expected: zero errors.

- [ ] **Step 4: Commit**

```bash
cd /home/pd/private_projects/ranking_forge && git add web/src/routes/projects/[id]/players/+page.svelte
git commit -m "feat(web): replace inline form with AddPlayersDialog, add inline rename, fix handle field"
```

---

### Task 10: Run the full test suite

- [ ] **Step 1: Run e2e tests only**

```bash
cd /home/pd/private_projects/ranking_forge/web && npm run test:e2e 2>&1 | tail -30
```

Expected: all tests pass including the four new ones added in Task 3.

If a test fails with "Cannot find module '$lib/components/ui/textarea'": the textarea component may need to be added explicitly:

```bash
cd /home/pd/private_projects/ranking_forge/web && npx shadcn-svelte@latest add --yes --overwrite textarea
```

Then re-run.

- [ ] **Step 2: Run unit tests**

```bash
cd /home/pd/private_projects/ranking_forge/web && npm run test:unit 2>&1 | tail -20
```

Expected: all pass (no unit tests touch the new components).

- [ ] **Step 3: Run the full suite**

```bash
cd /home/pd/private_projects/ranking_forge && bash test.sh 2>&1 | tail -40
```

Expected: all backend + frontend tests pass.

- [ ] **Step 4: Final commit if any test-only fixes were needed**

```bash
cd /home/pd/private_projects/ranking_forge && git add -p && git commit -m "fix(web): address test failures found during full suite run"
```

Only run Step 4 if fixes were needed. Skip if everything passed cleanly.
