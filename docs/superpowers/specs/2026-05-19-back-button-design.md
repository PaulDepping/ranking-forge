# Back Button Fix — Player Detail Page

**Date:** 2026-05-19  
**Status:** Approved

## Problem

The back button on the player detail page calls `history.back()`. When a user arrives via a direct link (no prior browser history for this site), this navigates them off the site entirely.

## Goal

Preserve navigational context when the user arrived via in-app navigation, and fall back to the players list when no context is available (direct link, hard refresh).

## Approach

Use a client-side Svelte store updated by SvelteKit's `afterNavigate` lifecycle hook. No server changes required.

## Changes

### 1. `web/src/lib/stores/navigation.ts` (new file)

Export a `writable<string | null>` store named `previousPage`, initialized to `null`.

### 2. `web/src/routes/+layout.svelte`

Call `afterNavigate` (from `$app/navigation`) and write `navigation.from?.url.pathname ?? null` into `previousPage` on every navigation.

### 3. `web/src/routes/projects/[id]/players/[player_id]/+page.svelte`

Replace `onclick={() => history.back()}` with a handler that:
- Reads `$previousPage`
- If non-null: `goto($previousPage)`
- Otherwise: `goto(\`/projects/${data.projectId}/players\`)`

## Data Flow

```
User navigates in-app
  → afterNavigate fires in +layout.svelte
  → previousPage store updated with from.url.pathname

User clicks Back on player detail page
  → reads $previousPage
  → non-null: goto(previousPage)
  → null (direct link): goto(/projects/[id]/players)
```

## Error Handling

The only failure case is `previousPage === null`, handled by the fallback URL. The fallback is always constructible from `data.projectId`, which is guaranteed by the server load function.

## Testing

Two e2e cases added to the existing player page test file:

1. Navigate from players list → player detail → click Back → assert URL is players list.
2. Navigate directly to player detail URL → click Back → assert URL is players list (fallback).
