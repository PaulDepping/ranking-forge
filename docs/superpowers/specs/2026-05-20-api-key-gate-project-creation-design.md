# Design: API Key Gate for Project Creation

**Date:** 2026-05-20  
**Status:** Approved

## Summary

Gate project creation behind having a start.gg API key configured. Both the frontend (new project page) and the backend (`POST /projects`) enforce this requirement. Motivation: game search already requires an API key, and a project is not useful without running an import, which also requires one.

## Frontend

**File:** `web/src/routes/projects/new/+page.server.ts`

Add a `load` function that exposes `user.has_startgg_key` to the page. `locals.user` is already populated by the server hook on every request, so no extra API call is needed.

**File:** `web/src/routes/projects/new/+page.svelte`

Wrap the existing form in `{#if data.user.has_startgg_key}...{/if}`. Add an `{:else}` block with a `Card` callout (matching the import page pattern):

> "A start.gg API key is required to create projects. Add your key in [account settings]."

The link points to `/account`. No link to `start.gg/admin/profile/developer` is needed here — the account settings page already provides it.

The game search's silent empty-result behavior is left unchanged; the callout blocks the form before the user reaches it.

## Backend

**File:** `backend/crates/api/src/routes/projects.rs`

Add an early guard at the top of `create_project`:

```rust
if user.startgg_api_key.is_none() {
    return Err(AppError::UnprocessableEntity(
        "A start.gg API key is required to create projects".into(),
    ));
}
```

`AuthUser(user)` already carries `startgg_api_key: Option<String>`, so no DB query is needed. This mirrors the identical pattern in `search_games`.

Add one test: a user with no API key gets 422 when `POST /projects`.

## What is not changing

- No schema or migration changes.
- No new routes.
- The game field remains optional in the form — the gate is on project creation itself, not on game selection.
- The import page callout is unchanged.
