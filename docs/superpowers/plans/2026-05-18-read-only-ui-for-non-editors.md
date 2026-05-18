# Read-only UI for Non-editors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hide edit-only UI (drag handles, rank editing, Save button, event checkboxes, bulk action buttons) from users whose `user_role` is `'viewer'` or `null`.

**Architecture:** Each affected page derives `canEdit` from `data.project.user_role` (already available via layout server load), then gates edit UI behind `{#if canEdit}`. The Ranking page splits into two `{#each}` branches (editor with `dragHandleZone`, viewer with a plain list). The Tournaments page hides checkboxes, bulk actions, and relabels the filter toggle.

**Tech Stack:** SvelteKit + Svelte 5 runes, Playwright for e2e tests, mock API in `web/tests/mock-api.js`.

**Spec:** `docs/superpowers/specs/2026-05-18-read-only-ui-for-non-editors-design.md`

---

### Task 1: Add viewer project fixtures to mock API

**Files:**
- Modify: `web/tests/mock-api.js`

- [ ] **Step 1: Add viewer project constant** — after the `MOCK_PROJECTS` constant, add:

```js
const MOCK_VIEWER_PROJECT = {
	id: 'proj-viewer',
	name: 'SSBM Power Ranking',
	game_id: 1,
	game_name: 'Super Smash Bros. Melee',
	created_at: '2026-01-01T00:00:00Z',
	published: true,
	user_role: 'viewer'
};
```

- [ ] **Step 2: Update the single-project GET handler** — find the block starting with `const projectMatch = path.match(...)` and replace it:

```js
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
```

- [ ] **Step 3: Update the tournaments GET handler** — find the block starting with `const tournamentsMatch = path.match(...)` and replace it:

```js
const tournamentsMatch = path.match(/^\/projects\/([^/]+)\/tournaments$/);
if (tournamentsMatch && req.method === 'GET') {
    const projectId = tournamentsMatch[1];
    const hasTournaments = projectId === 'proj-tournaments' || projectId === 'proj-viewer-tournaments';
    respond(res, 200, hasTournaments ? MOCK_TOURNAMENTS : []);
    return;
}
```

- [ ] **Step 4: Commit**

```bash
git add web/tests/mock-api.js
git commit -m "test: add viewer project fixtures to mock API"
```

---

### Task 2: Add failing Playwright tests for ranking viewer mode

**Files:**
- Modify: `web/tests/projects.test.ts`

- [ ] **Step 1: Add two tests at the end of the file**

```ts
test('ranking page hides edit controls for viewer role', async ({ page }) => {
	await page.goto('/projects/proj-viewer/ranking');
	await expect(page.getByText('Alice')).toBeVisible();
	// No Save button
	await expect(page.getByRole('button', { name: 'Save' })).not.toBeVisible();
	// No drag handle character
	await expect(page.locator('text=⠿')).not.toBeVisible();
	// Rank 1 is not a button
	await expect(page.getByRole('button', { name: '1' })).not.toBeVisible();
});

test('ranking page shows edit controls for owner role', async ({ page }) => {
	await page.goto('/projects/proj-1/ranking');
	await expect(page.getByRole('button', { name: 'Save' })).toBeVisible();
	await expect(page.locator('text=⠿').first()).toBeVisible();
	await expect(page.getByRole('button', { name: '1' })).toBeVisible();
});
```

- [ ] **Step 2: Run tests to verify the new ones fail**

```bash
cd web && npm run test:e2e -- --grep "ranking page hides"
```

Expected: FAIL — Save button and drag handles are currently shown to everyone.

---

### Task 3: Implement ranking page read-only mode

**Files:**
- Modify: `web/src/routes/projects/[id]/ranking/+page.svelte`

- [ ] **Step 1: Add `canEdit` derived** — in the `<script>` block, after the existing `let` declarations, add:

```ts
const canEdit = $derived(
	data.project.user_role === 'editor' || data.project.user_role === 'owner'
);
```

- [ ] **Step 2: Gate the save area** — find this block in the template:

```svelte
<div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold">Ranking</h2>
    <div class="flex items-center gap-3">
        {#if hasChanges && saveStatus !== 'saved'}
            <span class="text-sm text-muted-foreground">Unsaved changes</span>
        {/if}
        <Button
            onclick={save}
            disabled={!hasChanges || saveStatus === 'saving'}
            size="sm"
            variant={saveStatus === 'saved' ? 'outline' : 'default'}
        >
            {saveStatus === 'saving' ? 'Saving…' : saveStatus === 'saved' ? 'Saved ✓' : 'Save'}
        </Button>
    </div>
</div>
```

