# Reset All Filters Button Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Clear filters" button to the top of the tournament filter panel that resets all six filter dimensions in one click.

**Architecture:** A single `resetAllFilters()` function in `+page.svelte` resets all six filter state variables to their defaults. A thin header row is prepended inside the collapsible filter panel, with a "Filters" label on the left and the "Clear filters" button on the right — mirroring the bracket popover's existing label+reset pattern.

**Tech Stack:** SvelteKit, Svelte 5 runes (`$state`), Playwright (e2e tests), Vitest (unit tests, not applicable here — no extractable filter logic to unit test for this feature).

---

### Task 1: Add mock tournament data to the Playwright mock API

**Files:**
- Modify: `web/tests/mock-api.js`

The current mock returns `[]` for all tournament requests, so the filter panel never renders. We need one project with tournament data to test against.

- [ ] **Step 1: Add `MOCK_TOURNAMENTS` constant**

Open `web/tests/mock-api.js`. After the `MOCK_PLAYERS` array (around line 18), add:

```js
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
```

- [ ] **Step 2: Return mock tournaments for `proj-tournaments`**

Find the existing `tournamentsMatch` handler (around line 208):

```js
const tournamentsMatch = path.match(/^\/projects\/([^/]+)\/tournaments$/);
if (tournamentsMatch && req.method === 'GET') {
    respond(res, 200, []);
    return;
}
```

Replace with:

```js
const tournamentsMatch = path.match(/^\/projects\/([^/]+)\/tournaments$/);
if (tournamentsMatch && req.method === 'GET') {
    const projectId = tournamentsMatch[1];
    respond(res, 200, projectId === 'proj-tournaments' ? MOCK_TOURNAMENTS : []);
    return;
}
```

- [ ] **Step 3: Verify the mock API starts correctly**

```bash
cd web && node tests/mock-api.js &
curl -s http://localhost:9999/projects/proj-tournaments/tournaments | node -e "const d=require('fs').readFileSync('/dev/stdin','utf8');console.log(JSON.parse(d).length)"
kill %1
```

Expected output: `1`

---

### Task 2: Write the failing Playwright test

**Files:**
- Modify: `web/tests/projects.test.ts`

- [ ] **Step 1: Add the test**

At the end of `web/tests/projects.test.ts`, add:

```ts
test('tournaments filter panel has Clear filters button that resets search', async ({ page }) => {
	await page.goto('/projects/proj-tournaments/tournaments');
	// Open filter panel
	await page.getByRole('button', { name: /Filters & Actions/ }).click();
	// Type in the search box
	await page.getByPlaceholder('Search tournament or event name…').fill('melee');
	await expect(page.getByPlaceholder('Search tournament or event name…')).toHaveValue('melee');
	// Click "Clear filters"
	await page.getByRole('button', { name: 'Clear filters' }).click();
	// Search should be cleared
	await expect(page.getByPlaceholder('Search tournament or event name…')).toHaveValue('');
});
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd web && npm run test:e2e -- --grep "Clear filters"
```

Expected: FAIL — `getByRole('button', { name: 'Clear filters' })` not found (button doesn't exist yet).

---

### Task 3: Implement the feature in `+page.svelte`

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Add `resetAllFilters()` to the script block**

In the `<script>` block, after the existing `resetBracketFilter()` function (line 81–85), add:

```ts
function resetAllFilters() {
	search      = '';
	venueFilter = 'all';
	minEntrants = null;
	maxEntrants = null;
	dateFrom    = '';
	dateTo      = '';
	eventType   = 'all';
	bracketFilter = Object.fromEntries(
		BRACKET_TYPES.map(t => [t, 'neutral' as BracketTypeState])
	);
}
```

- [ ] **Step 2: Add the header row inside the filter panel**

Find the opening div of the collapsible filter panel (line 233):

```svelte
<div class="rounded-md border border-border bg-muted/30 p-4 space-y-3">
    <!-- Row 1: search + venue -->
```

Insert a new header row as its first child (before Row 1):

```svelte
<div class="rounded-md border border-border bg-muted/30 p-4 space-y-3">
    <!-- Header: label + clear button -->
    <div class="flex items-center justify-between">
        <span class="text-xs font-medium text-muted-foreground uppercase tracking-wide">Filters</span>
        <button
            type="button"
            onclick={resetAllFilters}
            class="text-xs text-muted-foreground hover:text-foreground"
        >Clear filters</button>
    </div>

    <!-- Row 1: search + venue -->
```

---

### Task 4: Run tests and commit

**Files:** none

- [ ] **Step 1: Run the e2e test suite**

```bash
cd web && npm run test:e2e
```

Expected: all tests pass, including the new "Clear filters" test.

- [ ] **Step 2: Run the unit test suite**

```bash
cd web && npm run test:unit
```

Expected: all tests pass (no regressions in `filter.test.ts`).

- [ ] **Step 3: Commit**

```bash
git add web/tests/mock-api.js web/tests/projects.test.ts \
        web/src/routes/projects/[id]/tournaments/+page.svelte
git commit -m "feat(tournaments): add Clear filters button to filter panel"
```
