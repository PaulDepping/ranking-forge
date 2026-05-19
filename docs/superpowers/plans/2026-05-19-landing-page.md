# Landing Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `/` redirect-to-projects placeholder with a public marketing landing page and upgrade the app header to a NavigationMenu that renders for all users.

**Architecture:** Three file changes — `+layout.svelte` gets a NavigationMenu-based header that always renders and adapts to auth state; `+page.server.ts` is deleted (no more redirect); `+page.svelte` becomes a four-section marketing page. The `NavigationMenu` shadcn-svelte component needs to be installed first. Login and register pages lose their redundant `<h1>RankingForge</h1>` heading now that the brand lives permanently in the nav.

**Tech Stack:** SvelteKit 5 (runes), TypeScript, Tailwind CSS v4, shadcn-svelte (`NavigationMenu`, `Button`/`buttonVariants`, `Card`, `Badge`, `Separator`), Playwright for e2e tests.

---

### Task 1: Commit the design doc

**Files:**
- Already created: `docs/superpowers/specs/2026-05-19-landing-page-design.md`

- [ ] **Step 1: Stage and commit the spec**

```bash
git add docs/superpowers/specs/2026-05-19-landing-page-design.md
git commit -m "docs: add landing page design spec"
```

Expected: commit succeeds with 1 file changed.

---

### Task 2: Install NavigationMenu and update component docs

**Files:**
- Install to: `web/src/lib/components/ui/navigation-menu/`
- Modify: `web/CLAUDE.md`

- [ ] **Step 1: Install the component**

```bash
cd web && npx shadcn-svelte@latest add --yes --overwrite navigation-menu
```

Expected: several files created under `src/lib/components/ui/navigation-menu/`.

- [ ] **Step 2: Add NavigationMenu to the installed components table in `web/CLAUDE.md`**

Find the table and add a row:

```markdown
| Navigation Menu | `$lib/components/ui/navigation-menu` |
```

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/components/ui/navigation-menu web/CLAUDE.md
git commit -m "feat: install shadcn-svelte navigation-menu component"
```

---

### Task 3: Write failing e2e tests for the landing page

**Files:**
- Create: `web/tests/landing.test.ts`

- [ ] **Step 1: Create the test file**

```typescript
import { test, expect } from '@playwright/test';

const authedTest = test.extend({
	page: async ({ page }, use) => {
		await page.context().addCookies([
			{ name: 'session_id', value: 'test-session', domain: 'localhost', path: '/' }
		]);
		await use(page);
	}
});

// --- Unauthenticated ---

test('landing page renders without redirect', async ({ page }) => {
	await page.goto('/');
	await expect(page).toHaveURL('/');
});

test('landing page shows hero heading', async ({ page }) => {
	await page.goto('/');
	await expect(
		page.getByRole('heading', { name: 'The data behind your power rankings.' })
	).toBeVisible();
});

test('landing page shows Get started and Sign in CTAs when logged out', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Get started' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Sign in' }).first()).toBeVisible();
});

test('header shows Sign in and Register nav buttons when logged out', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Register' }).first()).toBeVisible();
});

test('header does not show Projects link when logged out', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Projects' })).not.toBeVisible();
});

test('landing page shows all four feature cards', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByText('Import from start.gg')).toBeVisible();
	await expect(page.getByText('Curate your events')).toBeVisible();
	await expect(page.getByText('Stats at a glance')).toBeVisible();
	await expect(page.getByText('Collaborate with your panel')).toBeVisible();
});

test('landing page shows How it works section', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { name: 'How it works' })).toBeVisible();
	await expect(page.getByText('Create a project')).toBeVisible();
	await expect(page.getByText('Import & curate')).toBeVisible();
	await expect(page.getByText('Build your ranking')).toBeVisible();
});

test('landing page shows footer with creator and GitHub link', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByText('Created by King')).toBeVisible();
	await expect(page.getByRole('link', { name: 'Source on GitHub' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Open source under AGPL v3' })).toBeVisible();
});

test('header always shows RankingForge brand link', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'RankingForge' })).toBeVisible();
});

// --- Authenticated ---

authedTest('landing page shows Go to your projects CTA when logged in', async ({ page }) => {
	await page.goto('/');
	await expect(page).toHaveURL('/');
	await expect(page.getByRole('link', { name: 'Go to your projects' })).toBeVisible();
});

authedTest('landing page does not show Get started CTA when logged in', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Get started' })).not.toBeVisible();
});

authedTest('header shows Projects link when logged in', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('link', { name: 'Projects' })).toBeVisible();
});

