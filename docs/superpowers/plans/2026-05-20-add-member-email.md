# Add Member by Email Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the broken "add member" flow so it correctly uses email end-to-end, and update the members table to show display name + email stacked.

**Architecture:** Backend adds `email` to the `ProjectMember` struct and SQL query. Frontend types are updated to match, the form field is renamed from `username` to `email`, and the table cell is updated to show the stacked layout. `app.d.ts` is corrected to match the actual `/auth/me` response shape.

**Tech Stack:** Rust/sqlx (backend), SvelteKit + TypeScript (frontend), shadcn-svelte Table component.

---

## File Map

| File | Change |
|---|---|
| `backend/crates/common/src/models/mod.rs` | Add `email: String` to `ProjectMember` struct |
| `backend/crates/api/src/routes/members.rs` | Update `list_members` SQL; update test assertion |
| `backend/.sqlx/` | Refreshed by `prepare-sqlx.sh` |
| `web/src/lib/types.ts` | Add `email: string` to `ProjectMember` interface |
| `web/src/app.d.ts` | Fix session user type to match actual `/auth/me` response |
| `web/src/routes/projects/[id]/settings/+page.server.ts` | Rename `username` → `email` in `addMember` action |
| `web/src/routes/projects/[id]/settings/+page.svelte` | Update table header/cell, form label/input |

---

## Task 1: Backend — add email to ProjectMember

**Files:**
- Modify: `backend/crates/api/src/routes/members.rs`
- Modify: `backend/crates/common/src/models/mod.rs`

- [ ] **Step 1: Update the test to assert on the `email` field**

In `backend/crates/api/src/routes/members.rs`, find the `test_add_member_and_list` test. Replace the final assertion block:

```rust
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let members: Value = serde_json::from_slice(&bytes).unwrap();
        // Owner is identified by ranking_projects.owner_id and is not in project_members,
        // so the list only contains the added editor member.
        assert_eq!(members.as_array().unwrap().len(), 1);
```

with:

```rust
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let members: Value = serde_json::from_slice(&bytes).unwrap();
        // Owner is identified by ranking_projects.owner_id and is not in project_members,
        // so the list only contains the added editor member.
        assert_eq!(members.as_array().unwrap().len(), 1);
        assert_eq!(members[0]["email"], "mem_user@test.com");
        assert_eq!(members[0]["display_name"], "mem_user");
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd backend
cargo test -p api -- test_add_member_and_list
```

Expected: FAIL — `members[0]["email"]` is `null` (field not in response yet).

- [ ] **Step 3: Add `email` to the `ProjectMember` struct**

In `backend/crates/common/src/models/mod.rs`, update `ProjectMember`:

```rust
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ProjectMember {
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
}
```

- [ ] **Step 4: Update `list_members` SQL to select `u.email`**

In `backend/crates/api/src/routes/members.rs`, update the `list_members` query:

```rust
    let members = sqlx::query_as!(
        ProjectMember,
        r#"SELECT pm.project_id, pm.user_id, u.display_name, u.email,
                  pm.role as "role: MemberRole", pm.joined_at
           FROM project_members pm
           JOIN users u ON u.id = pm.user_id
           WHERE pm.project_id = $1
           ORDER BY pm.joined_at ASC"#,
        project_id,
    )
    .fetch_all(&state.db)
    .await?;
```

- [ ] **Step 5: Run the backend test suite to verify tests pass**

```bash
cd backend
bash test.sh
```

Expected: all tests pass. The `test_add_member_and_list` test now passes with the `email` assertion.

- [ ] **Step 6: Refresh the sqlx offline query cache**

```bash
cd backend
bash prepare-sqlx.sh
```

Expected output ends with: `Done. Offline cache updated in .sqlx/`

- [ ] **Step 7: Commit**

```bash
git add backend/crates/common/src/models/mod.rs \
        backend/crates/api/src/routes/members.rs \
        backend/.sqlx/
git commit -m "feat: include email in ProjectMember list response"
```

---

