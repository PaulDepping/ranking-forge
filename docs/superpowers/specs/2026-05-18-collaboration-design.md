# Collaboration & Publishing Design

**Date:** 2026-05-18
**Status:** Approved

## Overview

Add multi-user collaboration to ranking projects via a role-based membership system, invite links, and a public publishing feature. A project can be shared with specific users (by username) or via invite links. Published projects are readable by unauthenticated visitors at the same URL used by members.

---

## Data Model

### Schema changes

`ranking_projects.user_id` is **removed**. Project ownership is determined solely by `project_members`.

#### New enum

```sql
CREATE TYPE project_member_role AS ENUM ('owner', 'editor', 'viewer');
```

#### `project_members`

```sql
CREATE TABLE project_members (
    project_id  UUID                NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    user_id     UUID                NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL,
    joined_at   TIMESTAMPTZ         NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, user_id)
);
```

Exactly one row per project has `role = 'owner'`. Enforced at the application layer. When a project is created, the creator is inserted as owner.

#### `project_invite_links`

```sql
CREATE TABLE project_invite_links (
    id          UUID                PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID                NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL CHECK (role IN ('editor', 'viewer')),
    created_by  UUID                NOT NULL REFERENCES users(id),
    expires_at  TIMESTAMPTZ,        -- NULL = never expires
    revoked_at  TIMESTAMPTZ,        -- NULL = active
    created_at  TIMESTAMPTZ         NOT NULL DEFAULT NOW()
);
```

Invite links can only grant `editor` or `viewer` — never `owner`. Enforced by DB constraint and handler validation.

#### `ranking_projects` additions

```sql
published  BOOLEAN  NOT NULL DEFAULT FALSE
```

`user_id` column is dropped.

---

## Access Control

### Role hierarchy

`owner > editor > viewer`

A check for "at least editor" passes for owner too.

### Access resolution

Every project read endpoint uses `OptionalAuthUser` (returns `Option<User>` instead of 401 on missing session). Access is resolved as follows:

1. **Logged in + member** → grant access at their role level
2. **Logged in + not a member + published** → grant viewer-level access
3. **Not logged in + published** → grant viewer-level access
4. **Otherwise** → 404 (same response whether the project doesn't exist or the user isn't allowed — avoids leaking project existence)

Write endpoints (players, import, events toggle, member management) keep the hard `AuthUser` + role check.

### `require_project_read_access(db, project_id, Option<user_id>) -> Result<Project>`

Core helper used by all read handlers. Encodes the three-case logic above. Returns 404 on all failure paths.

### `require_project_access(db, project_id, user_id, min_role) -> Result<Project>`

Used by write handlers. Queries `project_members WHERE project_id = $1 AND user_id = $2`. Returns 404 if no membership row exists, 403 if role is below `min_role`.

### Per-endpoint access requirements

| Action | Requirement |
|---|---|
| View stats, H2H, tournaments, ranking | Viewer (or published) |
| Manage players, run import, toggle events | Editor (auth required) |
| Rename project | Owner |
| Publish / unpublish | Owner |
| Manage members (add, remove, change role) | Owner |
| Manage invite links | Owner |
| Delete project | Owner |
| Transfer ownership | Owner |

---

## Member Management

### Add by username

`POST /projects/:id/members` (owner only)

```json
{ "username": "someuser", "role": "editor" }
```

Looks up the user by username, then inserts or updates their `project_members` row. An owner cannot be added or demoted via this endpoint — ownership is transferred separately.

### Remove member

`DELETE /projects/:id/members/:user_id` (owner only)

An owner cannot remove themselves.

### Change role

`PATCH /projects/:id/members/:user_id` (owner only)

```json
{ "role": "viewer" }
```

Cannot target the owner row.

### Transfer ownership

`POST /projects/:id/transfer-ownership` (owner only)

```json
{ "user_id": "<uuid of target member>" }
```

In a single transaction:
1. Update the target member's role to `owner`
2. Update the current owner's role to `editor`

The previous owner remains a member (as editor). Target must already be a member.

### List members

`GET /projects/:id/members` (owner only)

Returns all members with their role, username, and `joined_at`.

---

## Invite Links

### Create

