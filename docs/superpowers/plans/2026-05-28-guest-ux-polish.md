# Guest UX Polish for Public Projects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish the experience for unauthenticated visitors viewing a published project: show a sign-up banner, fix the back link, and add a copy-link affordance in project settings.

**Architecture:** Three frontend-only changes — one to `+layout.svelte` (banner + back link), one to `settings/+page.svelte` (copy-link). No backend changes. `page.data.user` from the root layout is already available in all child layouts via `$app/state`.

**Tech Stack:** SvelteKit (Svelte 5 runes), Tailwind CSS v4, shadcn-svelte, Playwright (e2e tests)

---

### Task 1: Add mock fixtures for guest and published-owner projects

**Files:**
- Modify: `web/tests/mock-api.js`

- [ ] **Step 1: Add MOCK_GUEST_PROJECT constant**

Add after the existing `MOCK_VIEWER_PROJECT` constant (around line 27):

```javascript
const MOCK_GUEST_PROJECT = {
	id: 'proj-guest',
	name: 'Public Ranking',
	game_id: 1,
	game_name: 'Super Smash Bros. Melee',
	created_at: '2026-01-01T00:00:00Z',
	published: true,
	user_role: null,
	owner_has_startgg_key: true
};

const MOCK_PUBLISHED_OWNER_PROJECT = {
	id: 'proj-published',
	name: 'Published Owner Ranking',
	game_id: 1,
	game_name: 'Super Smash Bros. Melee',
	created_at: '2026-01-01T00:00:00Z',
	published: true,
	user_role: 'owner',
	owner_has_startgg_key: true
};
```

- [ ] **Step 2: Extend the project GET route**

Find the `projectMatch` block (around line 287) and add cases for the two new IDs:

```javascript
const projectMatch = path.match(/^\/projects\/([^/]+)$/);
if (projectMatch && req.method === 'GET') {
	const projectId = projectMatch[1];
	if (projectId === 'proj-guest') {
		respond(res, 200, MOCK_GUEST_PROJECT);
	} else if (projectId === 'proj-published') {
		respond(res, 200, MOCK_PUBLISHED_OWNER_PROJECT);
	} else if (projectId === 'proj-viewer' || projectId === 'proj-viewer-tournaments') {
		respond(res, 200, { ...MOCK_VIEWER_PROJECT, id: projectId });
	} else {
		respond(res, 200, MOCK_PROJECTS[0]);
	}
	return;
}
```

- [ ] **Step 3: Commit**

```bash
git add web/tests/mock-api.js
git commit -m "test(e2e): add guest and published-owner project fixtures to mock API"
```

---

### Task 2: Write failing tests — guest banner and back link

**Files:**
- Modify: `web/tests/projects.test.ts`

The file already imports `test as base` from `@playwright/test` and extends it with a pre-authenticated context. Use `base(...)` directly (no cookie) for guest tests.

- [ ] **Step 1: Add guest banner and back link tests**

Add these tests at the end of `web/tests/projects.test.ts`:

```typescript
// Guest (unauthenticated) tests — use base directly, no session cookie
base('guest sees banner on published project', async ({ page }) => {
	await page.goto('/projects/proj-guest/stats');
	await expect(page.getByText("You're viewing a shared project")).toBeVisible();
	await expect(page.getByRole('link', { name: 'Sign up' })).toHaveAttribute('href', '/register');
});

base('guest sees "← Home" back link instead of "← Projects"', async ({ page }) => {
	await page.goto('/projects/proj-guest/stats');
	await expect(page.getByRole('link', { name: '← Home' })).toHaveAttribute('href', '/');
	await expect(page.getByText('← Projects')).not.toBeVisible();
});

test('authenticated user does not see guest banner', async ({ page }) => {
	await page.goto('/projects/proj-1/stats');
	await expect(page.getByText("You're viewing a shared project")).not.toBeVisible();
});

test('authenticated user sees "← Projects" back link', async ({ page }) => {
	await page.goto('/projects/proj-1/stats');
	await expect(page.getByRole('link', { name: '← Projects' })).toHaveAttribute('href', '/projects');
});
```

