---
title: Player linking improvement — frontend
date: 2026-05-16
status: approved
---

# Player linking improvement — frontend

## Problem

The Players page currently has an inline name-only form for adding players and a per-player dialog for linking a start.gg account by slug. The backend now exposes bulk-add and tournament-entrants endpoints (see backend spec `2026-05-15-player-linking-improvement-design.md`) but the frontend has not been updated to use them.

## Goals

- Replace the inline add-player form with a single "Add players" button that opens a three-tab dialog.
- Support adding players from a tournament entrant list, by start.gg handle/URL, or by name only.
- Allow renaming a player inline without leaving the page.
- Update the existing "add account" dialog to accept handle, full slug, or full URL (label and field name update only).
- Update `Account.slug` → `Account.handle` in frontend types to match the renamed backend field.

## Component structure

### New files

| File | Responsibility |
|---|---|
| `web/src/lib/components/AddPlayersDialog.svelte` | Dialog shell: owns open/close state, renders `Dialog` + `Tabs`, passes `projectId` and `players` prop down to tabs |
| `web/src/lib/components/TournamentTab.svelte` | Full tournament flow: URL input → fetch entrants → searchable checklist → bulk add |
| `web/src/lib/components/HandleTab.svelte` | Textarea (one handle per line) → submit → per-line results (created / skipped / not found) |
| `web/src/lib/components/NameTab.svelte` | Single name input → POST /players → clear and stay open for quick multi-add |

### Modified files

| File | Change |
|---|---|
| `web/src/routes/projects/[id]/players/+page.svelte` | Remove inline add form; add `<AddPlayersDialog>`; add inline rename state per player row; show `account.handle` in badges |
| `web/src/routes/projects/[id]/players/+page.server.ts` | Add `renamePlayer` action; update `linkAccount` to send `handle` not `slug` |
| `web/src/lib/types.ts` | Rename `Account.slug` → `Account.handle`; add `TournamentEntrant` and `BulkAddResult` |
| `web/tests/mock-api.js` | Add handlers for `GET /tournament-entrants`, `POST /players/bulk`, `PATCH /players/:pid` |

## Page layout

The Players page heading row shows the page title on the left and a single **"Add players"** button on the right. The inline name-input form is removed entirely. The empty-state description updates to point users to the button.

## AddPlayersDialog

A `Dialog` wrapping a `Tabs` with three triggers: **From tournament**, **By handle**, **By name**.

Props:
- `projectId: string`
- `players: Player[]` — used by `TournamentTab` to detect already-added entrants

After any successful mutation, the component calls `invalidateAll()` and closes the dialog (exception: `NameTab` stays open after each add for quick multi-add).

## TournamentTab

**State:** `tournamentInput` (string), `loading` (bool), `entrants` (`TournamentEntrant[]`), `search` (string), `selected` (Set of `startgg_user_id`), `submitting` (bool).

**Flow:**
1. User pastes a tournament URL or bare handle. Clicks **Fetch**.
2. `GET /projects/:id/tournament-entrants?tournament=<handle>` (client-side fetch via `api.ts`).
3. Response populates a scrollable, searchable checklist. Each row shows gamer tag + bare handle + a checkbox.
4. Already-added entrants (matched by `startgg_user_id` against `players[*].accounts`) are shown greyed out and non-selectable, with an "already added" badge.
5. Search input filters by gamer tag or handle in real time ($derived).
6. Footer: `"N selected · M already added"` label + **"Add N players"** button (disabled when N = 0).
7. On submit: `POST /projects/:id/players/bulk` → `invalidateAll()` → close dialog.

## HandleTab

**State:** `input` (string, textarea), `submitting` (bool), `results` (`ByHandlesResult[]`).

**Flow:**
1. Textarea accepts one entry per line: bare handle, full slug, or full URL.
2. **Add players** submits `POST /projects/:id/players/by-handles` with `{ handles: string[] }`. The backend resolves each handle against the start.gg API and returns `ByHandlesResult[]`.
3. Results shown inline: green ✓ created, grey – skipped, red ✕ not found — one row per input line.
4. **Done** button closes the dialog and calls `invalidateAll()` (only if at least one was created).
5. Textarea and results clear when the tab is switched or the dialog is closed.

## NameTab

**State:** `name` (string), `submitting` (bool), `error` (string | null).

**Flow:**
1. Single name input + **Add player** button.
2. `POST /projects/:id/players` with `{ name }`.
3. On success: clear input, call `invalidateAll()`, stay on tab (ready for another).
4. On error: show inline error message below the input.

## Inline rename

Each player row gains an **Edit** button. Clicking it sets `editingPid` + `editingName` reactive state in the page. The row swaps its name display for an `Input` pre-filled with the current name, plus **Save** and **Cancel** buttons.

- **Save**: submits the `renamePlayer` form action via `use:enhance`. On success: `invalidateAll()` + clear editing state.
- **Cancel**: clears editing state immediately.
- Only one row can be in edit mode at a time (setting a new `editingPid` implicitly cancels any prior edit).
- Returns 422 if name is empty (surfaced as an inline error below the input).

`renamePlayer` action in `+page.server.ts`:
```
PATCH /projects/:id/players/:pid  { name }
```

## Link account dialog update

The existing per-player "add account" dialog:
- Field label: `"start.gg handle"` (was `"start.gg slug"`)
- Placeholder: `"mang0"` (was `"user/abc123"`)
- Helper text: `"Accepts bare handle, full slug, or full URL"`
- Field name sent to server: `handle` (was `slug`)
- Server action updated to POST `{ handle }` instead of `{ slug }`

## Types

```ts
// web/src/lib/types.ts

interface Account {
  id: string;
  startgg_user_id: number;
  handle: string;          // renamed from slug
  display_name: string | null;
}

interface TournamentEntrant {
  startgg_user_id: number;
  handle: string;
  name: string;
}

// Response from POST /players/bulk (TournamentTab)
interface BulkAddResult {
  name: string;
  handle: string;
  status: 'created' | 'skipped';
}

// Response from POST /players/by-handles (HandleTab)
interface ByHandlesResult {
  handle: string;
  name: string | null;     // null when status is 'not_found'
  status: 'created' | 'skipped' | 'not_found';
}
```

## Testing

### Playwright e2e (`tests/mock-api.js` + `tests/projects.test.ts`)

New mock-api handlers:
- `GET /projects/:id/tournament-entrants?tournament=*` → returns 2 entrants (one already in `MOCK_PLAYERS`, one new)
- `POST /projects/:id/players/bulk` → returns `[{ name, handle, status: 'created' }]` (used by TournamentTab)
- `POST /projects/:id/players/by-handles` → returns `[{ handle, name, status: 'created' }]` (used by HandleTab)
- `PATCH /projects/:id/players/:pid` → returns updated player

New e2e tests:
- Players page shows **"Add players"** button and no inline name form
- Dialog opens with three tabs: From tournament, By handle, By name
- **By name** tab: submitting a name calls the API and the dialog stays open
- **Rename**: clicking Edit on a player row shows an inline input; saving calls PATCH and clears edit mode

### No new unit tests

`TournamentTab`, `HandleTab`, and `NameTab` make direct API calls covered by the Playwright mock server. `AddPlayersDialog` is thin dialog chrome. No logic warrants isolated unit tests.

## Out of scope

- Pagination of tournament entrant lists (backend already handles `perPage` halving)
- Bulk rename
- Undo/redo for bulk add