authedTest('header shows username when logged in', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByText('testuser')).toBeVisible();
});
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd web && npm run test:e2e -- tests/landing.test.ts
```

Expected: all tests fail — the page still redirects to `/projects` and the header is the old one.

---

### Task 4: Update `+layout.svelte` with NavigationMenu header

**Files:**
- Modify: `web/src/routes/+layout.svelte`

- [ ] **Step 1: Check the actual export path for `navigationMenuTriggerStyle` after installation**

```bash
grep -r "navigationMenuTriggerStyle" web/src/lib/components/ui/navigation-menu/
```

Note the file it's exported from — typically `navigation-menu-trigger.svelte` or the index. Use that path in the import below.

- [ ] **Step 2: Replace the contents of `+layout.svelte`**

```svelte
<script lang="ts">
	import '../app.css';
	import favicon from '$lib/assets/favicon.svg';
	import { PUBLIC_API_URL } from '$env/static/public';
	import { afterNavigate, goto } from '$app/navigation';
	import { ModeWatcher } from 'mode-watcher';
	import ThemeToggle from '$lib/components/ThemeToggle.svelte';
	import { Button, buttonVariants } from '$lib/components/ui/button';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import * as NavigationMenu from '$lib/components/ui/navigation-menu';
	import { navigationMenuTriggerStyle } from '$lib/components/ui/navigation-menu/navigation-menu-trigger.svelte';
	import { previousPage } from '$lib/stores/navigation';

	let { children, data } = $props();

	afterNavigate((navigation) => {
		previousPage.set(
			navigation.from ? navigation.from.url.pathname + navigation.from.url.search : null
		);
	});

	async function logout() {
		await fetch(`${PUBLIC_API_URL}/auth/logout`, { method: 'POST', credentials: 'include' });
		await goto('/login');
	}
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<ModeWatcher />