- [ ] **Step 2: Run the new tests — verify they fail**

```bash
cd web && npx playwright test projects.test.ts --grep "guest sees banner|guest sees.*Home|authenticated user does not see|authenticated user sees.*Projects" 2>&1 | tail -20
```

Expected: 4 failures (banner and back link not yet implemented).

---

### Task 3: Implement guest banner and back link fix

**Files:**
- Modify: `web/src/routes/projects/[id]/+layout.svelte`

`page` from `$app/state` is already imported in this file. `page.data.user` contains the authenticated user (or `null` for guests) from the root layout.

- [ ] **Step 1: Replace the layout contents**

Replace the full contents of `web/src/routes/projects/[id]/+layout.svelte` with:

```svelte
<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { Separator } from "$lib/components/ui/separator";
  import * as Tabs from "$lib/components/ui/tabs";

  let { children, data } = $props();

  const allTabs = [
    { label: "Players", href: "players", minRole: "editor" as const },
    { label: "Import", href: "import", minRole: "editor" as const },
    { label: "Tournaments", href: "tournaments", minRole: null },
    { label: "Stats", href: "stats", minRole: null },
    { label: "H2H", href: "h2h", minRole: null },
    { label: "Ranking", href: "ranking", minRole: null },
    { label: "Settings", href: "settings", minRole: "owner" as const },
  ];

  const tabs = $derived(
    allTabs.filter((t) => {
      const role = data.project.user_role;
      if (t.minRole === null) return true;
      if (t.minRole === "editor") return role === "editor" || role === "owner";
      if (t.minRole === "owner") return role === "owner";
      return false;
    }),
  );

  function tabHref(slug: string) {
    return `/projects/${data.project.id}/${slug}`;
  }

  const currentTab = $derived(
    tabs.find((t) => page.url.pathname.startsWith(tabHref(t.href)))?.href ??
      tabs[0].href,
  );
</script>

<div class="space-y-4">
  {#if !page.data.user}
    <div class="border-b bg-muted px-4 py-2 text-sm text-muted-foreground">
      You're viewing a shared project · <a
        href="/register"
        class="underline hover:text-foreground">Sign up</a
      > to build your own rankings
    </div>
  {/if}

  <div class="space-y-4 {page.data.wide ? 'mx-auto max-w-5xl px-4' : ''}">
    <div>
      <a
        href={page.data.user ? "/projects" : "/"}
        class="text-sm text-muted-foreground hover:text-foreground"
        >{page.data.user ? "← Projects" : "← Home"}</a
      >
      <h1 class="mt-1 text-2xl font-bold">{data.project.name}</h1>
      {#if data.project.game_name}
        <p class="text-sm text-muted-foreground">{data.project.game_name}</p>
      {/if}
    </div>

    <Tabs.Root value={currentTab} onValueChange={(v) => v && goto(tabHref(v))}>
      <Tabs.List>
        {#each tabs as tab (tab.href)}
          <Tabs.Trigger value={tab.href}>{tab.label}</Tabs.Trigger>
        {/each}
      </Tabs.List>
    </Tabs.Root>
  </div>

  <Separator />

  {@render children()}
</div>
```

- [ ] **Step 2: Run the banner/back-link tests — verify they pass**

```bash
cd web && npx playwright test projects.test.ts --grep "guest sees banner|guest sees.*Home|authenticated user does not see|authenticated user sees.*Projects" 2>&1 | tail -20
```

Expected: 4 passing.

- [ ] **Step 3: Run the full frontend test suite — verify no regressions**

```bash
cd web && npm run test:e2e 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/projects/[id]/+layout.svelte
git commit -m "feat(web): add guest banner and fix back link for unauthenticated visitors"
```

---

