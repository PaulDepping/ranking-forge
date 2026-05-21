# API Client Separation Design

**Date:** 2026-05-21
**Status:** Approved

## Problem

The frontend has two different contexts for calling the backend API — the SvelteKit server (SSR, form actions) and the browser client — but both share a single `makeApi` factory with an overloaded interface. This has caused real bugs:

- Every `+page.server.ts` must manually pass `cookies.get("session_id")` to `makeApi`. Forgetting it causes silent 401s (already happened: commit `b2cef98`).
- The login action parses `Set-Cookie` via a regex to re-extract the session ID and re-set it as a browser cookie — fragile.
- The logout action deletes the cookie without a `domain` option, so the delete doesn't match the stored cookie in production.
- Several `.svelte` files reference `PUBLIC_API_URL` directly instead of going through the API helper.
- In production (`rankingforge.com` frontend, `api.rankingforge.com` API), client-side calls silently 401 unless `COOKIE_DOMAIN=rankingforge.com` is set — this requirement is undocumented.

## Goals

- Eliminate manual `cookies.get("session_id")` forwarding in every server route.
- Enforce server/client API separation at the build level via SvelteKit's `$lib/server/` boundary.
- Fix cookie set/delete symmetry.
- Remove raw `fetch` and direct `PUBLIC_API_URL` references from `.svelte` files.
- Document `COOKIE_DOMAIN` as a required production env var.

## Design

### 1. Server API helper — `src/lib/server/api.ts`

New file, server-only (enforced by `$lib/server/` import protection).

```typescript
import { env } from "$env/dynamic/private";

export function makeServerApi(fetchFn: typeof fetch, sessionId: string | undefined) {
  async function req(method: string, path: string, body?: unknown): Promise<Response> {
    const headers: Record<string, string> = {};
    if (sessionId) headers["Cookie"] = `session_id=${sessionId}`;
    if (body !== undefined) headers["Content-Type"] = "application/json";
    return fetchFn(env.INTERNAL_API_URL + path, {
      method,
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  }

  return {
    get: (path: string) => req("GET", path),
    post: (path: string, body?: unknown) => req("POST", path, body),
    patch: (path: string, body: unknown) => req("PATCH", path, body),
    put: (path: string, body: unknown) => req("PUT", path, body),
    delete: (path: string) => req("DELETE", path),
  };
}

export type ServerApi = ReturnType<typeof makeServerApi>;
```

No `credentials: "include"` — irrelevant for server-to-server calls. No `baseUrl` parameter — always uses `INTERNAL_API_URL`. No `sessionId` at callsites — read once in hooks and stored in `locals`.

### 2. Client API helper — `src/lib/api.ts`

Remove the `sessionId` and `baseUrl` parameters. `PUBLIC_API_URL` is now read internally. `credentials: "include"` stays — the browser automatically sends the `session_id` cookie.

```typescript
import { env } from "$env/dynamic/public";

export function makeApi(fetchFn: typeof fetch) {
  async function req(method: string, path: string, body?: unknown): Promise<Response> {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["Content-Type"] = "application/json";
    return fetchFn(env.PUBLIC_API_URL + path, {
      method,
      credentials: "include",
      headers,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
  }

  return {
    get: (path: string) => req("GET", path),
    post: (path: string, body?: unknown) => req("POST", path, body),
    patch: (path: string, body: unknown) => req("PATCH", path, body),
    put: (path: string, body: unknown) => req("PUT", path, body),
    delete: (path: string) => req("DELETE", path),
    putRanking: (projectId: string, playerIds: string[]) =>
      req("PUT", `/projects/${projectId}/ranking`, { player_ids: playerIds }),
  };
}
```

### 3. `hooks.server.ts` — build `locals.api` once per request

```typescript
import { makeServerApi } from "$lib/server/api";

export const handle: Handle = async ({ event, resolve }) => {
  const sessionId = event.cookies.get("session_id");
  event.locals.api = makeServerApi(event.fetch, sessionId);

  const res = await event.locals.api.get("/auth/me");
  if (res.ok) {
    event.locals.user = await res.json();
  } else {
    event.locals.user = null;
    // existing redirect logic unchanged
  }

  return resolve(event);
};
```

### 4. `app.d.ts` — add `api` to `App.Locals`

```typescript
import type { ServerApi } from "$lib/server/api";

declare global {
  namespace App {
    interface Locals {
      user: { ... } | null;
      api: ServerApi;
    }
  }
}
```

