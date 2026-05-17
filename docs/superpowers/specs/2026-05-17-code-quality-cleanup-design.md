# Code Quality Cleanup — Design Doc

**Date:** 2026-05-17
**Scope:** Full-stack (frontend + backend); refactors + UX improvements

---

## Overview

A structured cleanup pass across the RankingForge codebase, organised into three independently committable layers:

1. **Shared utilities & component extraction** — eliminate duplication, extract reusable primitives
2. **shadcn policy fixes** — replace all raw `<button>`/`<label>` elements with shadcn equivalents
3. **UX improvements** — replace `window.confirm()` with AlertDialog, fix SvelteKit navigation

No new features. No behaviour changes except where explicitly noted in Layer 3.

---

## Layer 1 — Shared utilities & component extraction

### Frontend

#### `DateRangePicker.svelte` (new component)

**Problem:** The `Popover + Calendar` date-picker pattern is copy-pasted four times across two files:
- `import/+page.svelte` — "From date" and "To date" pickers (×2)
- `tournaments/+page.svelte` — "From" and "To" filter pickers (×2)

**Fix:** Install `range-calendar` via shadcn-svelte CLI, then extract a single `src/lib/components/DateRangePicker.svelte` component.

**Component contract:**
```ts
// Props
let {
  value,    // DateRange | undefined — { start: CalendarDate, end: CalendarDate } or undefined
  onSelect, // (range: DateRange | undefined) => void
  placeholder = 'Pick date range',
}: {
  value: DateRange | undefined;
  onSelect: (range: DateRange | undefined) => void;
  placeholder?: string;
} = $props();
```

**Behaviour:** The popover only closes once the user has selected both `start` and `end` (full range enforced — no partial selections leak to callers). A "Clear" button inside the popover resets both bounds together. Trigger button displays the formatted range (e.g. `"01.01.2024 – 31.12.2024"`) or the placeholder when unset.

**Installation step:** `npx shadcn-svelte@latest add --yes --overwrite range-calendar`

**Call sites updated:**
- `import/+page.svelte` — replace the two-column "From date / To date" grid with a single `<DateRangePicker>`; derive `afterDateStr` / `beforeDateStr` from the range value for the hidden form inputs
- `tournaments/+page.svelte` — replace the From/To popover row in the filter panel with a single `<DateRangePicker>`; remove `dateFromOpen`, `dateToOpen`, `dateFrom`, `dateTo` states and replace with a single `dateRange` state

#### `winRate()` in `utils.ts`

**Problem:** Duplicated across two files with a subtle difference:
- `stats/+page.svelte`: returns `'0%'` when total is 0
- `ranking/+page.svelte`: returns `''` when total is 0

**Fix:** Add to `src/lib/utils.ts`:
```ts
export function winRate(wins: number, losses: number, zeroValue = ''): string {
  const total = wins + losses;
  if (total === 0) return zeroValue;
  return `${Math.round((wins / total) * 100)}%`;
}
```

Update callers:
- `stats/+page.svelte`: `winRate(wins, losses, '0%')`
- `ranking/+page.svelte`: `winRate(s.wins.length, s.losses.length)` (default empty string)

Remove the local `winRate` functions from both files.

---

### Backend (`backend/crates/api/src/routes/players.rs`)

#### `create_player_with_account` helper

**Problem:** The two-step "insert player → insert startgg_account" SQL block is copy-pasted verbatim between `bulk_add_players` and `add_players_by_handles`.

**Fix:** Extract a private async function:
```rust
async fn create_player_with_account(
    pool: &PgPool,
    project_id: Uuid,
    name: &str,
    user_id: i64,
    handle: &str,
    display_name: Option<&str>,
) -> sqlx::Result<Uuid>
```

Returns the new player's UUID. Both handlers call this instead of repeating the SQL.

#### `strip_startgg_url_prefix` helper

**Problem:** `normalize_handle` and `normalize_tournament_handle` share four identical prefix-stripping steps (`https://`, `http://`, `www.start.gg/`, `start.gg/`).

**Fix:** Extract:
```rust
fn strip_startgg_url_prefix(s: &str) -> &str {
    s.trim_start_matches("https://")
     .trim_start_matches("http://")
     .trim_start_matches("www.start.gg/")
     .trim_start_matches("start.gg/")
}
```

`normalize_handle` and `normalize_tournament_handle` each call this first, then apply their own trailing strip (`user/` vs `tournament/` + split on `/`).

#### `PlayerResponse` construction

**Problem:** `add_player` and `rename_player` both manually construct `PlayerResponse { id: p.id, project_id: p.project_id, name: p.name, created_at: p.created_at, accounts: vec![] }`.

**Fix:** Add an associated function:
```rust
impl PlayerResponse {
    fn from_player(p: Player, accounts: Vec<AccountResponse>) -> Self {
        PlayerResponse { id: p.id, project_id: p.project_id, name: p.name, created_at: p.created_at, accounts }
    }
}
```

Both handlers call `PlayerResponse::from_player(player, vec![])`. `list_players` calls `PlayerResponse::from_player(p, accounts)`.

---

## Layer 2 — shadcn policy fixes

### Raw `<button>` replacements

**`players/+page.svelte:103`** — `<button type="submit">×</button>` inside a `Badge` (account remove button)
→ `Button` `variant="ghost"` `size="icon"` with tight height/width class to fit inside the Badge

**`players/+page.svelte:111`** — `<button>+ add account</button>` with hand-rolled underline classes
→ `Button` `variant="link"` `size="sm"`

