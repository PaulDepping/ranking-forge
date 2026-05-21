# Wide Layout Design

**Date:** 2026-05-21  
**Status:** Approved

## Problem

The global layout constrains all pages to `max-w-5xl` (~1024px). Data-dense views — particularly the H2H matrix and the Stats card grid — leave large amounts of screen space unused on desktop and, in the H2H case, cause the table to overflow or require horizontal scrolling even for modest rosters (~12+ players).

Text-heavy pages (settings, players, import, ranking) are fine at the current width; the constraint only hurts pages that display tabular or grid data.

## Approach

Per-page opt-in via a `wide` flag in page load data (SvelteKit Approach A).

- Each wide page's `+page.server.ts` returns `wide: true`.
- The root layout reads `$page.data.wide` and removes the `max-w-5xl` constraint from `<main>` on those pages.
- The project layout (`[id]/+layout.svelte`) keeps its chrome (project name, tabs) anchored at `max-w-5xl` on wide pages, so navigation feels consistent. Only the content below the separator expands.
- All other pages default to `wide: false` (established in the root layout server load) and are unaffected.

## Pages that opt in

| Page | Reason |
|---|---|
| H2H (`/projects/[id]/h2h`) | N×N matrix — needs full width to show all players without horizontal scroll |
| Stats (`/projects/[id]/stats`) | Auto-fill card grid — more viewport width yields more cards per row |

All other project pages (players, import, tournaments, ranking, settings) remain narrow.

## Files changed

### 1. `src/routes/+layout.server.ts`
Add `wide: false as boolean` to the return value. This establishes `wide` as a typed key in `$page.data` with a safe default, so the root layout can read it without a type assertion.

```ts
export const load: LayoutServerLoad = ({ locals }) => {
  return { user: locals.user, wide: false as boolean };
};
```

### 2. `src/routes/+layout.svelte`
Import `page` from `$app/state`. Switch `<main>` class based on `page.data.wide`:

```svelte
<main class={page.data.wide ? 'px-4 py-8' : 'mx-auto max-w-5xl px-4 py-8'}>
```

The `<header>` is unchanged — it stays at `max-w-5xl` regardless.

### 3. `src/routes/projects/[id]/+layout.svelte`
Import `page` from `$app/state`. On wide pages, wrap the project-name + tabs block in `mx-auto max-w-5xl` so it stays anchored. Move `<Separator>` outside that wrapper so it spans the full viewport width, acting as a clean visual break between the anchored chrome and the expanded content area.

```svelte
<div class="space-y-4">
  <div class={page.data.wide ? 'mx-auto max-w-5xl' : ''}>
    <!-- back link, project name, game name -->
    <Tabs.Root ...>...</Tabs.Root>
  </div>
  <Separator />
  {@render children()}
</div>
```

### 4. `src/routes/projects/[id]/h2h/+page.server.ts`
Add `wide: true` to the load return:

```ts
return { h2h, players, wide: true };
```

### 5. `src/routes/projects/[id]/stats/+page.server.ts`
Add `wide: true` to the load return:

```ts
return { stats, wide: true };
```

### 6. `src/routes/projects/[id]/h2h/+page.svelte`
Wrap the `<Table.Root>` in an `overflow-x-auto` div as a fallback for rosters too large for any screen width:

```svelte
<div class="overflow-x-auto">
  <Table.Root class="border-collapse">
    ...
  </Table.Root>
</div>
```

## What does not change

- The global nav header — always `max-w-5xl`.
- All non-H2H/Stats project pages — untouched, naturally narrow.
- The ranking list's own `max-w-xl` self-constraint — unchanged.
- No new components, abstractions, or route group restructuring needed.

## Testing

Manually verify:
- H2H page: table uses full viewport width; project name and tabs remain centered; separator spans full width.
- Stats page: card grid fills more columns on wide screens.
- Any narrow page (e.g. Settings, Ranking): layout unchanged, no regression.
- H2H on a narrow viewport or with a very large roster: `overflow-x-auto` kicks in, table scrolls horizontally.
