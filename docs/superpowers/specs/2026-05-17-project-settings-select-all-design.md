# Design: Project Settings Page + Select All Attendees

**Date:** 2026-05-17
**Status:** Approved

## Overview

Two independent frontend improvements:
1. A dedicated **Settings tab** on the project detail layout, housing project rename and delete.
2. A **select-all checkbox** in the "From tournament" tab of the Add Players dialog.

---

## Feature 1: Project Settings Page

### Routes and Navigation

- New route: `web/src/routes/projects/[id]/settings/`
- New tab **"Settings"** added at the end of the project layout tab list (after H2H) in `+layout.svelte`.
- The existing Delete button on the projects list cards (`/projects` page) is removed — delete moves exclusively to the settings page.

### Page Layout

The settings page (`+page.svelte`) is divided into two sections:

**Project name**
- An `<Input>` pre-populated with the current project name, with a Save button.
- Submits a `PATCH /projects/{id}` request with `{ "name": "..." }`.
- On success: stays on the settings page; the layout header reflects the updated name via `invalidateAll()`.
- Validation: name must not be empty; max 100 characters (matching backend).

**Danger zone**
- A red-bordered card containing a "Delete project" button with a confirmation prompt.
- Submits a `DELETE /projects/{id}` request (existing endpoint, no backend change needed).
- On success: redirects to `/projects`.

### Backend: New PATCH Endpoint

- **Endpoint:** `PATCH /projects/{project_id}`
- **Request body:** `{ "name": "string" }` — name is required, max 100 chars.
- **Response:** `200 Project` — returns the updated project object.
- **Errors:** `400` for invalid name, `401` for unauthenticated, `404` if project not found.
- OpenAPI spec updated to document this endpoint.

### Server Action

`+page.server.ts` for the settings page provides only `actions` — no `load` needed, since `data.project` is inherited from the layout (`+layout.server.ts` already fetches the project):
- `rename` action: validates name, calls `PATCH /projects/{id}`, returns updated project or error.
- `delete` action: calls `DELETE /projects/{id}`, redirects to `/projects` on success.

---

## Feature 2: Select All Attendees in TournamentTab

### Location

`web/src/lib/components/TournamentTab.svelte` — the "From tournament" tab in the Add Players dialog.

### Behavior

- After entrants are fetched and the list is shown, a **Checkbox + "Select all" label** appears above the entrant list (below the search input, above the ScrollArea).
- **Clicking when not all selectable entrants are selected:** selects all selectable (non-already-added) entrants that match the current search filter.
- **Clicking when all selectable filtered entrants are selected:** deselects all.
- "Already added" entrants are excluded from the toggle — their checkboxes remain disabled.
- The checkbox `checked` state is `true` when all selectable filtered entrants are selected, `false` otherwise. No indeterminate state needed.
- Pure frontend change — no backend work required.

### Logic

```
selectableFiltered = filteredEntrants.filter(e => !alreadyAddedIds.has(e.startgg_user_id))
allSelected = selectableFiltered.every(e => selected.has(e.startgg_user_id))

toggleAll():
  if allSelected → deselect all selectable filtered entrants
  else → select all selectable filtered entrants
```

---

## What Does Not Change

- The projects list page retains its cards and "New project" button; only the Delete button is removed from each card.
- The projects list page server actions lose the `delete` action (it moves to settings).
- All other tabs (Players, Import, Tournaments, Stats, H2H) are unaffected.
- No changes to the worker, common crate, or any other backend crates.