<Tooltip.Provider>
	<header class="border-b border-border bg-card">
		<div class="mx-auto flex max-w-5xl items-center px-4">
			<NavigationMenu.Root>
				<NavigationMenu.List>
					<NavigationMenu.Item>
						<NavigationMenu.Link href="/" class={navigationMenuTriggerStyle()}>
							<span class="font-semibold">RankingForge</span>
						</NavigationMenu.Link>
					</NavigationMenu.Item>
					{#if data.user}
						<NavigationMenu.Item>
							<NavigationMenu.Link href="/projects" class={navigationMenuTriggerStyle()}>
								Projects
							</NavigationMenu.Link>
						</NavigationMenu.Item>
					{/if}
				</NavigationMenu.List>
			</NavigationMenu.Root>
			<div class="ml-auto flex items-center gap-2">
				<ThemeToggle />
				{#if data.user}
					<span class="text-sm text-muted-foreground">{data.user.username}</span>
					<Button variant="ghost" size="sm" onclick={logout}>Logout</Button>
				{:else}
					<a href="/login" class={buttonVariants({ variant: 'outline', size: 'sm' })}>Sign in</a>
					<a href="/register" class={buttonVariants({ variant: 'default', size: 'sm' })}>Register</a>
				{/if}
			</div>
		</div>
	</header>

	<main class="mx-auto max-w-5xl px-4 py-8">
		{@render children()}
	</main>
</Tooltip.Provider>
```

> **Note:** If `navigationMenuTriggerStyle` is not exported from `navigation-menu-trigger.svelte`, check the grep output from Step 1 and adjust the import path accordingly.

- [ ] **Step 3: Start the dev server and visually verify the header**

```bash
cd web && npm run dev
```

Open http://localhost:5173 (logged out). Confirm: RankingForge brand link, Sign in and Register buttons on the right, ThemeToggle visible.

---

### Task 5: Remove the redirect and build the landing page

**Files:**
- Delete: `web/src/routes/+page.server.ts`
- Modify: `web/src/routes/+page.svelte`

- [ ] **Step 1: Delete `+page.server.ts`**

```bash
rm web/src/routes/+page.server.ts
```

`data.user` is already provided by `+layout.server.ts` — no page-level load needed.

- [ ] **Step 2: Replace `+page.svelte` with the full landing page**

```svelte
<script lang="ts">
	import { buttonVariants } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Badge } from '$lib/components/ui/badge';
	import { Separator } from '$lib/components/ui/separator';

	let { data } = $props();

	const features = [
		{
			title: 'Import from start.gg',
			description: 'Fetch your tournament history automatically — just provide the player slugs.'
		},
		{
			title: 'Curate your events',
			description:
				"Manually exclude tournaments that shouldn't count. You stay in control of what goes into the ranking."
		},
		{
			title: 'Stats at a glance',
			description:
				"Per-player win/loss breakdowns and head-to-head tables, ready to reference when you're building your list."
		},
		{
			title: 'Collaborate with your panel',
			description:
				'Invite other panelists to work on a ranking together — multiple people can contribute to the same project.'
		}
	];

	const steps = [
		{
			title: 'Create a project',
			description:
				'Add the players you want to rank using their start.gg slugs. Invite your fellow panelists to collaborate.'
		},
		{
			title: 'Import & curate',
			description:
				"We fetch the tournament history automatically. Deselect any events that shouldn't count toward the ranking."
		},
		{
			title: 'Build your ranking',
			description:
				"Use the win/loss breakdowns and head-to-head tables to inform your panel's decisions."
		}
	];
</script>

<!-- Hero -->
<section class="py-20 text-center">
	<h1 class="text-4xl font-bold tracking-tight sm:text-5xl">
		The data behind your power rankings.
	</h1>
	<p class="mx-auto mt-4 max-w-2xl text-lg text-muted-foreground">
		Pull your tournament data from start.gg, curate which events count, and get the stats to back
		up your ranking decisions.
	</p>
	<div class="mt-8 flex justify-center gap-3">
		{#if data.user}
			<a href="/projects" class={buttonVariants()}>Go to your projects</a>
		{:else}
			<a href="/register" class={buttonVariants()}>Get started</a>
			<a href="/login" class={buttonVariants({ variant: 'outline' })}>Sign in</a>
		{/if}
	</div>
</section>

<!-- Features -->
<section class="py-12">
	<div class="grid grid-cols-1 gap-4 md:grid-cols-2">
		{#each features as feature}
			<Card.Root>
				<Card.Header>
					<Card.Title>{feature.title}</Card.Title>
				</Card.Header>
				<Card.Content>
					<p class="text-muted-foreground">{feature.description}</p>
				</Card.Content>
			</Card.Root>
		{/each}
	</div>
</section>

<!-- How it works -->
<section class="py-12">
	<h2 class="mb-8 text-center text-2xl font-bold">How it works</h2>
	<div class="grid grid-cols-1 gap-8 md:grid-cols-3">
		{#each steps as step, i}
			<div class="text-center">
				<Badge class="mb-3">{i + 1}</Badge>
				<h3 class="mb-2 font-semibold">{step.title}</h3>
				<p class="text-sm text-muted-foreground">{step.description}</p>
			</div>
		{/each}
	</div>
</section>

<!-- Footer -->
<Separator class="mt-12" />
<footer class="py-6 text-center text-sm text-muted-foreground">
	Created by King ·
	<a
		href="https://github.com/PaulDepping/ranking-forge"
		class="underline hover:text-foreground"
		target="_blank"
		rel="noopener noreferrer">Source on GitHub</a
	> ·
	<a
		href="https://www.gnu.org/licenses/agpl-3.0.html"
		class="underline hover:text-foreground"
		target="_blank"
		rel="noopener noreferrer">Open source under AGPL v3</a
	>
</footer>
```

- [ ] **Step 3: Visually verify the landing page in the dev server**

Open http://localhost:5173. Confirm all four sections render correctly in both light and dark mode. Resize to mobile width — feature grid should stack to a single column.

- [ ] **Step 4: Run landing page tests**

```bash
cd web && npm run test:e2e -- tests/landing.test.ts
```

Expected: all landing page tests pass.

---

### Task 6: Remove duplicate headings from login and register pages, update auth test

Both `login/+page.svelte` and `register/+page.svelte` have `<h1>RankingForge</h1>` that duplicates the brand now always in the nav. Remove them. Update the auth test that checked for that heading.

**Files:**
- Modify: `web/src/routes/login/+page.svelte`
- Modify: `web/src/routes/register/+page.svelte`
- Modify: `web/tests/auth.test.ts`

- [ ] **Step 1: Update `login/+page.svelte` — replace the heading block**

Old:
```svelte
<div class="space-y-1 text-center">
    <h1 class="text-2xl font-bold">RankingForge</h1>
    <p class="text-muted-foreground">Sign in to your account</p>
</div>
```

New:
```svelte
<div class="space-y-1 text-center">
    <p class="text-lg font-semibold">Sign in to your account</p>
</div>
```

- [ ] **Step 2: Update `register/+page.svelte` — replace the heading block**

Old:
```svelte
<div class="space-y-1 text-center">
    <h1 class="text-2xl font-bold">RankingForge</h1>
    <p class="text-muted-foreground">Create an account</p>
</div>
```

New:
```svelte
<div class="space-y-1 text-center">
    <p class="text-lg font-semibold">Create an account</p>
</div>
```

- [ ] **Step 3: Update `auth.test.ts` — the login test no longer finds a heading named RankingForge**

Old assertion in `'login page renders the sign-in form'`:
```typescript
await expect(page.getByRole('heading', { name: 'RankingForge' })).toBeVisible();
```

Replace with a check for the nav brand link (which is always present):
```typescript
await expect(page.getByRole('link', { name: 'RankingForge' })).toBeVisible();
```

- [ ] **Step 4: Run auth tests to confirm they still pass**

```bash
cd web && npm run test:e2e -- tests/auth.test.ts
```

Expected: all auth tests pass.

---

### Task 7: Run the full test suite and commit

**Files:** none — verification only, then commit.

- [ ] **Step 1: Run the complete e2e suite**

```bash
cd web && npm run test:e2e
```

Expected: all tests in `auth.test.ts`, `projects.test.ts`, `player.test.ts`, and `landing.test.ts` pass.

- [ ] **Step 2: Run unit tests**

```bash
cd web && npm run test:unit
```

Expected: all unit tests pass.

- [ ] **Step 3: Commit all changes**

```bash
git add \
  web/src/routes/+layout.svelte \
  web/src/routes/+page.svelte \
  web/src/routes/login/+page.svelte \
  web/src/routes/register/+page.svelte \
  web/tests/landing.test.ts \
  web/tests/auth.test.ts
git rm web/src/routes/+page.server.ts
git commit -m "feat: add public landing page and upgrade header to NavigationMenu"
```