### Task 4: Write failing tests — copy-link in settings

**Files:**
- Modify: `web/tests/projects.test.ts`

The settings page only renders for project owners (`user_role === 'owner'`). The `test` fixture (pre-authenticated) plus `proj-published` (`user_role: 'owner'`, `published: true`) covers the published case. `proj-1` (`published: false`) covers the hidden case.

- [ ] **Step 1: Add copy-link tests**

Add at the end of `web/tests/projects.test.ts`:

```typescript
test('settings shows copy-link input and button when project is published', async ({ page }) => {
	await page.goto('/projects/proj-published/settings');
	const urlInput = page.locator('input[readonly]');
	await expect(urlInput).toBeVisible();
	await expect(urlInput).toHaveValue(/proj-published/);
	await expect(page.getByRole('button', { name: 'Copy link' })).toBeVisible();
});

test('settings does not show copy-link when project is not published', async ({ page }) => {
	await page.goto('/projects/proj-1/settings');
	await expect(page.locator('input[readonly]')).not.toBeVisible();
	await expect(page.getByRole('button', { name: 'Copy link' })).not.toBeVisible();
});

test('copy link button changes to Copied! after click', async ({ page, context }) => {
	await context.grantPermissions(['clipboard-write']);
	await page.goto('/projects/proj-published/settings');
	await page.getByRole('button', { name: 'Copy link' }).click();
	await expect(page.getByRole('button', { name: 'Copied!' })).toBeVisible();
	// Reverts after 2 seconds
	await expect(page.getByRole('button', { name: 'Copy link' })).toBeVisible({ timeout: 4000 });
});
```

- [ ] **Step 2: Run the copy-link tests — verify they fail**

```bash
cd web && npx playwright test projects.test.ts --grep "copy.link|Copy link|Copied" 2>&1 | tail -20
```

Expected: 3 failures (copy-link UI not yet implemented).

---

### Task 5: Implement copy-link in settings page

**Files:**
- Modify: `web/src/routes/projects/[id]/settings/+page.svelte`

- [ ] **Step 1: Add `page` import and `copied` state**

Add to the `<script lang="ts">` block, after the existing imports and before the `let { data, form }` line:

```typescript
import { page } from "$app/state";
```

Add after the existing `$state` declarations (after `let deleteFormEl` line):

```typescript
let copied = $state(false);

async function copyLink() {
  await navigator.clipboard.writeText(
    `${page.url.origin}/projects/${data.project.id}`,
  );
  copied = true;
  setTimeout(() => (copied = false), 2000);
}
```

- [ ] **Step 2: Add copy-link UI block inside the Publish section**

In the Publish section, add the copy-link div after the description `<p>` tag and before the `<form>`. The existing paragraph block looks like:

```svelte
<p class="text-sm text-muted-foreground">
  {#if data.project.published}
    This project is publicly visible. Anyone with the link can view stats,
    H2H, and rankings.
  {:else}
    This project is private. Only members can view it.
  {/if}
</p>
```

Add this block immediately after that `</p>`:

```svelte
{#if data.project.published}
  <div class="flex gap-2">
    <Input
      readonly
      value="{page.url.origin}/projects/{data.project.id}"
      class="flex-1 font-mono text-sm"
    />
    <Button type="button" variant="outline" onclick={copyLink}>
      {copied ? "Copied!" : "Copy link"}
    </Button>
  </div>
{/if}
```

- [ ] **Step 3: Run the copy-link tests — verify they pass**

```bash
cd web && npx playwright test projects.test.ts --grep "copy.link|Copy link|Copied" 2>&1 | tail -20
```

Expected: 3 passing.

- [ ] **Step 4: Run the full frontend test suite — verify no regressions**

```bash
cd web && npm run test:e2e 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 5: Format**

```bash
cd web && npm run format
```

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/[id]/settings/+page.svelte
git commit -m "feat(web): add copy-link to settings page for published projects"
```
