# shadcn Audit: Replace Hand-Rolled Components — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace four hand-rolled Tailwind patterns with their shadcn-svelte equivalents across three files, plus install the `toggle-group` component.

**Architecture:** Four independent, self-contained changes — each touches one file. No shared state or ordering dependencies between tasks. The ranking rows (`ranking/+page.svelte`) are intentionally excluded: DnD needs raw elements and there is no fitting shadcn list-item component.

**Tech Stack:** SvelteKit, Svelte 5 (runes), shadcn-svelte, bits-ui, Tailwind CSS v4. Tests: Vitest (`npm run test:unit` in `web/`), Playwright (`npm run test:e2e` in `web/`).

---

## Files modified

| File | Change |
|---|---|
| `web/src/lib/components/TournamentTab.svelte` | Remove hand-rolled toggle buttons → `ToggleGroup` |
| `web/src/lib/components/ui/toggle-group/` | Created by `npx shadcn-svelte@latest add` |
| `web/src/routes/projects/[id]/tournaments/+page.svelte` | Tournament container divs → `Card` |
| `web/src/routes/projects/[id]/settings/+page.svelte` | Invite link divs → `Card` |
| `web/src/routes/projects/[id]/h2h/+page.svelte` | Raw header divs → `Popover.Header/Title/Description` |
| `web/CLAUDE.md` | Add Toggle Group to installed-components table |

---

## Task 1: Install ToggleGroup and replace sort toggle in TournamentTab

**Files:**
- Modify: `web/src/lib/components/TournamentTab.svelte`
- Create: `web/src/lib/components/ui/toggle-group/` (via CLI)
- Modify: `web/CLAUDE.md`

- [ ] **Step 1: Run unit tests to establish baseline**

```bash
cd web && npm run test:unit
```

Expected: all tests pass (green).

- [ ] **Step 2: Install the toggle-group component**

```bash
cd web && npx shadcn-svelte@latest add --yes --overwrite toggle-group
```

Expected output: files written under `src/lib/components/ui/toggle-group/`. Verify with:

```bash
ls web/src/lib/components/ui/toggle-group/
```

Expected: at least `toggle-group.svelte`, `toggle-group-item.svelte`, `index.ts`.

- [ ] **Step 3: Add ToggleGroup import and replace the sort toggle**

In `web/src/lib/components/TournamentTab.svelte`, add this import after the existing imports (around line 8):

```svelte
import * as ToggleGroup from '$lib/components/ui/toggle-group';
```

Then find the sort toggle block (inside the `{#if activeTab !== 'all'}` block, currently lines ~192–206):

```svelte
<div class="flex rounded-md border overflow-hidden flex-shrink-0">
    <Button
        variant={sortMode === 'placement' ? 'default' : 'ghost'}
        size="sm"
        class="rounded-none h-8 text-xs"
        onclick={() => (sortMode = 'placement')}
    >Placement</Button>
    <Button
        variant={sortMode === 'seed' ? 'default' : 'ghost'}
        size="sm"
        class="rounded-none border-l h-8 text-xs"
        onclick={() => (sortMode = 'seed')}
    >Seed</Button>
</div>
```

Replace it with:

```svelte
<ToggleGroup.Root
    type="single"
    value={sortMode}
    onValueChange={(v) => { if (v === 'placement' || v === 'seed') sortMode = v; }}
    class="flex-shrink-0"
>
    <ToggleGroup.Item value="placement" class="h-8 text-xs">Placement</ToggleGroup.Item>
    <ToggleGroup.Item value="seed" class="h-8 text-xs">Seed</ToggleGroup.Item>
</ToggleGroup.Root>
```

`onValueChange` is used instead of `bind:value` because ToggleGroup's value type is `string` — binding directly to `sortMode: 'placement' | 'seed'` would cause a TypeScript error on write-back. The guard `if (v === 'placement' || v === 'seed')` also handles the case where single-type ToggleGroup emits an empty string on deselect (bits-ui prevents deselection in single mode, but the guard is a cheap safety net). The `sortMode` state variable declaration is unchanged.

- [ ] **Step 4: Update CLAUDE.md installed-components table**

In `web/CLAUDE.md`, add a row to the installed components table:

```markdown
| Toggle Group | `$lib/components/ui/toggle-group` |
```

- [ ] **Step 5: Run unit tests to verify no regression**

```bash
cd web && npm run test:unit
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/lib/components/TournamentTab.svelte \
        web/src/lib/components/ui/toggle-group \
        web/CLAUDE.md
git commit -m "refactor: replace sort toggle buttons with ToggleGroup in TournamentTab"
```

---

## Task 2: Replace tournament container divs with Card