**`h2h/+page.svelte:124`** — raw `<button>` for H2H matrix cells with hand-rolled conditional ring/color classes
→ `Button` `variant="ghost"` with class overrides preserving win/loss/selected colour states

**`h2h/+page.svelte:182`** — raw `<button>` for set rows in the side panel
→ `Button` `variant="ghost"` `class="w-full justify-start h-auto"`

**`stats/+page.svelte:55,69`** — raw `<button>` for wins and losses list rows
→ `Button` `variant="ghost"` `class="w-full justify-start h-auto text-xs"`

**`tournaments/+page.svelte:222`** — raw `<button>` in `bracketRow` snippet for neutral/required/excluded toggles
→ `Button` `size="icon"` with conditional `variant` matching current active state colour

### Raw `<label>` replacement

**`tournaments/+page.svelte:507`** — raw `<label class="flex cursor-pointer items-center ...">` as clickable event row
→ shadcn `Label` with the same layout classes

### Component extractions

#### `AccountBadge.svelte` (new component)

The badge-with-remove-button pattern in `players/+page.svelte` (one per linked account) is a self-contained unit: a `<form>`, a `Badge`, and a submit `Button`. Extracting it removes ~25 lines of inline form boilerplate per account and makes the player card readable.

**Props:** `playerId: string`, `accountId: string`, `displayName: string | null`, `handle: string`

#### `PlayerCard.svelte` (new component)

The `{#if editingPid === player.id} ... {:else} ...` block in `players/+page.svelte` is ~90 lines of inline edit/view mode logic. Extracting it makes the page template a clean `{#each}` loop.

**Props:** `player: Player`, `isEditing: boolean`, `form: ActionData`, callbacks: `onEdit`, `onCancelEdit`, `onOpenLinkDialog`

Uses `AccountBadge.svelte` internally for each linked account.

---

## Layer 3 — UX improvements

### Install `alert-dialog`

`npx shadcn-svelte@latest add --yes --overwrite alert-dialog`

Not currently installed. Required for all three confirm replacements below.

### Replace `window.confirm()` with AlertDialog (×3)

**Common pattern across all three sites:**
1. The form's submit button changes to `type="button"`
2. `bind:this={formEl}` on the `<form>` element
3. Button click handler opens the AlertDialog (or submits directly if no confirmation needed)
4. AlertDialog "Confirm" action calls `formEl.requestSubmit()`, which re-triggers `use:enhance` normally

#### `players/+page.svelte` — "Remove {player.name}?"

Handled inside `PlayerCard.svelte`. The delete button opens a `AlertDialog` scoped to the card. Confirm calls `formEl.requestSubmit()` on the delete form.

#### `settings/+page.svelte` — "Delete this project? This cannot be undone."

Add `let deleteDialogOpen = $state(false)` and `let deleteFormEl: HTMLFormElement`. Delete button becomes `type="button"` and sets `deleteDialogOpen = true`. AlertDialog confirm calls `deleteFormEl.requestSubmit()`.

AlertDialog content:
- Title: "Delete this project?"
- Description: "Permanently removes all players, tournaments, and stats. This cannot be undone."
- Cancel + destructive Confirm button

#### `import/+page.svelte` — "An import is already running. Start a new one?"

Add `let importDialogOpen = $state(false)` and `let importFormEl: HTMLFormElement`. Submit button becomes `type="button"`. Click handler: if job is active, set `importDialogOpen = true`; otherwise call `importFormEl.requestSubmit()`. AlertDialog confirm calls `importFormEl.requestSubmit()`.

AlertDialog content:
- Title: "Import already running"
- Description: "An import is currently in progress. Start a new one anyway?"
- Cancel + Confirm button

### Fix `location.href` navigation

**`+layout.svelte:14`** — `location.href = '/login'` after logout causes a full page reload.

Fix: import `goto` from `$app/navigation` and replace with `goto('/login')`.

---

## Files changed summary

### New files
- `web/src/lib/components/DateRangePicker.svelte`
- `web/src/lib/components/AccountBadge.svelte`
- `web/src/lib/components/PlayerCard.svelte`
- `web/src/lib/components/ui/range-calendar/` (installed via shadcn CLI)
- `web/src/lib/components/ui/alert-dialog/` (installed via shadcn CLI)

### Modified files
- `web/src/lib/utils.ts` — add `winRate()`
- `web/src/routes/+layout.svelte` — `goto()` fix
- `web/src/routes/projects/[id]/import/+page.svelte` — DateRangePicker, AlertDialog
- `web/src/routes/projects/[id]/tournaments/+page.svelte` — DateRangePicker, label fix, bracketRow button fix
- `web/src/routes/projects/[id]/players/+page.svelte` — PlayerCard extraction, AccountBadge extraction, AlertDialog
- `web/src/routes/projects/[id]/stats/+page.svelte` — winRate, button fix
- `web/src/routes/projects/[id]/ranking/+page.svelte` — winRate
- `web/src/routes/projects/[id]/h2h/+page.svelte` — button fixes
- `web/src/routes/projects/[id]/settings/+page.svelte` — AlertDialog
- `backend/crates/api/src/routes/players.rs` — helpers, PlayerResponse::from_player

---

## Testing

- Run `bash test.sh` from repo root after each layer to confirm nothing regresses
- For Layer 3 AlertDialog changes: manually test the confirmation flow in browser (unit tests can't test `window.confirm()` removal since e2e tests can't log in, but the form submission path is covered by existing API tests)
- No new tests required — all changes are structural/cosmetic or replace equivalent behaviour
