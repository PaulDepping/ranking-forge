# Project Settings Page + Select All Attendees Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Settings tab to the project layout (rename + delete), and add a select-all checkbox to the "From tournament" entrant list.

**Architecture:** Feature 1 adds a `PATCH /projects/{id}` backend endpoint, a new `settings/` SvelteKit route, and moves delete off the projects list. Feature 2 is a pure frontend addition to `TournamentTab.svelte`. Both features are independent and can be implemented in either order.

**Tech Stack:** Rust/Axum (backend), SvelteKit 5 with runes, shadcn-svelte, sqlx

---

## File Map

### Task 1 — Backend PATCH endpoint
- Modify: `backend/crates/api/src/routes/projects.rs` — add `RenameProjectRequest` struct + `rename_project` handler; wire `.patch(rename_project)` onto the `/{id}` route
- Modify: `backend/crates/e2e/tests/full_flow.rs` — add `test_rename_project` test function
- Modify: `backend/openapi.yaml` — add `patch:` block to `/projects/{project_id}`

### Task 2 — Frontend settings page
- Create: `web/src/routes/projects/[id]/settings/+page.server.ts` — `rename` and `delete` actions (no load needed; layout provides `data.project`)
- Create: `web/src/routes/projects/[id]/settings/+page.svelte` — rename form + danger zone
- Modify: `web/src/routes/projects/[id]/+layout.svelte` — add Settings tab
- Modify: `web/src/routes/projects/+page.svelte` — remove Delete button from cards
- Modify: `web/src/routes/projects/+page.server.ts` — remove `delete` action

### Task 3 — Select all attendees
- Modify: `web/src/lib/components/TournamentTab.svelte` — add `selectableFiltered`, `allSelected` deriveds, `toggleAll` function, and checkbox above the list

---

## Task 1: Backend — PATCH /projects/{id}

**Files:**
- Modify: `backend/crates/api/src/routes/projects.rs`
- Modify: `backend/crates/e2e/tests/full_flow.rs`
- Modify: `backend/openapi.yaml`

- [ ] **Step 1: Write the failing test**

