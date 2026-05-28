# Design: Guest UX Polish for Public Projects

**Date:** 2026-05-28

## Overview

Three small frontend-only changes to improve the experience for unauthenticated visitors viewing a published ranking project. No backend changes required — the underlying access control, published flag, and tab filtering are already fully implemented.

## Background

Published projects are already accessible to unauthenticated visitors at `/projects/{id}`. The frontend already hides editor-only tabs (Players, Import, Settings) for guests. Three UX gaps remain:

1. Guests have no indication they're viewing a shared project, and no path to sign up.
2. The "← Projects" back link takes guests to `/projects`, which redirects them to `/login` — a dead end.
3. Project owners have no way to copy the shareable link from the UI; they must manually grab the browser URL.

## Changes

### 1. Guest banner (`web/src/routes/projects/[id]/+layout.svelte`)

When `!page.data.user` (unauthenticated visitor), render a slim full-width bar before the project header:

> "You're viewing a shared project · [Sign up](/register) to build your own rankings"

The "Sign up" portion is an anchor linking to `/register`.

**Placement:** Inside the outer `<div class="space-y-4">`, above the constrained `<div>` that holds the project header and tabs. This keeps the banner full-width regardless of wide/narrow layout mode.

**Styling:** `bg-muted text-muted-foreground text-sm py-2 px-4` — subtle, non-intrusive. No dismiss button.

### 2. Back link fix (`web/src/routes/projects/[id]/+layout.svelte`)

The "← Projects" / "← Home" back link at the top of the project header is conditional on auth state:

- **Logged-in user** → `← Projects` → `/projects` (unchanged)
- **Guest** → `← Home` → `/`

Condition: `page.data.user ? '/projects' : '/'` and `page.data.user ? '← Projects' : '← Home'`.

### 3. Copy-link in settings (`web/src/routes/projects/[id]/settings/+page.svelte`)

When `data.project.published`, add below the existing "Anyone with the link can view stats, H2H, and rankings." paragraph:

- A `<div class="flex gap-2">` containing:
  - A read-only `<Input>` displaying `{page.url.origin}/projects/{data.project.id}`
  - A `<Button variant="outline">` labelled "Copy link" (briefly shows "Copied!" for 2 seconds after clicking)
- Copy action: `navigator.clipboard.writeText(url)`
- State: `let copied = $state(false)`; on click set to `true`, then `setTimeout(() => copied = false, 2000)`

Requires adding `import { page } from '$app/state'` to the settings page script.

## What is not changing

- No backend changes. The API, access control, published flag, and tab filtering are all already correct.
- No changes to the private-project error page (`+error.svelte`) — it already shows a helpful "This project is private" message with a "Create an account" button.
- The banner is not shown for logged-in users who are viewer-role members — they already know they have an account.