`POST /projects/:id/invite-links` (owner only)

```json
{ "role": "editor", "expires_at": "2026-06-01T00:00:00Z" }
```

`expires_at` is optional. Returns the full invite link record including the UUID token. The frontend constructs the accept URL as `/invite/:token`.

### Accept

`POST /invite/:token/accept` (requires auth — the accepting user must be logged in)

1. Look up the invite link; return 404 if not found, 410 if revoked or expired
2. If the user is already the project owner, return 409 (no-op with explanation)
3. If the user is already a member, update their role to the link's role
4. Otherwise insert a new `project_members` row
5. Return `{ project_id }` so the frontend can redirect to the project

### Revoke

`DELETE /projects/:id/invite-links/:link_id` (owner only)

Sets `revoked_at = NOW()`. Links are never hard-deleted.

### List

`GET /projects/:id/invite-links` (owner only)

Returns all non-revoked links with role, expiry, created-by, and created-at.

---

## Publishing

### Toggle

`PATCH /projects/:id` gains an optional `published: bool` field alongside the existing `name` field. Owner only.

### Public access

Published projects are accessible at the same `/projects/:id` URL used by members. No separate route tree. The `OptionalAuthUser` extractor on read endpoints handles unauthenticated visitors transparently.

### "Private project" page

When an unauthenticated user receives a 404 from the project endpoint, the SvelteKit `/projects/[id]` page shows:

> "This project is private. [Create an account] to request access."

The "Create an account" text links to `/register`. If the user is logged in and still receives 404, a plain "Project not found" message is shown instead.

---

## API Changes Summary

### New endpoints

| Method | Path | Auth | Description |
|---|---|---|---|
| `GET` | `/projects/:id/members` | Owner | List members |
| `POST` | `/projects/:id/members` | Owner | Add member by username |
| `PATCH` | `/projects/:id/members/:uid` | Owner | Change member role |
| `DELETE` | `/projects/:id/members/:uid` | Owner | Remove member |
| `POST` | `/projects/:id/transfer-ownership` | Owner | Transfer ownership |
| `GET` | `/projects/:id/invite-links` | Owner | List invite links |
| `POST` | `/projects/:id/invite-links` | Owner | Create invite link |
| `DELETE` | `/projects/:id/invite-links/:lid` | Owner | Revoke invite link |
| `POST` | `/invite/:token/accept` | AuthUser | Accept invite link |

### Modified endpoints

| Method | Path | Change |
|---|---|---|
| `GET` | `/projects` | Return all projects where the authenticated user has any `project_members` row (any role) |
| `GET` | `/projects/:id` | Switch to `OptionalAuthUser`, use `require_project_read_access` |
| `PATCH` | `/projects/:id` | Add optional `published` field; owner-only for this field |
| `GET` | `/projects/:id/stats` | Switch to `OptionalAuthUser` |
| `GET` | `/projects/:id/head-to-head` | Switch to `OptionalAuthUser` |
| `GET` | `/projects/:id/head-to-head/:a/:b/sets` | Switch to `OptionalAuthUser` |
| `GET` | `/projects/:id/tournaments` | Switch to `OptionalAuthUser` |

### Unchanged

All write endpoints (`players`, `import`, `events/:eid`) keep `AuthUser` + editor-level check.

---

## Frontend Changes

- `hooks.server.ts`: exempt `/projects/[id]` (and sub-pages) from the unauthenticated redirect so public visitors can reach project pages
- `/projects/[id]`: handle 404 with auth-aware messaging (private project vs. not found)
- Project settings page: add Members section (list, add by username, remove, change role) and Invite Links section (create, list, revoke, copy link)
- New `/invite/[token]` page: shows project name and role, prompts login if needed, calls accept endpoint
- Project header/nav: show publish status and toggle for owners

---

## Testing

- Backend: `#[sqlx::test]` per the existing pattern — no mocks, real isolated schema per test
- Cover: membership CRUD, role enforcement (403 vs 404), ownership transfer atomicity, invite link lifecycle (create, accept, revoke, expiry), published flag access (authenticated member, unauthenticated public, private 404)
- Frontend E2E: extend mock API to cover member and invite link endpoints; add Playwright tests for the invite accept flow and the private-project 404 page