## Task 2: Frontend — update types and app.d.ts

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/src/app.d.ts`

- [ ] **Step 1: Add `email` to the `ProjectMember` interface**

In `web/src/lib/types.ts`, update `ProjectMember`:

```ts
export interface ProjectMember {
	project_id: string;
	user_id: string;
	display_name: string;
	email: string;
	role: 'editor' | 'viewer';
	joined_at: string;
}
```

- [ ] **Step 2: Fix the session user type in `app.d.ts`**

Replace the entire content of `web/src/app.d.ts`:

```ts
// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		interface Locals {
			user: { id: string; email: string; display_name: string; created_at: string } | null;
		}
		// interface PageData {}
		// interface PageState {}
		// interface Platform {}
	}
}

export {};
```

- [ ] **Step 3: Run TypeScript check**

```bash
cd web
npm run check
```

Expected: no new type errors introduced by these changes.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/types.ts web/src/app.d.ts
git commit -m "fix: update ProjectMember type and app.d.ts to include email"
```

---

## Task 3: Frontend — fix the addMember server action

**Files:**
- Modify: `web/src/routes/projects/[id]/settings/+page.server.ts`

- [ ] **Step 1: Update the `addMember` action**

In `web/src/routes/projects/[id]/settings/+page.server.ts`, replace the `addMember` action:

```ts
	addMember: async ({ fetch, params, cookies, request }) => {
		const data = await request.formData();
		const email = ((data.get('email') as string) ?? '').trim();
		const role = data.get('role') as string;
		if (!email) return fail(400, { memberError: 'Email is required' });
		if (!['editor', 'viewer'].includes(role)) return fail(400, { memberError: 'Invalid role' });
		const api = makeApi(fetch, env.INTERNAL_API_URL, cookies.get('session_id'));
		const res = await api.post(`/projects/${params.id}/members`, { email, role });
		if (!res.ok) {
			const body = await res.json().catch(() => ({ message: 'Failed to add member' }));
			return fail(res.status, { memberError: body.message });
		}
		return {};
	},
```

- [ ] **Step 2: Run TypeScript check**

```bash
cd web
npm run check
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add web/src/routes/projects/[id]/settings/+page.server.ts
git commit -m "fix: send email (not username) when adding project member"
```

---

## Task 4: Frontend — update settings page UI

**Files:**
- Modify: `web/src/routes/projects/[id]/settings/+page.svelte`

- [ ] **Step 1: Update the members table header**

Find this line in `+page.svelte`:

```svelte
					<Table.Head>Username</Table.Head>
```

Replace with:

```svelte
					<Table.Head>Member</Table.Head>
```

- [ ] **Step 2: Update the members table cell to show stacked display name + email**

Find this line:

```svelte
						<Table.Cell>{member.username}</Table.Cell>
```

Replace with:

```svelte
						<Table.Cell>
							<div class="font-medium">{member.display_name}</div>
							<div class="text-xs text-muted-foreground">{member.email}</div>
						</Table.Cell>
```

- [ ] **Step 3: Update the add-member form label, input name, placeholder, and type**

Find this block:

```svelte
			<form method="POST" action="?/addMember" use:enhance class="flex gap-2 items-end">
				<div class="flex-1 space-y-1">
					<Label for="member-username">Add by username</Label>
					<Input id="member-username" name="username" placeholder="username" />
				</div>
```

Replace with:

```svelte
			<form method="POST" action="?/addMember" use:enhance class="flex gap-2 items-end">
				<div class="flex-1 space-y-1">
					<Label for="member-email">Add by email</Label>
					<Input id="member-email" name="email" type="email" placeholder="player@example.com" />
				</div>
```

- [ ] **Step 4: Run TypeScript check**

```bash
cd web
npm run check
```

Expected: no errors. In particular, `member.username` is gone and `member.display_name` + `member.email` are both valid on the updated `ProjectMember` interface.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/[id]/settings/+page.svelte
git commit -m "fix: update members table and add-member form to use email"
```