Replace with:

```svelte
<div class="flex items-center justify-between">
    <h2 class="text-lg font-semibold">Ranking</h2>
    {#if canEdit}
        <div class="flex items-center gap-3">
            {#if hasChanges && saveStatus !== 'saved'}
                <span class="text-sm text-muted-foreground">Unsaved changes</span>
            {/if}
            <Button
                onclick={save}
                disabled={!hasChanges || saveStatus === 'saving'}
                size="sm"
                variant={saveStatus === 'saved' ? 'outline' : 'default'}
            >
                {saveStatus === 'saving' ? 'Saving…' : saveStatus === 'saved' ? 'Saved ✓' : 'Save'}
            </Button>
        </div>
    {/if}
</div>
```

- [ ] **Step 3: Replace the item list with two branches** — find the `<div class="flex max-w-xl flex-col gap-1" use:dragHandleZone=...>` block through to its closing `</div>`. Replace the entire block with:

```svelte
{#if canEdit}
    <div
        class="flex max-w-xl flex-col gap-1"
        use:dragHandleZone={{ items, flipDurationMs: 0 }}
        onconsider={handleConsider}
        onfinalize={handleFinalize}
    >
        {#each items as item, i (item.id)}
            {@const s = statsMap[item.id]}
            {@const moved = isMoved(item.id, i)}
            <div
                class="flex items-center gap-3 rounded-md border px-3 py-2.5 text-sm transition-colors
                {moved ? 'border-primary/40 bg-primary/5' : 'bg-card'}"
            >
                <span
                    use:dragHandle
                    class="cursor-grab select-none text-base text-muted-foreground active:cursor-grabbing"
                >
                    ⠿
                </span>

                {#if editingId === item.id}
                    <Input
                        bind:ref={editInput}
                        type="number"
                        class="h-7 w-12 px-1 text-center text-xs [appearance:textfield]"
                        bind:value={editingValue}
                        onblur={commitEdit}
                        onkeydown={onRankKeydown}
                    />
                {:else}
                    <Button
                        variant="ghost"
                        size="sm"
                        class="h-7 w-8 rounded p-0 text-xs font-normal text-muted-foreground"
                        onclick={() => startEdit(item.id, i + 1)}
                    >
                        {i + 1}
                    </Button>
                {/if}

                <span class="flex-1 font-semibold">{item.name}</span>

                {#if s}
                    <span class="text-xs text-muted-foreground">{wlRecord(s)}</span>
                    <span class="min-w-[36px] text-right text-xs font-semibold">{winRate(s.wins.length, s.losses.length)}</span>
                {/if}
            </div>
        {/each}
    </div>
{:else}
    <div class="flex max-w-xl flex-col gap-1">
        {#each items as item, i (item.id)}
            {@const s = statsMap[item.id]}
            <div class="flex items-center gap-3 rounded-md border bg-card px-3 py-2.5 text-sm">
                <span class="w-8 text-center text-xs text-muted-foreground">{i + 1}</span>
                <span class="flex-1 font-semibold">{item.name}</span>
                {#if s}
                    <span class="text-xs text-muted-foreground">{wlRecord(s)}</span>
                    <span class="min-w-[36px] text-right text-xs font-semibold">{winRate(s.wins.length, s.losses.length)}</span>
                {/if}
            </div>
        {/each}
    </div>
{/if}
```

- [ ] **Step 4: Gate the hint text** — find:

```svelte
<p class="text-xs text-muted-foreground">
    Click the rank number to edit · Drag ⠿ to reorder · Click Save to persist
</p>
```

Replace with:

```svelte
{#if canEdit}
    <p class="text-xs text-muted-foreground">
        Click the rank number to edit · Drag ⠿ to reorder · Click Save to persist
    </p>
{/if}
```

- [ ] **Step 5: Run all e2e tests to confirm pass**

```bash
cd web && npm run test:e2e
```

