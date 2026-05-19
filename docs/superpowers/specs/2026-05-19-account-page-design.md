# Account Page Design

**Date:** 2026-05-19

## Overview

Add a `/account` settings page that lets users change their password, update their profile, and delete their account. Alongside this, the user model is redesigned: `username` becomes a non-unique `display_name`, and `email` becomes the unique login identifier.

## Scope

- Email (stored only — no verification, no password reset)
- Change display name
- Change email
- Change password
- Delete account (cascades to owned projects)

Out of scope: email verification, forgot-password flows, avatar, start.gg account linking, session management.

---

## Data Model

### `users` table (migration rewrite — no prod DB)

| Column | Type | Constraints | Notes |
|---|---|---|---|
| `id` | UUID | PK | |
| `email` | TEXT | NOT NULL UNIQUE | New login identifier |
| `display_name` | TEXT | NOT NULL | Renamed from `username`; uniqueness removed |
| `password_hash` | TEXT | NOT NULL | |
| `created_at` | TIMESTAMPTZ | NOT NULL DEFAULT NOW() | |

Validation: `display_name` 1–50 chars; `email` valid format; `password` 8–128 chars.

### `ranking_projects` table

Add `owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE`.

Ownership moves from `project_members.role = 'owner'` to a direct column. The DB enforces exactly one owner per project and auto-deletes projects when their owner's account is deleted. No application-level pre-delete work needed.

### `project_members` table

Drop the `owner` value from the `project_member_role` enum. Remaining roles: `editor`, `viewer`. The owner is no longer a member row — they are identified by `ranking_projects.owner_id`.

### `project_invite_links` table

Change `created_by UUID NOT NULL REFERENCES users(id)` to add `ON DELETE CASCADE` so invite links are deleted when their creator's account is deleted.

### Account deletion cascade chain

`DELETE FROM users WHERE id = $user_id` triggers:
- `ranking_projects` (via `owner_id ON DELETE CASCADE`) → cascades to players, events, sets, jobs, etc.
- `sessions` (already `ON DELETE CASCADE`)
- `project_members` (already `ON DELETE CASCADE`)
- `project_invite_links` (new `ON DELETE CASCADE`)

---

## API

### Updated endpoints

**`POST /auth/register`**
```json
{ "email": "...", "display_name": "...", "password": "..." }
```
Returns `201` with `UserResponse`. Validates display_name length, email format, password length. Returns 422 if email already taken.

**`POST /auth/login`**
```json
{ "email": "...", "password": "..." }
```
Looks up user by email instead of username. Constant-time dummy hash path unchanged.

**`GET /auth/me`**
```json
{ "id": "...", "email": "...", "display_name": "...", "created_at": "..." }
```

### New `/account` endpoints

All require an authenticated session (`AuthUser` extractor). Return `204 No Content` on success.

**`PATCH /account/profile`**
```json
{ "display_name": "...", "email": "..." }
```
Both fields optional; at least one required. Email uniqueness checked; returns 422 if taken.

**`PATCH /account/password`**
```json
{ "current_password": "...", "new_password": "..." }
```
Verifies `current_password` via `verify_password` before hashing and storing `new_password`. Returns 401 if current password is wrong. New password validated to 8–128 chars.

**`DELETE /account`**
No body. Deletes the authenticated user. The DB cascade handles everything (projects, sessions, members, invite links). Response sets the session cookie to expired (same `clear_cookie()` helper used by logout).

### Router changes

Add a new `routes/account.rs` module with its own `Router` mounted at `/account` in `routes/mod.rs`. No rate-limit layer (actions already require an authenticated session).

`verify_password` and `hash_password` helpers in `auth.rs` are extracted to a shared location (same file, just used by the new module via `pub(super)` or moved to a `password` submodule).

### Projects routes

All places that currently check `project_members.role = 'owner'` for ownership checks must be updated to query `ranking_projects.owner_id` instead.

### openapi.yaml

- Update `UserResponse` schema: replace `username` with `display_name`, add `email`
- Update register/login request schemas
- Add `PATCH /account/profile`, `PATCH /account/password`, `DELETE /account`

---

## Frontend

### Updated pages

**`/login`** — change `<Input name="username">` to `<Input name="email" type="email" autocomplete="email">`, update label.

**`/register`** — add `email` field (first, type email), rename `username` field to `display_name` with label "Display name". Remove any uniqueness hint copy.

**`+layout.svelte`** — `data.user.username` → `data.user.display_name`. Wrap in `<a href="/account">` so it links to the account page.

### New page: `/account`

**`+page.server.ts`** — load returns current user from `locals.user`. Three form actions:

| Action | API call | On success |
|---|---|---|
| `updateProfile` | `PATCH /account/profile` | Invalidate, show success state |
| `updatePassword` | `PATCH /account/password` | Invalidate, clear fields |
| `deleteAccount` | `DELETE /account` | `redirect(303, '/login')` |

Page is auth-protected: if `locals.user` is null, redirect to `/login`.

**`+page.svelte`** — three shadcn `Card` components in a single scrollable `max-w-2xl` column:

**Card 1 — Profile**
- `CardHeader`: title "Profile", description "Update your display name and email address."
- `CardContent`: Display name input, Email input
- `CardFooter`: "Save changes" button (right-aligned)

**Card 2 — Password**
- `CardHeader`: title "Password", description "Use a strong, unique password."
- `CardContent`: Current password, New password, Confirm new password (all `type="password"`)
- `CardFooter`: "Change password" button (right-aligned)

**Card 3 — Delete account**
- `CardHeader`: title "Delete account" (destructive colour), description "Permanently deletes your account and all projects you own. This cannot be undone."
- `CardFooter`: "Delete account" button (destructive variant) → opens `AlertDialog` for confirmation
- `AlertDialog` confirm button submits the `deleteAccount` form action

---

## Validation summary

| Field | Rule |
|---|---|
| `display_name` | 1–50 chars |
| `email` | Valid email format, unique |
| `password` / `new_password` | 8–128 chars |
| `current_password` | Must match stored hash |
| Profile update | At least one of `display_name` / `email` must be present |

---

## Testing

- **Backend unit tests** (`common`): no changes needed
- **Backend integration tests** (`api`): new tests for `PATCH /account/profile`, `PATCH /account/password`, `DELETE /account`. Existing auth tests updated for email-based login. Project ownership tests updated to use `owner_id`.
- **Frontend unit tests**: update login/register mocks for email field; add account page action tests
- **e2e tests**: mock API updated for new `UserResponse` shape and new account endpoints
- Run `bash backend/prepare-sqlx.sh` after all query changes
