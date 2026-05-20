# Editor Route Access Control

**Date:** 2026-05-20  
**Status:** Approved

## Problem

Logged-out visitors to public projects can access editor-only pages (`/players`, `/import`) by navigating directly to their URLs. The tab UI correctly hides these tabs for non-editors, but there is no server-side guard enforcing the restriction. Visiting `/projects/{id}` also unconditionally redirects to `/players`, which forces public visitors through a login wall before they can see anything.

## Goals

- Block unauthenticated and viewer-role visitors from accessing `/players` and `/import`
- Redirect blocked visitors to `/login` with a return URL so they land back where they intended after logging in
- Send public visitors who hit `/projects/{id}` to `/ranking` by default
- Keep the existing behaviour for editors and owners (they still land on `/players`)
- Make the pattern easy to extend: future editor-only pages just go in the route group

## Approach: SvelteKit Route Groups

Use a `(editor)` route group under `projects/[id]/`. Route groups are parenthetical folder names that do not affect URL paths, so `/projects/{id}/players` continues to resolve correctly after the move.

A single `+layout.server.ts` inside the group enforces the role check for every page in the group, using `parent()` to read the already-loaded project (no extra API call).

## Route Structure

```
routes/projects/[id]/
  +layout.svelte          unchanged
  +layout.server.ts       unchanged — loads project, provides user_role
  +page.server.ts         updated — role-aware redirect (see below)
  +error.svelte           unchanged

  (editor)/
    +layout.server.ts     NEW — enforces editor/owner role
    players/
      +page.svelte        moved (no content changes)
      +page.server.ts     moved (no content changes)
    import/
      +page.svelte        moved (no content changes)
      +page.server.ts     moved (no content changes)

  tournaments/            unchanged
  stats/                  unchanged
  h2h/                    unchanged
  ranking/                unchanged
  settings/               unchanged — keeps its own owner-only guard
```

`settings` is left with its own `parent()` + `error(403)` guard. A future `(owner)` group can be introduced if more owner-only pages are added.

## Implementation Details

### `(editor)/+layout.server.ts` (new file)

```ts
import { redirect } from '@sveltejs/kit';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ parent, url }) => {
    const { project } = await parent();
    const role = project.user_role;
    if (role !== 'editor' && role !== 'owner') {
        redirect(303, `/login?redirect=${encodeURIComponent(url.pathname)}`);
    }
};
```

- `viewer` is intentionally excluded — viewers cannot manage players or run imports
- `url.pathname` is used (not `url.href`) so query strings are not carried into the redirect param

### `projects/[id]/+page.server.ts` (updated)

```ts
import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, parent }) => {
    const { project } = await parent();
    const role = project.user_role;
    if (role === 'editor' || role === 'owner') {
        redirect(303, `/projects/${params.id}/players`);
    }
    redirect(303, `/projects/${params.id}/ranking`);
};
```

Editors and owners land on `players` as before. Viewers and logged-out visitors land on `ranking`.

### `login/+page.server.ts` (updated)

`load` reads the `redirect` query param and passes it to the page:

```ts
export const load = ({ locals, url }) => {
    if (locals.user) redirect(303, '/projects');
    const redirectTo = url.searchParams.get('redirect') ?? '/projects';
    return { redirectTo };
};
```

The action reads it from the same `formData` parse already used for email/password, then validates it:

```ts
default: async ({ fetch, request, cookies }) => {
    const data = await request.formData();
    const email = data.get('email') as string;
    const password = data.get('password') as string;
    const redirectTo = (data.get('redirect') as string) ?? '/projects';

    // ... existing auth logic ...

    const safe = redirectTo.startsWith('/') ? redirectTo : '/projects';
    redirect(303, safe);
}
```

The `startsWith('/')` check prevents open redirect attacks (e.g. `?redirect=https://evil.com`).

### `login/+page.svelte` (updated)

Add one hidden input inside the existing form:

```svelte
<input type="hidden" name="redirect" value={data.redirectTo} />
```

## Security

The redirect destination is validated to be a relative path (`startsWith('/')`) before use. This ensures an attacker cannot craft a login URL that redirects to an external site after login.

## What Does Not Change

- URL paths — no routes are renamed or moved from a user perspective
- The tab visibility logic in `+layout.svelte` — tabs are still hidden in the UI for non-editors
- The Settings page guard — it already works correctly and uses a different permission tier
- All other page server files — players and import `+page.server.ts` files move but their content is unchanged
