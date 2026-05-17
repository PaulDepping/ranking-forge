# Bracket Filter Rare Types Dialog — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 5 rare bracket type rows in the filter popover with an "All bracket types…" button that opens a viewport-centred Dialog showing all 10 types.

**Architecture:** All changes are confined to `+page.svelte`. A new `bracketDialogOpen` boolean drives the Dialog. A new `rareActiveCount` derived counts active rare-type states and drives a count badge on the link button. The Dialog shares `bracketFilter` directly — no extra state.

**Tech Stack:** Svelte 5 (runes), shadcn-svelte Dialog, Vitest (unit tests)

---

## File map

| File | Change |
|------|--------|
| `web/src/routes/projects/[id]/tournaments/filter.test.ts` | Add `rareActiveCount` helper + 3 unit tests |
| `web/src/routes/projects/[id]/tournaments/+page.svelte` | Add Dialog import, `bracketDialogOpen` state, `rareActiveCount` derived; rework popover; add Dialog markup |

---

## Task 1: Add `rareActiveCount` — TDD

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/filter.test.ts`
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte` (derived only)

- [ ] **Step 1: Add standalone helper and failing tests to filter.test.ts**

  Add after the existing `eventVisible` function (around line 76), before the first `describe` block:

  ```ts
  const RARE_TYPES = [
      'EXHIBITION', 'RACE', 'CIRCUIT', 'CUSTOM_SCHEDULE', 'ELIMINATION_ROUNDS',
  ] as const;

  function rareActiveCount(bracketFilter: Record<string, BracketTypeState>): number {
      return RARE_TYPES.filter(t => bracketFilter[t] !== 'neutral').length;
  }
  ```

  Then add a new `describe` block at the end of the file:

  ```ts
  describe('rareActiveCount', () => {
      it('returns 0 when all types are neutral', () => {
          const filter: Record<string, BracketTypeState> = {};
          expect(rareActiveCount(filter)).toBe(0);
      });

      it('counts required rare types', () => {
          const filter: Record<string, BracketTypeState> = {
              EXHIBITION: 'required',
              RACE: 'excluded',
          };
          expect(rareActiveCount(filter)).toBe(2);
      });

      it('ignores common types', () => {
          const filter: Record<string, BracketTypeState> = {
              DOUBLE_ELIMINATION: 'required',
          };
          expect(rareActiveCount(filter)).toBe(0);
      });
  });
  ```

- [ ] **Step 2: Run to confirm the tests pass (the helper is defined in the same file)**

  ```bash
  cd web && npm run test:unit -- filter
  ```

  Expected: all tests pass (the helper is self-contained in the test file, so these pass immediately).

- [ ] **Step 3: Add the derived to `+page.svelte`**

  In the script block, after the existing `bracketTriggerLabel` derived (around line 85), add:

  ```ts
  const rareActiveCount = $derived(
      RARE_BRACKET_TYPES.filter(t => bracketFilter[t] !== 'neutral').length
  );
  ```

- [ ] **Step 4: Add `bracketDialogOpen` state**

  In the script block, after `let bracketPopoverOpen = $state(false);` (line 74), add:

  ```ts
  let bracketDialogOpen = $state(false);
  ```

- [ ] **Step 5: Commit**

  ```bash
  cd web && npm run test:unit -- filter
  ```

  Expected: all tests pass.

  ```bash
  git add web/src/routes/projects/[id]/tournaments/filter.test.ts \
          web/src/routes/projects/[id]/tournaments/+page.svelte
  git commit -m "feat(web): add rareActiveCount derived for bracket filter badge"
  ```

---

## Task 2: Rework popover — remove rare rows, add link button

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Remove the rare bracket type rows and their divider from the popover**

  In the template, find the comment `<!-- Rarer bracket types -->` and the divider above it (around lines 388–395). Remove this entire block:

  ```svelte
  <div class="border-t border-border my-1.5"></div>

  <!-- Rarer bracket types -->
  {#each RARE_BRACKET_TYPES as bt}
      {@render bracketRow(bt)}
  {/each}
  ```

