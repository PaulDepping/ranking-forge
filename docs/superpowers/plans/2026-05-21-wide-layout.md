# Wide Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let H2H and Stats pages opt into a full-viewport-width layout while keeping all other pages, and the project chrome (name + tabs), anchored at the existing `max-w-5xl` width.

**Architecture:** A `wide: boolean` key is added to the root layout server load as the default (`false`). Wide pages return `wide: true` from their own `+page.server.ts`, which SvelteKit merges into `$page.data`, overriding the default. The root `+layout.svelte` reads `page.data.wide` and switches `<main>` between constrained and unconstrained width. The project layout reads the same flag to wrap its chrome (project name + tabs) in `max-w-5xl` so it stays anchored even when the content area is full-width.

**Tech Stack:** SvelteKit 5, Svelte 5 runes, Tailwind CSS v4, Playwright (e2e tests in `web/tests/`)

---

## File Map

| File | Change |
|---|---|
| `web/src/routes/+layout.server.ts` | Add `wide: false as boolean` to return |
| `web/src/routes/+layout.svelte` | Add `page` import; switch `<main>` class on `page.data.wide` |
| `web/src/routes/projects/[id]/+layout.svelte` | Wrap chrome in `max-w-5xl` div when wide; move Separator outside |
| `web/src/routes/projects/[id]/h2h/+page.server.ts` | Add `wide: true` to return |
| `web/src/routes/projects/[id]/h2h/+page.svelte` | Wrap table in `overflow-x-auto` div |
| `web/src/routes/projects/[id]/stats/+page.server.ts` | Add `wide: true` to return |
| `web/tests/projects.test.ts` | Add 3 layout-width assertions |

---

## Task 1: Wide flag infrastructure + H2H opt-in

**Files:**
- Modify: `web/tests/projects.test.ts`
- Modify: `web/src/routes/+layout.server.ts`
- Modify: `web/src/routes/+layout.svelte`
- Modify: `web/src/routes/projects/[id]/h2h/+page.server.ts`

- [ ] **Step 1: Write failing layout tests**

Add these three tests to the bottom of `web/tests/projects.test.ts` (before the final closing, after the existing tests):

```ts
test('h2h page uses full-width layout', async ({ page }) => {
  await page.goto('/projects/proj-1/h2h');
  await expect(page.locator('main')).not.toHaveClass(/max-w-5xl/);
});

test('stats page uses full-width layout', async ({ page }) => {
  await page.goto('/projects/proj-1/stats');
  await expect(page.locator('main')).not.toHaveClass(/max-w-5xl/);
});

test('non-wide pages keep centered max-w-5xl layout', async ({ page }) => {
  await page.goto('/projects/proj-1/ranking');
  await expect(page.locator('main')).toHaveClass(/max-w-5xl/);
});
```

- [ ] **Step 2: Run tests — confirm all three fail**

```bash
cd web && npx playwright test --grep "full-width layout|max-w-5xl layout"
```

Expected:
- `h2h page uses full-width layout` → FAIL (`<main>` currently has `max-w-5xl`)
- `stats page uses full-width layout` → FAIL (`<main>` currently has `max-w-5xl`)
- `non-wide pages keep centered max-w-5xl layout` → PASS (ranking page already has `max-w-5xl`)

The two failing tests produce:
```
Error: Locator expected not to match value "mx-auto max-w-5xl px-4 py-8"
```

- [ ] **Step 3: Add `wide` default to root layout server**

Replace the entire content of `web/src/routes/+layout.server.ts`:

```ts
import type { LayoutServerLoad } from "./$types";

export const load: LayoutServerLoad = ({ locals }) => {
  return { user: locals.user, wide: false as boolean };
};
```

The `as boolean` cast is required: without it TypeScript infers the literal type `false`, which would reject child pages returning `true`.

- [ ] **Step 4: Switch `<main>` class in root layout**

In `web/src/routes/+layout.svelte`, add the `page` import after the existing imports in the `<script>` block:

```ts
import { page } from "$app/state";
```

Then change the `<main>` tag (currently line 84):

```svelte
<!-- before -->
<main class="mx-auto max-w-5xl px-4 py-8">

<!-- after -->
<main class={page.data.wide ? 'px-4 py-8' : 'mx-auto max-w-5xl px-4 py-8'}>
```

