# Design: Add Member by Email + Members Table Email Display

**Date:** 2026-05-20

## Problem

Login was updated to use email instead of display name, but the project settings page still labels its "add member" form as "Add by username" and sends a `username` field — while the backend `AddMemberRequest` already expects `email`. This makes the feature currently broken. Additionally, the members table reads `member.username` but `ProjectMember` only returns `display_name`, so the table renders blank names.

## Goal

Fix the broken add-member form end-to-end, and show both display name and email in the members table (stacked layout: bold display name, muted email below).

## Backend Changes

### `backend/crates/common/src/models/mod.rs`

Add `email: String` to `ProjectMember`:

```rust
pub struct ProjectMember {
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,          // new
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
}
```

### `backend/crates/api/src/routes/members.rs`

Update `list_members` SQL to select `u.email`:

```sql
SELECT pm.project_id, pm.user_id, u.display_name, u.email,
       pm.role as "role: MemberRole", pm.joined_at
FROM project_members pm
JOIN users u ON u.id = pm.user_id
WHERE pm.project_id = $1
ORDER BY pm.joined_at ASC
```

No change to `AddMemberRequest` — it already has `email: String`.

After changes: run `bash backend/prepare-sqlx.sh` to refresh `.sqlx` offline cache.

## Frontend Changes

### `web/src/lib/types.ts`

Add `email: string` to the `ProjectMember` interface.

### `web/src/app.d.ts`

Replace the stale `{ id: string; username: string }` shape with the actual `/auth/me` response type:

```ts
user: { id: string; email: string; display_name: string; created_at: string } | null;
```

### `web/src/routes/projects/[id]/settings/+page.server.ts`

In the `addMember` action:
- Read `email` from form data (was `username`)
- Send `{ email, role }` to the API (was `{ username, role }`)
- Error message: `'Email is required'` (was `'Username is required'`)

### `web/src/routes/projects/[id]/settings/+page.svelte`

**Members table column header:** `Username` → `Member`

**Members table cell:** Replace `member.username` with stacked layout:
```html
<div class="font-medium">{member.display_name}</div>
<div class="text-xs text-muted-foreground">{member.email}</div>
```

**Add member form:**
- Label: `Add by username` → `Add by email`
- Input: `name="username"` → `name="email"`, `placeholder="username"` → `placeholder="email@example.com"`, add `type="email"`

## Testing

Existing `test_invite_link_lifecycle` and `test_list_members` tests in `members.rs` cover the backend. The SQL change is validated by `prepare-sqlx.sh` at build time. No new test cases required — the change corrects a mismatch, not new behaviour.

## Out of Scope

- Adding a distinct `username` field to the `users` table
- Exposing email in any other API response
- Changes to invite link flow (invite links work by token, not by looking up users)