**Files:**
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`

The tournament containers are currently `<div class="rounded-md border border-border">` wrappers. `Card.Root` replaces these, providing the ring, rounded corners, and shadow from the design system. Using `py-0 gap-0` on `Card.Root` keeps the compact layout (no extra vertical padding or gap between header and content).

- [ ] **Step 1: Add Card import**

In `web/src/routes/projects/[id]/tournaments/+page.svelte`, the current imports include `Badge`, `Button`, `Checkbox`, etc. Add Card:

```svelte
import * as Card from '$lib/components/ui/card';
```

- [ ] **Step 2: Replace each tournament container**

Find the `{#each visibleTournaments as tournament (tournament.id)}` block (around line 455). Replace the outer div and its children:

**Before:**
```svelte
<div class="rounded-md border border-border">
    <div class="flex items-start justify-between p-3">
        <div>
            <p class="font-medium">{tournament.name}</p>
            <p class="text-xs text-muted-foreground">
                {[tournament.city, tournament.addr_state, tournament.country_code]
                    .filter(Boolean)
                    .join(', ')}
                {tournament.online ? '(Online)' : ''}
                {tournament.start_at ? '· ' + formatDate(tournament.start_at) : ''}
            </p>
        </div>
        <Badge variant="outline">
            {tournament.events.length} event{tournament.events.length !== 1 ? 's' : ''}
        </Badge>
    </div>
    <div class="divide-y divide-border border-t border-border">
        {#each tournament.events as event (event.id)}
```

**After:**
```svelte
<Card.Root class="py-0 gap-0">
    <Card.Header class="p-3">
        <Card.Title class="text-sm font-medium">{tournament.name}</Card.Title>
        <Card.Description class="text-xs">
            {[tournament.city, tournament.addr_state, tournament.country_code]
                .filter(Boolean)
                .join(', ')}
            {tournament.online ? '(Online)' : ''}
            {tournament.start_at ? '· ' + formatDate(tournament.start_at) : ''}
        </Card.Description>
        <Card.Action>
            <Badge variant="outline">
                {tournament.events.length} event{tournament.events.length !== 1 ? 's' : ''}
            </Badge>
        </Card.Action>
    </Card.Header>
    <Card.Content class="p-0">
        <div class="divide-y divide-border border-t border-border">
            {#each tournament.events as event (event.id)}
```

Also close the new elements at the end of the each block. **Before** (closing):
```svelte
        </div>
    </div>
</div>
```

**After** (closing):
```svelte
            </div>
        </div>
    </Card.Content>
</Card.Root>
```

Notes on the replacement:
- `Card.Root class="py-0 gap-0"`: removes the card's vertical padding and flex gap so the header and event list sit flush.
- `Card.Header class="p-3"`: matches the current 12px all-around header padding.
- `Card.Title class="text-sm font-medium"`: `text-sm` preserves the current size (Card.Title defaults to `text-base` which would be larger); `font-medium` matches the current `<p class="font-medium">`.
- `Card.Description class="text-xs"`: `text-xs` overrides Card.Description's default `text-sm`.
- `Card.Action`: uses the card header's grid to position the Badge in the right column, spanning both title and description rows.
- `Card.Content class="p-0"`: removes horizontal padding so the event rows extend full-width.

- [ ] **Step 3: Run unit tests**

```bash
cd web && npm run test:unit
```

Expected: all tests pass. The filter tests (`filter.test.ts`) test pure logic functions and are unaffected.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/projects/[id]/tournaments/+page.svelte
git commit -m "refactor: replace hand-rolled tournament containers with Card"
```

---

## Task 3: Replace invite link items with Card

**Files:**
- Modify: `web/src/routes/projects/[id]/settings/+page.svelte`

The invite link rows are currently `<div class="flex items-center justify-between rounded-md border p-3 gap-2">`. Replace with Card — same strategy as Task 2.

- [ ] **Step 1: Verify Card is already imported**

`settings/+page.svelte` already imports `* as Table`, `* as Select`, etc. Check that `* as Card` is NOT already imported (it is not, based on the audit). Add the import after the existing ones:

```svelte
import * as Card from '$lib/components/ui/card';
```

- [ ] **Step 2: Replace each invite link container**

Find the `{#each data.inviteLinks as link}` block (around line 138 in settings). 

**Before:**
```svelte
{#each data.inviteLinks as link}
    <div class="flex items-center justify-between rounded-md border p-3 gap-2">
        <div class="text-sm space-y-0.5">
            <span class="font-medium capitalize">{link.role}</span>
            {#if link.expires_at}
                <span class="text-muted-foreground">
                    · expires {new Date(link.expires_at).toLocaleDateString()}</span
                >
            {/if}
        </div>
        <div class="flex gap-2">
            <Button
                type="button"
                variant="outline"
                size="sm"
                onclick={() => navigator.clipboard.writeText(`${location.origin}/invite/${link.id}`)}
            >
                Copy link
            </Button>
            <form method="POST" action="?/revokeInviteLink" use:enhance class="inline">
                <input type="hidden" name="link_id" value={link.id} />
                <Button type="submit" variant="ghost" size="sm">Revoke</Button>
            </form>
        </div>
    </div>
{/each}
```

**After:**
```svelte
{#each data.inviteLinks as link}
    <Card.Root class="py-0 gap-0">
        <Card.Header class="p-3">
            <Card.Title class="text-sm font-medium capitalize">{link.role}</Card.Title>
            {#if link.expires_at}
                <Card.Description class="text-xs">
                    expires {new Date(link.expires_at).toLocaleDateString()}
                </Card.Description>
            {/if}
            <Card.Action class="flex gap-2">
                <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onclick={() => navigator.clipboard.writeText(`${location.origin}/invite/${link.id}`)}
                >
                    Copy link
                </Button>
                <form method="POST" action="?/revokeInviteLink" use:enhance class="inline">
                    <input type="hidden" name="link_id" value={link.id} />
                    <Button type="submit" variant="ghost" size="sm">Revoke</Button>
                </form>
            </Card.Action>
        </Card.Header>
    </Card.Root>
{/each}
```

Notes:
- `Card.Root class="py-0 gap-0"`: compact card, no outer vertical padding.
- `Card.Header class="p-3"`: matches the current `p-3`.
- `Card.Title class="text-sm font-medium capitalize"`: preserves role label styling.
- `Card.Description class="text-xs"`: shown only when `expires_at` is set. Card.Header's grid layout handles the conditional row automatically via `has-data-[slot=card-description]:grid-rows-[auto_auto]`.
- `Card.Action class="flex gap-2"`: positions buttons flush right, spanning title/description rows.

- [ ] **Step 3: Run unit tests**

```bash
cd web && npm run test:unit
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/projects/[id]/settings/+page.svelte
git commit -m "refactor: replace hand-rolled invite link containers with Card"
```

---

## Task 4: Replace H2H popover header with Popover sub-components

**Files:**
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`

The popover already imports `* as Popover from '$lib/components/ui/popover'`. `Popover.Header`, `Popover.Title`, and `Popover.Description` are already exported from that module — no new install or import change needed.

`Popover.Header` renders `<div class="flex flex-col gap-1 text-sm">`.  
`Popover.Title` renders `<div class="font-medium">`.  
`Popover.Description` renders `<p class="text-muted-foreground text-sm">`.

The outer `justify-between` div and the close `Button` stay unchanged — `Popover.Close` is an unstyled primitive and the Popover is already controlled via the `open` prop, so the existing `onclick={() => (selectedPair = null)}` close pattern is correct.

- [ ] **Step 1: Run the H2H unit tests to establish baseline**

```bash
cd web && npm run test:unit -- h2h
```

Expected: all 7 H2H tests pass.

- [ ] **Step 2: Replace the popover header structure**

In `web/src/routes/projects/[id]/h2h/+page.svelte`, find the popover header inside the `{:else if selectedPair}` branch (around line 157). 

**Before:**
```svelte
<div class="mb-3 flex items-start justify-between gap-2 border-b border-border pb-2">
    <div>
        <p class="font-semibold text-sm">{selectedPair.rowPlayer.name} vs {selectedPair.colPlayer.name}</p>
        <p class="text-xs text-muted-foreground">{selectedPair.wins} wins · {selectedPair.losses} losses</p>
    </div>
    <Button
        variant="ghost"
        size="icon"
        onclick={() => (selectedPair = null)}
        aria-label="Close"
    >×</Button>
</div>
```

**After:**
```svelte
<div class="mb-3 flex items-start justify-between gap-2 border-b border-border pb-2">
    <Popover.Header>
        <Popover.Title class="text-sm font-semibold">{selectedPair.rowPlayer.name} vs {selectedPair.colPlayer.name}</Popover.Title>
        <Popover.Description class="text-xs">{selectedPair.wins} wins · {selectedPair.losses} losses</Popover.Description>
    </Popover.Header>
    <Button
        variant="ghost"
        size="icon"
        onclick={() => (selectedPair = null)}
        aria-label="Close"
    >×</Button>
</div>
```

Notes:
- The existing `import * as Popover from '$lib/components/ui/popover'` already exports `Header`, `Title`, and `Description` — no import change needed.
- `Popover.Title class="text-sm font-semibold"`: `Popover.Title` defaults to `font-medium`; `font-semibold` overrides to match the current `<p class="font-semibold text-sm">`.
- `Popover.Description class="text-xs"`: overrides `Popover.Description`'s default `text-sm` to match the current `text-xs`.
- The outer `<div>`, the close `Button`, and the `onclick={() => (selectedPair = null)}` are all unchanged.

- [ ] **Step 3: Run the H2H unit tests**

```bash
cd web && npm run test:unit -- h2h
```

Expected: all 7 H2H tests pass. The test `'does not show popover content before any cell is clicked'` checks `queryByText(/wins ·/i)` — `Popover.Description` still renders the same text, so this passes.

- [ ] **Step 4: Run the full unit test suite**

```bash
cd web && npm run test:unit
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/[id]/h2h/+page.svelte
git commit -m "refactor: use Popover.Header/Title/Description in H2H popover"
```

---

## Final verification

- [ ] **Run the full test suite**

From the repo root:

```bash
bash test.sh
```

Expected: PASS for all sections (backend, frontend unit, frontend e2e). The e2e tests cover the H2H grid (`h2h page renders the player grid`) and the projects page — both should still pass since text content is preserved.
