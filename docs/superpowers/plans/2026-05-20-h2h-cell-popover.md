# H2H Cell Popover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the side-panel Card on the H2H page with a shadcn Popover anchored to the clicked cell, so set details float over the table near the click point regardless of table size.

**Architecture:** Each non-diagonal cell that has a record wraps its trigger Button in a `Popover.Root` (controlled via `isSelected()`). `selectCell` sets a provisional `selectedPair` immediately on click (so the popover opens and shows a skeleton during the fetch), then replaces it with real data when the fetch completes. `Popover.Content` renders via a Portal (never clipped by the table). One `selectedPair` state controls which popover is open.

**Tech Stack:** SvelteKit, Svelte 5 (runes), shadcn-svelte `Popover` (bits-ui + floating-ui), Tailwind CSS v4, Vitest + @testing-library/svelte

---

## Files

- **Modify:** `web/src/routes/projects/[id]/h2h/+page.svelte` — primary change: set provisional selectedPair in selectCell, remove side panel, add per-cell Popover
- **Modify:** `web/src/routes/projects/[id]/h2h/h2h.test.ts` — update one test description

---

## Task 1: Fix `selectCell` to open the popover immediately on click

**Files:**
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`

The current `selectCell` sets `selectedPair = null` before fetching, which means `isSelected()` returns false during the load and the popover would never open to show a skeleton. Fix: set a provisional `selectedPair` immediately so `isSelected()` becomes true right away.

- [ ] **Step 1: Update `selectCell` to use a provisional selectedPair**

Replace the existing `selectCell` function:

```ts
async function selectCell(
  rowPlayer: { id: string; name: string },
  colPlayer: { id: string; name: string }
) {
  if (
    selectedPair?.rowPlayer.id === rowPlayer.id &&
    selectedPair?.colPlayer.id === colPlayer.id
  ) {
    selectedPair = null;
    return;
  }
  loading = true;
  selectedPair = { rowPlayer, colPlayer, sets: [], wins: 0, losses: 0 };
  try {
    const api = makeApi(fetch, env.PUBLIC_API_URL);
    const res = await api.get(
      `/projects/${data.project.id}/head-to-head/${rowPlayer.id}/${colPlayer.id}/sets`
    );
    const sets: H2HSet[] = res.ok ? await res.json() : [];
    selectedPair = {
      rowPlayer,
      colPlayer,
      sets,
      wins: sets.filter((s) => s.is_win).length,
      losses: sets.filter((s) => !s.is_win).length,
    };
  } finally {
    loading = false;
  }
}
```

- [ ] **Step 2: Commit**

```bash
git add web/src/routes/projects/[id]/h2h/+page.svelte
git commit -m "fix: set provisional selectedPair immediately on h2h cell click"
```

---

## Task 2: Replace side panel with per-cell Popover

**Files:**
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`

- [ ] **Step 1: Add the Popover import**

In the `<script>` block, add alongside the existing imports:

```svelte
import * as Popover from '$lib/components/ui/popover';
```

- [ ] **Step 2: Collapse the outer flex wrapper**

The current markup wraps both the table and the side panel in a flex div:

```svelte
<div class="flex gap-4 items-start flex-wrap">
  <!-- Matrix -->
  <div>
    <Table.Root ...>...</Table.Root>
    <p class="mt-1 ...">Row player's record vs. column player</p>
  </div>

  <!-- Side panel -->
  {#if loading}
    <div class="min-w-[220px] ...">...</div>
  {:else if selectedPair}
    <Card.Root ...>...</Card.Root>
  {/if}
</div>
```

Remove the outer flex div and the entire side panel block (both the `{#if loading}` and `{:else if selectedPair}` branches). The result should be just:

```svelte
<div>
  <Table.Root class="border-collapse">
    ...
  </Table.Root>
  <p class="mt-1 text-xs text-muted-foreground">Row player's record vs. column player</p>
</div>
```

- [ ] **Step 3: Wrap each record cell Button in a Popover**

Inside the `{#each data.players as col}` loop, find the `{#if rec}` block that currently renders the Button. Replace it with:

