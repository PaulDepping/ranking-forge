# Wide Page Heading Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align the `<h2>` headings on the H2H and Stats pages with the project title/tabs nav above them, while leaving the body content full-width.

**Architecture:** Each wide page currently wraps heading + body in one `<div class="space-y-4 px-4">`. We split that into two sibling sections: a constrained heading zone (`mx-auto max-w-5xl px-4`) and an unconstrained body zone. The outer wrapper keeps `space-y-4` for spacing. `max-w-5xl` matches the value already used in the project layout's title/tabs area (`+layout.svelte:40`).

**Tech Stack:** SvelteKit, Tailwind CSS v4

---

### Task 1: Fix H2H page heading alignment

**Files:**
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte:75-78`

These are CSS layout changes with no testable logic unit. Verification is visual (run the dev server) and regression-checked via e2e tests.

- [ ] **Step 1: Edit `h2h/+page.svelte`**

Replace the outer wrapper div (line 75) and heading (line 76):

Before:
```svelte
<div class="space-y-4 px-4">
  <h2 class="text-lg font-semibold">Head-to-head</h2>
```

After:
```svelte
<div class="space-y-4">
  <h2 class="mx-auto max-w-5xl px-4 text-lg font-semibold">Head-to-head</h2>
```

The closing `</div>` on line 278 stays as-is. The `{#if}` / `{:else}` block (lines 78–277) is now the body zone — leave it untouched.

- [ ] **Step 2: Run e2e tests to confirm no regressions**

```bash
cd web && npm run test:e2e
```

Expected: all tests pass (the layout change is visual-only; no e2e test asserts heading position).

- [ ] **Step 3: Commit**

```bash
cd web && npm run format
git add web/src/routes/projects/[id]/h2h/+page.svelte
git commit -m "fix(web): align H2H page heading with nav on wide layout"
```

---

### Task 2: Fix Stats page heading alignment

**Files:**
- Modify: `web/src/routes/projects/[id]/stats/+page.svelte:27-29`

- [ ] **Step 1: Edit `stats/+page.svelte`**

Replace the outer wrapper div (line 27) and heading (line 28):

Before:
```svelte
<div class="space-y-4 px-4">
  <h2 class="text-lg font-semibold">Stats</h2>
```

After:
```svelte
<div class="space-y-4">
  <h2 class="mx-auto max-w-5xl px-4 text-lg font-semibold">Stats</h2>
```

The closing `</div>` on line 107 stays as-is. The `{#if}` / `{:else}` block (lines 30–106) is the body zone — leave it untouched.

- [ ] **Step 2: Run e2e tests to confirm no regressions**

```bash
cd web && npm run test:e2e
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
cd web && npm run format
git add web/src/routes/projects/[id]/stats/+page.svelte
git commit -m "fix(web): align Stats page heading with nav on wide layout"
```