- [ ] **Step 2: Add the "All bracket types…" button in its place**

  In the gap you just created (still inside `Popover.Content`, before the Legend block), add:

  ```svelte
  <div class="border-t border-border my-1.5"></div>
  <Button
      type="button"
      variant="ghost"
      size="sm"
      class="w-full text-xs {rareActiveCount > 0 ? 'text-green-400' : 'text-muted-foreground'}"
      onclick={() => { bracketPopoverOpen = false; bracketDialogOpen = true; }}
  >
      All bracket types…{#if rareActiveCount > 0}<Badge class="ml-1.5 bg-green-900 text-green-400 border-0 text-[9px] px-1.5">{rareActiveCount} active</Badge>{/if}
  </Button>
  ```

- [ ] **Step 3: Run unit tests**

  ```bash
  cd web && npm run test:unit
  ```

  Expected: all pass.

- [ ] **Step 4: Commit**

  ```bash
  git add web/src/routes/projects/[id]/tournaments/+page.svelte
  git commit -m "feat(web): replace rare bracket rows with 'All bracket types' button"
  ```

---

## Task 3: Add Dialog with all 10 bracket types

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

- [ ] **Step 1: Add Dialog import**

  In the script block, after the existing `* as Popover` import, add:

  ```ts
  import * as Dialog from '$lib/components/ui/dialog';
  ```

- [ ] **Step 2: Add the Dialog markup**

  In the template, after the closing `</Collapsible.Root>` tag (and still inside the `{:else}` branch), add:

  ```svelte
  <Dialog.Root bind:open={bracketDialogOpen}>
      <Dialog.Content class="sm:max-w-sm">
          <Dialog.Header>
              <Dialog.Title>Bracket Types</Dialog.Title>
          </Dialog.Header>

          <!-- Column headers -->
          <div class="grid grid-cols-[1fr_28px_28px_28px] gap-1 mb-1">
              <span></span>
              <span class="text-xs text-muted-foreground text-center">–</span>
              <span class="text-xs text-muted-foreground text-center">✓</span>
              <span class="text-xs text-muted-foreground text-center">✕</span>
          </div>

          <!-- Common types -->
          {#each COMMON_BRACKET_TYPES as bt}
              {@render bracketRow(bt)}
          {/each}

          <!-- Rare types -->
          <p class="text-[10px] uppercase tracking-wide text-muted-foreground mt-3 mb-1">Rare formats</p>
          {#each RARE_BRACKET_TYPES as bt}
              {@render bracketRow(bt)}
          {/each}

          <!-- Legend -->
          <div class="flex gap-3 flex-wrap border-t border-border pt-2 mt-2">
              <span class="text-[10px] text-muted-foreground"><span class="text-indigo-400">–</span> don't care</span>
              <span class="text-[10px] text-muted-foreground"><span class="text-green-400">✓</span> required</span>
              <span class="text-[10px] text-muted-foreground"><span class="text-red-400">✕</span> excluded</span>
          </div>

          <Dialog.Footer>
              <Button variant="outline" size="sm" onclick={() => { bracketDialogOpen = false; }}>
                  Done
              </Button>
          </Dialog.Footer>
      </Dialog.Content>
  </Dialog.Root>
  ```

- [ ] **Step 3: Run unit tests**

  ```bash
  cd web && npm run test:unit
  ```

  Expected: all pass.

- [ ] **Step 4: Commit**

  ```bash
  git add web/src/routes/projects/[id]/tournaments/+page.svelte
  git commit -m "feat(web): add bracket filter dialog with all 10 bracket types"
  ```

---

## Task 4: Full test suite verification

- [ ] **Step 1: Run the full test suite**

  ```bash
  bash test.sh
  ```

  Expected: PASS for all sections (backend, frontend unit, frontend e2e).

- [ ] **Step 2: Smoke-test the UI manually**

  - Open the tournaments page with the filter panel expanded
  - Confirm the popover shows only 5 common types + "All bracket types…" button
  - Click "All bracket types…" → dialog opens with all 10 types and a "Rare formats" label
  - Set a rare type to required → close dialog → confirm badge shows "1 active" in green
  - Click Reset in the popover header → badge disappears, all types back to neutral
  - Open the dialog again → confirm all types are neutral