- [ ] **Step 5: H2H page opts into wide layout**

In `web/src/routes/projects/[id]/h2h/+page.server.ts`, add `wide: true` to the return statement:

```ts
// before
return { h2h, players };

// after
return { h2h, players, wide: true };
```

- [ ] **Step 6: Run tests — confirm h2h test now passes, stats still fails**

```bash
cd web && npx playwright test --grep "full-width layout|max-w-5xl layout"
```

Expected:
- `h2h page uses full-width layout` → PASS
- `stats page uses full-width layout` → FAIL (stats not opted in yet)
- `non-wide pages keep centered max-w-5xl layout` → PASS

- [ ] **Step 7: Commit**

```bash
git add web/src/routes/+layout.server.ts web/src/routes/+layout.svelte \
        web/src/routes/projects/\[id\]/h2h/+page.server.ts \
        web/tests/projects.test.ts
git commit -m "feat(web): add wide layout flag; H2H page opts in"
```

---

## Task 2: Anchor project chrome on wide pages

**Files:**
- Modify: `web/src/routes/projects/[id]/+layout.svelte`

The project layout already imports `page` from `$app/state` (used to derive `currentTab`). The change wraps the project-name + tabs block in a `max-w-5xl mx-auto` div on wide pages so they stay anchored to the center, while the `<Separator>` and page content expand to full width.

- [ ] **Step 1: Restructure the project layout template**

In `web/src/routes/projects/[id]/+layout.svelte`, replace the entire template section (the `<div class="space-y-4">` block) with:

```svelte
<div class="space-y-4">
  <div class={page.data.wide ? 'mx-auto max-w-5xl' : ''}>
    <div>
      <a
        href="/projects"
        class="text-sm text-muted-foreground hover:text-foreground">← Projects</a
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

Key structural change: the inner `<div>` (project name) and `<Tabs.Root>` are now children of a new wrapper div that applies `mx-auto max-w-5xl` only on wide pages. `<Separator>` is moved outside that wrapper so it spans the full content width on wide pages.

- [ ] **Step 2: Run full e2e suite to confirm no regression**

```bash
cd web && npm run test:e2e
```

Expected: all previously passing tests still pass. The H2H layout test from Task 1 still passes.

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/projects/\[id\]/+layout.svelte
git commit -m "feat(web): anchor project chrome to max-w-5xl on wide pages"
```

---

## Task 3: Add overflow-x-auto to H2H table

**Files:**
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`

This is a safety net for rosters too large to fit even on the widest screens. No new test needed — the existing `h2h page renders the player grid` test already verifies the table renders correctly.

- [ ] **Step 1: Wrap the table in an overflow container**

In `web/src/routes/projects/[id]/h2h/+page.svelte`, find this block (inside the `{:else}` branch, around line 88):

```svelte
    <div>
      <Table.Root class="border-collapse">
```

Change it to:

```svelte
    <div class="overflow-x-auto">
      <Table.Root class="border-collapse">
```

Only the opening `<div>` changes — nothing else in this block.

- [ ] **Step 2: Run the H2H e2e test to confirm the table still renders**

```bash
cd web && npx playwright test --grep "h2h page renders the player grid"
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/projects/\[id\]/h2h/+page.svelte
git commit -m "feat(web): add overflow-x-auto to H2H table for large rosters"
```

---

## Task 4: Stats page opts into wide layout

**Files:**
- Modify: `web/src/routes/projects/[id]/stats/+page.server.ts`

- [ ] **Step 1: Confirm stats layout test still fails**

```bash
cd web && npx playwright test --grep "stats page uses full-width layout"
```

Expected: FAIL — `<main>` still has `max-w-5xl` on the stats page.

- [ ] **Step 2: Stats page opts into wide layout**

In `web/src/routes/projects/[id]/stats/+page.server.ts`, add `wide: true` to the return statement:

```ts
// before
return { stats };

// after
return { stats, wide: true };
```

- [ ] **Step 3: Run all layout tests — confirm all pass**

```bash
cd web && npx playwright test --grep "full-width layout|max-w-5xl layout"
```

Expected: all 3 tests PASS.

- [ ] **Step 4: Run full e2e suite**

```bash
cd web && npm run test:e2e
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/\[id\]/stats/+page.server.ts
git commit -m "feat(web): Stats page opts into wide layout"
```