### 5. All `+page.server.ts` files

Replace every occurrence of:
```typescript
const api = makeApi(fetch, env.INTERNAL_API_URL, cookies.get("session_id"));
```
with:
```typescript
const { api } = locals;
```

Remove `import { makeApi }` and the `cookies.get("session_id")` pattern everywhere. Remove `cookies` from the function signature of any load function or action that used it *only* for API forwarding (login/logout still need `cookies` for set/delete). Remove `import { env }` from `$env/dynamic/private` in files that no longer use any private env vars.

Files affected:
- `routes/account/+page.server.ts`
- `routes/projects/+page.server.ts`
- `routes/projects/new/+page.server.ts`
- `routes/projects/[id]/+layout.server.ts`
- `routes/projects/[id]/(editor)/import/+page.server.ts`
- `routes/projects/[id]/(editor)/players/+page.server.ts`
- `routes/projects/[id]/(editor)/players/[player_id]/+page.server.ts`
- `routes/projects/[id]/h2h/+page.server.ts`
- `routes/projects/[id]/ranking/+page.server.ts`
- `routes/projects/[id]/settings/+page.server.ts`
- `routes/projects/[id]/stats/+page.server.ts`
- `routes/projects/[id]/tournaments/+page.server.ts`
- `routes/invite/[token]/+page.server.ts`

### 6. Cookie fixes

**Login (`routes/login/+page.server.ts`) and Register (`routes/register/+page.server.ts`):**

Both routes create a session and must set the `session_id` cookie on the browser. The same fix applies to both.


The Rust API login handler currently returns only the user object in the body. Add `session_id` to the response body so SvelteKit can read it without parsing `Set-Cookie`:

```rust
// In auth.rs login handler, change response to include session_id:
Json(serde_json::json!({ "session_id": session_id, "user": user }))
```

SvelteKit login action reads `body.session_id` directly instead of parsing `Set-Cookie`.

**Logout (`routes/logout/+page.server.ts`):**

```typescript
cookies.delete("session_id", {
  path: "/",
  ...(env.COOKIE_DOMAIN ? { domain: env.COOKIE_DOMAIN } : {}),
});
```

### 7. Client-side `.svelte` cleanup

Replace all `makeApi(fetch, env.PUBLIC_API_URL)` with `makeApi(fetch)`. Remove all `import { env } from "$env/dynamic/public"` that were only there for `PUBLIC_API_URL`.

Fix `routes/projects/new/+page.svelte`: replace raw `fetch(env.PUBLIC_API_URL + '/games?...', { credentials: 'include' })` with `makeApi(fetch).get('/games?...')`.

Files affected:
- `routes/projects/new/+page.svelte`
- `routes/projects/[id]/(editor)/import/+page.svelte`
- `routes/projects/[id]/h2h/+page.svelte`
- `routes/projects/[id]/ranking/+page.svelte`
- `routes/projects/[id]/tournaments/+page.svelte`
- `src/lib/components/HandleTab.svelte`
- `src/lib/components/NameTab.svelte`
- `src/lib/components/TournamentTab.svelte`

### 8. Environment variable documentation

Add `COOKIE_DOMAIN` to the env var table in `CLAUDE.md` and create `web/.env.example`:

```
PUBLIC_API_URL=http://localhost:8080
INTERNAL_API_URL=http://localhost:8080
# Required in production: set to the root domain so the session cookie
# is sent to the API subdomain (e.g. rankingforge.com)
# COOKIE_DOMAIN=
```

## Production correctness

With `COOKIE_DOMAIN=rankingforge.com` set:
- SvelteKit sets `session_id` with `domain=rankingforge.com`
- Browser JS on `rankingforge.com` sends the cookie to `api.rankingforge.com` via `credentials: "include"`
- `SameSite=Strict` is not an issue: both share eTLD+1 `rankingforge.com`, so they are same-site
- The API already has `allow_credentials(true)` and a specific `allow_origin` in its CORS config

## Testing

- `src/lib/api.test.ts` needs updating: remove the `sessionId` and `baseUrl` parameters from test calls to `makeApi`. Mock `$env/dynamic/public` via `src/__mocks__/env.ts` (already exists).
- All existing backend and e2e tests are unaffected (they test the API directly, not the SvelteKit layer).
- Manual smoke test: login → navigate to import page → trigger import → verify polling works → logout → verify session is cleared.