Add a new test function in `backend/crates/e2e/tests/full_flow.rs`, after the existing `full_import_flow` function:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_rename_project(pool: PgPool) {
    let app = make_app(pool, "http://unused");
    let cookie = register(&app, "alice", "password123").await;

    // Create a project
    let resp = post_json(&app, "/projects", &cookie, json!({"name": "Original"})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = read_json(resp).await;
    let project_id = body["id"].as_str().unwrap().to_string();

    // Rename it
    let resp = patch_json(
        &app,
        &format!("/projects/{project_id}"),
        &cookie,
        json!({"name": "Renamed"}),
    ).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body["name"], "Renamed");

    // Confirm GET reflects new name
    let resp = get_req(&app, &format!("/projects/{project_id}"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body["name"], "Renamed");

    // Empty name is rejected
    let resp = patch_json(
        &app,
        &format!("/projects/{project_id}"),
        &cookie,
        json!({"name": "   "}),
    ).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Name over 100 chars is rejected
    let resp = patch_json(
        &app,
        &format!("/projects/{project_id}"),
        &cookie,
        json!({"name": "a".repeat(101)}),
    ).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd backend && bash test.sh -v 2>&1 | grep -A3 "test_rename_project"
```

Expected: FAIL — `PATCH /projects/{id}` returns 405 Method Not Allowed (route not yet wired).

- [ ] **Step 3: Add the handler and request type**

In `backend/crates/api/src/routes/projects.rs`, add after the `CreateProjectRequest` struct (around line 23):

```rust
#[derive(Deserialize)]
pub struct RenameProjectRequest {
    pub name: String,
}
```

Then add the handler after `get_project` (around line 127):

```rust
async fn rename_project(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<RenameProjectRequest>,
) -> Result<impl IntoResponse> {
    if body.name.trim().is_empty() {
        return Err(AppError::UnprocessableEntity(
            "name must not be empty".into(),
        ));
    }
    if body.name.trim().chars().count() > 100 {
        return Err(AppError::UnprocessableEntity(
            "name must be at most 100 characters".into(),
        ));
    }

    let project = sqlx::query_as!(
        Project,
        "UPDATE ranking_projects SET name = $1 WHERE id = $2 AND user_id = $3
         RETURNING id, user_id, name, game_id, game_name, created_at",
        body.name.trim(),
        project_id,
        user.id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(ProjectResponse::from(project)))
}
```

- [ ] **Step 4: Wire the route**

In the `router()` function in `projects.rs`, update the `/{id}` route from:

```rust
.route("/{id}", get(get_project).delete(delete_project))
```

to:

```rust
.route("/{id}", get(get_project).delete(delete_project).patch(rename_project))
```

- [ ] **Step 5: Update the sqlx offline cache**

```bash
cd backend && bash prepare-sqlx.sh
```

Expected: completes without errors. The `.sqlx/` directory will have a new entry for the UPDATE query.

- [ ] **Step 6: Run the test to confirm it passes**

```bash
cd backend && bash test.sh -v 2>&1 | grep -A3 "test_rename_project"
```

Expected: PASS.

- [ ] **Step 7: Update the OpenAPI spec**

In `backend/openapi.yaml`, find the `/projects/{project_id}:` path block (around line 473). Add a `patch:` block after the `delete:` block:

```yaml
    patch:
      summary: Rename a project
      tags: [Projects]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              required: [name]
              properties:
                name:
                  type: string
                  minLength: 1
                  maxLength: 100
      responses:
        '200':
          description: Updated
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Project'
        '401':
          $ref: '#/components/responses/Unauthorized'
        '404':
          $ref: '#/components/responses/NotFound'
        '422':
          $ref: '#/components/responses/UnprocessableEntity'
```

- [ ] **Step 8: Commit**

```bash
git add backend/crates/api/src/routes/projects.rs backend/crates/e2e/tests/full_flow.rs backend/.sqlx/ backend/openapi.yaml
git commit -m "feat(api): add PATCH /projects/{id} rename endpoint"
```

---

## Task 2: Frontend — Project Settings Page

**Files:**
- Create: `web/src/routes/projects/[id]/settings/+page.server.ts`
- Create: `web/src/routes/projects/[id]/settings/+page.svelte`
- Modify: `web/src/routes/projects/[id]/+layout.svelte`
- Modify: `web/src/routes/projects/+page.svelte`
- Modify: `web/src/routes/projects/+page.server.ts`

- [ ] **Step 1: Add the Settings tab to the project layout**

In `web/src/routes/projects/[id]/+layout.svelte`, update the `tabs` array from:

```ts
const tabs = [
    { label: 'Players', href: 'players' },
    { label: 'Import', href: 'import' },
    { label: 'Tournaments', href: 'tournaments' },
    { label: 'Stats', href: 'stats' },
    { label: 'H2H', href: 'h2h' }
];
```

to:

```ts
const tabs = [
    { label: 'Players', href: 'players' },
    { label: 'Import', href: 'import' },
    { label: 'Tournaments', href: 'tournaments' },
    { label: 'Stats', href: 'stats' },
    { label: 'H2H', href: 'h2h' },
    { label: 'Settings', href: 'settings' }
];
```

- [ ] **Step 2: Create the server actions file**

Create `web/src/routes/projects/[id]/settings/+page.server.ts`:

```typescript
import { fail, redirect } from '@sveltejs/kit';
import type { Actions } from './$types';
import { makeApi } from '$lib/api';
import { INTERNAL_API_URL } from '$env/static/private';

export const actions: Actions = {
    rename: async ({ fetch, params, cookies, request }) => {
        const data = await request.formData();
        const name = (data.get('name') as string ?? '').trim();
        if (!name) return fail(400, { error: 'Name is required' });
        if ([...name].length > 100) return fail(400, { error: 'Name must be at most 100 characters' });
        const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
        const res = await api.patch(`/projects/${params.id}`, { name });
        if (!res.ok) {
            const body = await res.json().catch(() => ({ message: 'Rename failed' }));
            return fail(res.status, { error: body.message });
        }
        const project = await res.json();
        return { project };
    },

    delete: async ({ fetch, params, cookies }) => {
        const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
        const res = await api.delete(`/projects/${params.id}`);
        if (!res.ok) {
            const body = await res.json().catch(() => ({ message: 'Delete failed' }));
            return fail(res.status, { error: body.message });
        }
        redirect(303, '/projects');
    }
};
```

- [ ] **Step 3: Create the settings page component**

Create `web/src/routes/projects/[id]/settings/+page.svelte`:

```svelte
<script lang="ts">
    import { enhance } from '$app/forms';
    import { invalidateAll } from '$app/navigation';
    import { Button } from '$lib/components/ui/button';
    import { Input } from '$lib/components/ui/input';
    import { Label } from '$lib/components/ui/label';

    let { data, form } = $props();

    let name = $state(data.project.name);
</script>

<div class="max-w-lg space-y-8">
    <div class="space-y-3">
        <h2 class="text-lg font-semibold">Project name</h2>
        <form
            method="POST"
            action="?/rename"
            class="flex gap-2"
            use:enhance={() => {
                return async ({ result, update }) => {
                    if (result.type === 'success' && result.data?.project) {
                        name = (result.data.project as { name: string }).name;
                        await invalidateAll();
                    } else {
                        await update();
                    }
                };
            }}
        >
            <Label for="project-name" class="sr-only">Project name</Label>
            <Input id="project-name" name="name" bind:value={name} class="flex-1" />
            <Button type="submit">Save</Button>
        </form>
        {#if form?.error}
            <p class="text-sm text-destructive">{form.error}</p>
        {/if}
    </div>

    <div class="border-t border-border"></div>

    <div class="space-y-3">
        <h2 class="text-lg font-semibold text-destructive">Danger zone</h2>
        <div class="flex items-center justify-between rounded-md border border-destructive/40 p-4">
            <div>
                <p class="font-medium">Delete this project</p>
                <p class="text-sm text-muted-foreground">
                    Permanently removes all players, tournaments, and stats.
                </p>
            </div>
            <form method="POST" action="?/delete" use:enhance>
                <Button
                    type="submit"
                    variant="destructive"
                    onclick={(e: MouseEvent) => {
                        if (!confirm('Delete this project? This cannot be undone.')) e.preventDefault();
                    }}
                >Delete project</Button>
            </form>
        </div>
    </div>
</div>
```

- [ ] **Step 4: Remove Delete from the projects list page**

In `web/src/routes/projects/+page.svelte`, remove the entire `<form>` block for delete inside each card's `<CardFooter>`. The footer should become read-only, showing only the date:

```svelte
<CardFooter>
    <span class="text-xs text-muted-foreground">
        {formatDate(project.created_at)}
    </span>
</CardFooter>
```

- [ ] **Step 5: Remove the delete action from the projects list server file**

In `web/src/routes/projects/+page.server.ts`, remove the entire `actions` export. The file becomes:

```typescript
import type { PageServerLoad } from './$types';
import { makeApi } from '$lib/api';
import type { Project } from '$lib/types';
import { INTERNAL_API_URL } from '$env/static/private';

export const load: PageServerLoad = async ({ fetch, cookies }) => {
    const api = makeApi(fetch, INTERNAL_API_URL, cookies.get('session_id'));
    const res = await api.get('/projects');
    if (!res.ok) return { projects: [] as Project[] };
    const projects: Project[] = await res.json();
    return { projects };
};
```

- [ ] **Step 6: Manually test the settings page**

Start the dev stack (`docker compose up -d` from repo root, then `cd web && npm run dev`). Log in, navigate to any project, click **Settings**. Verify:
- The project name input is pre-filled.
- Saving a new name updates the layout header.
- Saving an empty name shows an error.
- Clicking "Delete project" → confirming → redirects to `/projects`.
- The projects list no longer shows a Delete button on cards.

- [ ] **Step 7: Commit**

```bash
git add web/src/routes/projects/
git commit -m "feat(web): add project settings page with rename and delete"
```

---

## Task 3: Select All Attendees

**Files:**
- Modify: `web/src/lib/components/TournamentTab.svelte`

- [ ] **Step 1: Add derived state and toggleAll function**

In `web/src/lib/components/TournamentTab.svelte`, add after the existing `$derived` blocks (after line 39 — after `const alreadyAddedCount = ...`):

```ts
const selectableFiltered = $derived(
    filteredEntrants.filter(e => !alreadyAddedIds.has(e.startgg_user_id))
);
const allSelected = $derived(
    selectableFiltered.length > 0 &&
    selectableFiltered.every(e => selected.has(e.startgg_user_id))
);

function toggleAll() {
    const next = new Set(selected);
    if (allSelected) {
        for (const e of selectableFiltered) next.delete(e.startgg_user_id);
    } else {
        for (const e of selectableFiltered) next.add(e.startgg_user_id);
    }
    selected = next;
}
```

- [ ] **Step 2: Add the checkbox to the template**

In `web/src/lib/components/TournamentTab.svelte`, `Checkbox` and `Label` are already imported. Find the block that conditionally renders when `entrants.length > 0` (around line 110). Add a select-all row between the `<Input bind:value={search} .../>` line and the `<ScrollArea ...>` line:

```svelte
<div class="flex items-center gap-2">
    <Checkbox id="select-all" checked={allSelected} onCheckedChange={toggleAll} />
    <Label for="select-all" class="cursor-pointer text-sm font-normal">Select all</Label>
</div>
```

- [ ] **Step 3: Manually test the select-all behavior**

Navigate to a project → Players → Add players → From tournament. Fetch entrants for any tournament slug (e.g. `genesis-9`). Verify:
- The "Select all" checkbox appears above the list.
- Clicking it selects all non-already-added visible entrants.
- Typing in the search box narrows the list; clicking "Select all" again only selects the filtered results.
- With all selectable entrants selected, clicking again deselects all.
- Already-added entrants are unaffected throughout.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/components/TournamentTab.svelte
git commit -m "feat(web): add select-all checkbox to tournament entrant list"
```