Expected: all tests pass, including the two new ranking viewer tests.

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/\[id\]/ranking/+page.svelte web/tests/projects.test.ts
git commit -m "feat: hide ranking edit controls from non-editors"
```

---

### Task 4: Add failing Playwright tests for tournament viewer mode

**Files:**
- Modify: `web/tests/projects.test.ts`

- [ ] **Step 1: Add two tests at the end of the file**

```ts
test('tournaments page hides checkboxes and bulk actions for viewer role', async ({ page }) => {
	await page.goto('/projects/proj-viewer-tournaments/tournaments');
	await page.waitForLoadState('networkidle');
	await expect(page.getByText('Genesis 10')).toBeVisible();
	// No checkboxes in event rows
	await expect(page.getByRole('checkbox')).not.toBeVisible();
	// Open the filter panel — button says "Filters", not "Filters & Actions"
	await expect(page.getByRole('button', { name: /Filters & Actions/ })).not.toBeVisible();
	await page.getByRole('button', { name: /Filters/ }).click();
	// No bulk action buttons
	await expect(page.getByRole('button', { name: /Include all visible/ })).not.toBeVisible();
	await expect(page.getByRole('button', { name: /Exclude all visible/ })).not.toBeVisible();
});

test('tournaments page shows checkboxes and bulk actions for owner role', async ({ page }) => {
	await page.goto('/projects/proj-tournaments/tournaments');
	await page.waitForLoadState('networkidle');
	await expect(page.getByRole('checkbox').first()).toBeVisible();
	await page.getByRole('button', { name: /Filters & Actions/ }).click();
	await expect(page.getByRole('button', { name: /Include all visible/ })).toBeVisible();
	await expect(page.getByRole('button', { name: /Exclude all visible/ })).toBeVisible();
});
```

- [ ] **Step 2: Run tests to verify the new ones fail**

```bash
cd web && npm run test:e2e -- --grep "tournaments page hides"
```

Expected: FAIL — checkboxes and bulk actions are currently shown to all roles.

---

### Task 5: Implement tournament page changes

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Add `canEdit` derived** — in the `<script>` block, after the existing `let` declarations (e.g. after `let editingId` or similar), add:

```ts
const canEdit = $derived(
	data.project.user_role === 'editor' || data.project.user_role === 'owner'
);
```

- [ ] **Step 2: Change the filter toggle label** — find:

```svelte
<Button {...props} variant="outline" size="sm">
    ⚙ Filters &amp; Actions {filterOpen ? '▲' : '▼'}
</Button>
```

Replace with:

```svelte
<Button {...props} variant="outline" size="sm">
    ⚙ {canEdit ? 'Filters & Actions' : 'Filters'} {filterOpen ? '▲' : '▼'}
</Button>
```

- [ ] **Step 3: Gate the bulk actions section** — find the comment `<!-- Divider + bulk actions -->` and the `<div class="flex items-center justify-between border-t border-border pt-3">` block that follows it. Wrap the entire block (comment through closing `</div>`) with `{#if canEdit}`:

```svelte
{#if canEdit}
    <!-- Divider + bulk actions -->
    <div class="flex items-center justify-between border-t border-border pt-3">
        <span class="text-xs text-muted-foreground">
            Bulk actions apply to {visibleEventCount} visible event{visibleEventCount !== 1 ? 's' : ''}
        </span>
        <div class="flex gap-2">
            <Button variant="outline" size="sm" onclick={() => bulkSetIncluded(true)}>
                ✓ Include all visible
            </Button>
            <Button
                variant="outline"
                size="sm"
                class="border-destructive text-destructive hover:bg-destructive/10"
                onclick={() => bulkSetIncluded(false)}
            >
                ✕ Exclude all visible
            </Button>
        </div>
    </div>
{/if}
```

- [ ] **Step 4: Gate the event row checkbox and remove interactive classes from Label** — find the `<Label class="flex cursor-pointer items-center justify-between px-4 py-2 hover:bg-accent/50">` block and replace through its closing `</Label>`:

```svelte
<Label
    class="flex items-center justify-between px-4 py-2
        {canEdit ? 'cursor-pointer hover:bg-accent/50' : ''}"
>
    <div>
        <span class="text-sm">{event.name}</span>
        {#if event.num_entrants}
            <span class="ml-2 text-xs text-muted-foreground">{event.num_entrants} entrants</span>
        {/if}
    </div>
    {#if canEdit}
        <Checkbox
            checked={event.included}
            onCheckedChange={() => handleToggle(data.project.id, event)}
        />
    {/if}
</Label>
```

- [ ] **Step 5: Run all e2e tests**

```bash
cd web && npm run test:e2e
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/\[id\]/tournaments/+page.svelte web/tests/projects.test.ts
git commit -m "feat: hide tournament edit controls from non-editors"
```

---

### Task 6: Final verification

- [ ] **Step 1: Run the full test suite**

```bash
cd /home/pd/private_projects/ranking_forge && bash test.sh
```

Expected: PASS for all sections (backend + frontend unit + frontend e2e).
