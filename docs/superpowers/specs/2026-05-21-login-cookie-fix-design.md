# Login/Register Cookie Fix

**Date:** 2026-05-21  
**Status:** Approved

## Problem

The login and register flows intermittently fail — specifically, when a `session_id` cookie is already present in the browser, clicking Sign In does nothing. The root cause is that two `Set-Cookie: session_id=...` headers are sent in the same response:

1. Axum's `login`/`register` handlers return a `CookieJar` with the new session cookie. SvelteKit's `event.fetch` intercepts this `Set-Cookie` header from the API response and tracks it internally.
2. The SvelteKit form action then also calls `cookies.set("session_id", body.session_id, {...})` explicitly.

When SvelteKit detects that the same cookie name is being set twice in one request cycle, it silently drops the redirect, causing the "button does nothing" symptom.

## Solution

Remove `CookieJar` from the *response* of the three Axum auth handlers. SvelteKit becomes the sole manager of the browser-facing `session_id` cookie. The backend remains a pure API.

This is correct by design: all auth in this app goes through SvelteKit form actions, not direct browser-to-Axum requests. SvelteKit's `cookies.set()` already handles `COOKIE_DOMAIN` (for multi-subdomain production) and the correct `Secure` flag per environment.

## Changes

### `backend/crates/api/src/routes/auth.rs`

**`login` handler**
- Remove `jar: CookieJar` parameter
- Change return from `Ok((jar, Json(SessionResponse { ... })))` to `Ok(Json(SessionResponse { ... }))`

**`register` handler**
- Remove `jar: CookieJar` parameter
- Change return from `Ok((StatusCode::CREATED, jar, Json(SessionResponse { ... })))` to `Ok((StatusCode::CREATED, Json(SessionResponse { ... })))`

**`logout` handler**
- Remove `jar: CookieJar` parameter
- Remove `jar.add(clear_cookie())` from the response
- Return `Ok(StatusCode::NO_CONTENT)` directly
- The SvelteKit logout action already calls `cookies.delete("session_id", {...})`, so no coverage is lost

**Dead code removal**
- Delete `session_cookie()` helper function
- Delete `clear_cookie()` helper function
- Remove unused `CookieJar` imports from handler signatures (keep the import for `AuthUser`/`OptionalAuthUser` extractors)

### `web/` — no changes

The `cookies.set()` / `cookies.delete()` calls in `+page.server.ts` are already correct.

## No-change areas

- `AuthUser` and `OptionalAuthUser` extractors still read `session_id` from request cookies via `CookieJar` — this is unaffected.
- `makeServerApi` still passes `Cookie: session_id=<value>` as a request header — unaffected.
- `makeApi` (client-side) still sends cookies via `credentials: "include"` — unaffected.
- Mock API in `tests/mock-api.js` still sets `Set-Cookie` in login/register responses — these are test-only and do not conflict because the mock is called directly by Playwright (not via `event.fetch`). They can be cleaned up in a follow-up but do not need to change for this fix.

## Testing

Run `bash test.sh` to verify. The existing Playwright auth tests cover the login redirect flow.
