# Back Button Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the player detail page back button so it navigates to the previous in-app page, falling back to the players list when arrived via direct link.

**Architecture:** A new `previousPage` Svelte store is updated by `afterNavigate` in the root layout on every client-side navigation. The player detail page reads the store at click time: if non-null it calls `goto($previousPage)`, otherwise falls back to `/projects/[id]/players`.

**Tech Stack:** SvelteKit (`afterNavigate`, `goto` from `$app/navigation`), Svelte 5 runes, Playwright e2e tests.

---

### Task 1: Write failing e2e tests

**Files:**
- Modify: `web/tests/player.test.ts`

- [ ] **Step 1: Add two test cases to `web/tests/player.test.ts`**

Append these two tests after the existing tests in the file:

```typescript
test('back button returns to previous in-app page', async ({ page }) => {
	await page.goto('/projects/proj-1/players');
	await page.getByRole('link', { name: 'Alice' }).click();
	await expect(page).toHaveURL(/\/projects\/proj-1\/players\/player-1/);
	await page.getByRole('button', { name: '← Back' }).click();
	await expect(page).toHaveURL('/projects/proj-1/players');
});

test('back button falls back to players list on direct link', async ({ page }) => {
	await page.goto('/projects/proj-1/players/player-1');
	await page.getByRole('button', { name: '← Back' }).click();
	await expect(page).toHaveURL('/projects/proj-1/players');
});
```

- [ ] **Step 2: Run the new tests and verify they fail**

```bash
cd web && npx playwright test tests/player.test.ts --grep "back button"
```

Expected: both tests FAIL. The first may pass or hang (depends on browser history state), the second will navigate off-site or to a blank page. Either way, neither reliably lands on `/projects/proj-1/players`.

---

### Task 2: Create the navigation store

**Files:**
- Create: `web/src/lib/stores/navigation.ts`

- [ ] **Step 1: Create `web/src/lib/stores/navigation.ts`**

```typescript
import { writable } from 'svelte/store';

export const previousPage = writable<string | null>(null);
```

---

### Task 3: Wire `afterNavigate` in the root layout

**Files:**
- Modify: `web/src/routes/+layout.svelte`

- [ ] **Step 1: Add `afterNavigate` import and call to `web/src/routes/+layout.svelte`**

Replace the existing `<script lang="ts">` block with:

```svelte
<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { afterNavigate, goto } from '$app/navigation';
	import { ModeWatcher } from 'mode-watcher';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { previousPage } from '$lib/stores/navigation';

	let { children, data } = $props();

	afterNavigate((navigation) => {
		previousPage.set(navigation.from?.url.pathname ?? null);
	});

	async function logout() {
		await fetch(`${PUBLIC_API_URL}/auth/logout`, { method: 'POST', credentials: 'include' });
		await goto('/login');
	}
</script>
```

---

### Task 4: Update the back button on the player detail page

**Files:**
- Modify: `web/src/routes/projects/[id]/players/[player_id]/+page.svelte`

- [ ] **Step 1: Replace the `<script>` block in `web/src/routes/projects/[id]/players/[player_id]/+page.svelte`**

Replace the existing `<script lang="ts">` block with:

```svelte
<script lang="ts">
	import type { SetRecord } from '$lib/types';
	import SetDetailModal from '$lib/components/SetDetailModal.svelte';
	import * as Card from '$lib/components/ui/card';
	import { ScrollArea } from '$lib/components/ui/scroll-area';
	import * as Empty from '$lib/components/ui/empty';
	import { Button } from '$lib/components/ui/button';
	import * as Table from '$lib/components/ui/table';
	import { winRate, toOrdinal, formatDate } from '$lib/utils';
	import { goto } from '$app/navigation';
	import { previousPage } from '$lib/stores/navigation';

	let { data } = $props();

	let selectedSet = $state<SetRecord | null>(null);
	let selectedIsWin = $state(false);

	function openModal(set: SetRecord, isWin: boolean) {
		selectedSet = set;
		selectedIsWin = isWin;
	}

	function goBack() {
		if ($previousPage) {
			goto($previousPage);
		} else {
			goto(`/projects/${data.projectId}/players`);
		}
	}

	const wins = $derived(data.stats.wins);
	const losses = $derived(data.stats.losses);
	const winRateStr = $derived(winRate(wins.length, losses.length, '0%'));
	const tournamentCount = $derived(data.tournaments.length);
</script>
```

- [ ] **Step 2: Update the back button in the template**

Find this line in the template section of the same file:

```svelte
<Button variant="link" class="px-0" onclick={() => history.back()}>← Back</Button>
```

Replace it with:

```svelte
<Button variant="link" class="px-0" onclick={goBack}>← Back</Button>
```

---

### Task 5: Run tests and commit

**Files:** none

- [ ] **Step 1: Run the full e2e player test suite**

```bash
cd web && npx playwright test tests/player.test.ts
```

Expected: all 4 tests pass (the 2 existing + 2 new).

- [ ] **Step 2: Run the full test suite to check for regressions**

```bash
cd /home/pd/private_projects/ranking_forge/web && npm run test:e2e
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/stores/navigation.ts \
        web/src/routes/+layout.svelte \
        web/src/routes/projects/\[id\]/players/\[player_id\]/+page.svelte \
        web/tests/player.test.ts
git commit -m "fix: back button on player page respects in-app navigation history"
```