```svelte
{#if rec}
  <Popover.Root
    open={isSelected(row.id, col.id)}
    onOpenChange={(v) => { if (!v) selectedPair = null; }}
  >
    <Popover.Trigger>
      {#snippet child({ props })}
        <Button
          {...props}
          variant="ghost"
          class="h-auto rounded px-1 py-0
            {isSelected(row.id, col.id)
              ? 'ring-2 ring-primary bg-primary/10 hover:bg-primary/10'
              : rec.wins > rec.losses
                ? 'bg-green-50 dark:bg-green-950/30 text-green-700 dark:text-green-400'
                : rec.wins < rec.losses
                  ? 'bg-red-50 dark:bg-red-950/30 text-red-700 dark:text-red-400'
                  : ''}"
          onclick={() => selectCell(row, col)}
        >
          {rec.wins}–{rec.losses}
        </Button>
      {/snippet}
    </Popover.Trigger>
    <Popover.Content side="right" align="center" class="w-64 p-0">
      {#if loading}
        <div class="p-3 space-y-3">
          <Skeleton class="h-8 w-full rounded-md" />
          <Skeleton class="h-4 w-3/4" />
          <div class="space-y-1.5">
            <Skeleton class="h-7 w-full" />
            <Skeleton class="h-7 w-full" />
            <Skeleton class="h-7 w-full" />
          </div>
        </div>
      {:else if selectedPair}
        <div class="p-3">
          <div class="mb-3 flex items-start justify-between gap-2 border-b border-border pb-2">
            <div>
              <p class="font-semibold text-sm">{selectedPair.rowPlayer.name} vs {selectedPair.colPlayer.name}</p>
              <p class="text-xs text-muted-foreground">{selectedPair.wins} wins · {selectedPair.losses} losses</p>
            </div>
            <Button
              variant="ghost"
              size="icon"
              onclick={() => (selectedPair = null)}
              aria-label="Close panel"
            >×</Button>
          </div>
          {#if selectedPair.sets.length === 0}
            <p class="text-xs text-muted-foreground">No sets found.</p>
          {:else}
            <div class="space-y-px">
              {#each selectedPair.sets as set, i (i)}
                <Button
                  variant="ghost"
                  class="h-auto w-full flex items-center gap-2 rounded px-2 py-1.5 text-xs border-b border-border last:border-0 justify-start"
                  onclick={() => { selectedSet = set; selectedIsWin = set.is_win; }}
                >
                  <span class={set.is_win
                    ? 'font-bold text-green-600 dark:text-green-400 min-w-[12px]'
                    : 'font-bold text-red-600 dark:text-red-400 min-w-[12px]'}>
                    {set.is_win ? 'W' : 'L'}
                  </span>
                  {#if set.winner_score !== null && set.loser_score !== null}
                    <span class="tabular-nums">
                      {set.is_win
                        ? `${set.winner_score}–${set.loser_score}`
                        : `${set.loser_score}–${set.winner_score}`}
                    </span>
                  {/if}
                  <span class="text-muted-foreground truncate flex-1 text-left">{set.tournament_name}</span>
                  {#if set.round_name}
                    <span class="text-muted-foreground shrink-0">{set.round_name}</span>
                  {/if}
                </Button>
              {/each}
            </div>
          {/if}
          <p class="mt-2 text-xs text-muted-foreground">Click a row for full details</p>
        </div>
      {/if}
    </Popover.Content>
  </Popover.Root>
{:else}
  <span class="text-muted-foreground">—</span>
{/if}
```

**Implementation notes:**
- `onclick={() => selectCell(row, col)}` on the Button overrides bits-ui's default toggle onclick from `{...props}` (Svelte 5 last-attribute-wins). This is intentional: we manage open state ourselves via `selectedPair`, so bits-ui's toggle is a no-op — close-on-outside-click still fires via `onOpenChange`.
- `Popover.Content` uses a Portal (renders to document body) so it is never clipped by the table's overflow or scroll container.
- `Card` import can be removed from the script block once the side panel is gone.

- [ ] **Step 4: Remove unused Card import**

In the `<script>` block, remove:

```svelte
import * as Card from '$lib/components/ui/card';
```

- [ ] **Step 5: Run the unit tests**

```bash
cd web && npm run test:unit -- --reporter=verbose 2>&1 | head -60
```

Expected: all 7 H2H tests pass. If `'does not show popover content before any cell is clicked'` fails, it means bits-ui is rendering the Popover.Content in the DOM even when closed — move to Task 3 before committing.

- [ ] **Step 6: Commit**

```bash
git add web/src/routes/projects/[id]/h2h/+page.svelte
git commit -m "feat: replace h2h side panel with anchored cell popover"
```

---

## Task 3: Update test description

**Files:**
- Modify: `web/src/routes/projects/[id]/h2h/h2h.test.ts`

- [ ] **Step 1: Update the test description**

Change:

```ts
it('does not show side panel before any cell is clicked', () => {
```

to:

```ts
it('does not show popover content before any cell is clicked', () => {
```

- [ ] **Step 2: Run tests to confirm all pass**

```bash
cd web && npm run test:unit -- --reporter=verbose 2>&1 | head -60
```

Expected:

```
✓ H2H page > renders player names in header row
✓ H2H page > renders win–loss records between players
✓ H2H page > shows empty message when h2h data is absent
✓ H2H page > shows table footer note when data is present
✓ H2H page > renders a dash for same-player diagonal cells
✓ H2H page > renders non-diagonal cells as clickable buttons
✓ H2H page > does not show popover content before any cell is clicked
```

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/projects/[id]/h2h/h2h.test.ts
git commit -m "test: update h2h test description for popover terminology"
```
