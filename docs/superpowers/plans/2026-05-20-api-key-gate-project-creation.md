# API Key Gate for Project Creation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Block project creation when the user has no start.gg API key — enforced in both the backend handler and the frontend new-project page.

**Architecture:** The backend gets an early guard in `create_project` returning 422. Existing tests that create projects as setup (not testing creation itself) need a SQL key-fixture call added before the route call. The frontend new-project page reads `locals.user.has_startgg_key` from a new load function and shows a blocking `Card` callout (matching the import page pattern) in place of the form when false.

**Tech Stack:** Rust/Axum (backend), SvelteKit + Svelte 5 runes, shadcn-svelte Card component, Playwright (e2e tests)

---

### Task 1: Backend — guard `create_project` and fix all affected tests

**Files:**
- Modify: `backend/crates/api/src/routes/projects.rs`
- Modify: `backend/crates/api/src/routes/account.rs`
- Modify: `backend/crates/e2e/tests/full_flow.rs`

- [ ] **Step 1: Write the failing test**

Add this inside the existing `mod tests` block in `backend/crates/api/src/routes/projects.rs`, after `test_create_project_sets_owner_id`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_create_project_requires_startgg_key(pool: PgPool) {
    let app = make_app(pool.clone());
    let cookie = register(&app, "nokeyuser").await;
    // Newly registered users have no API key — route must return 422

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "My Project"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
```

- [ ] **Step 2: Run the new test to confirm it fails**

```bash
cd backend && cargo test -p api -- test_create_project_requires_startgg_key
```

Expected: FAIL — the handler currently creates the project without checking for a key.

- [ ] **Step 3: Add the guard to `create_project`**

In `backend/crates/api/src/routes/projects.rs`, find `async fn create_project`. Add this block immediately after the two `name` validation checks (after the `> 100` chars check, before the `sqlx::query_as!` INSERT):

```rust
if user.startgg_api_key.is_none() {
    return Err(AppError::UnprocessableEntity(
        "A start.gg API key is required to create projects".into(),
    ));
}
```

- [ ] **Step 4: Run the new test to confirm it passes**

```bash
cd backend && cargo test -p api -- test_create_project_requires_startgg_key
```

Expected: PASS.

- [ ] **Step 5: Add a `with_api_key` test helper to `projects.rs`**

The `create_project` helper in `mod tests` calls `POST /projects` directly. After the guard, all tests using it (or calling that endpoint inline) will get 422 unless the user has a key. Add this helper inside `mod tests`:

```rust
async fn with_api_key(pool: &PgPool, email: &str) {
    sqlx::query!(
        "UPDATE users SET startgg_api_key = 'test-key' WHERE email = $1",
        email
    )
    .execute(pool)
    .await
    .unwrap();
}
```

- [ ] **Step 6: Fix all tests in `projects.rs` that call `create_project`**

Each test that calls `create_project(&app, &cookie, ...)` needs a `with_api_key` call inserted before it, using the owner's email. Apply these changes:

**`test_list_projects_shows_all_member_roles`** — insert after `register(&app, "owner1")`:
```rust
let owner_cookie = register(&app, "owner1").await;
with_api_key(&pool, "owner1@test.com").await;
let proj_id = create_project(&app, &owner_cookie, "Test Project").await;
```

**`test_create_project_sets_owner_id`** — insert after `register(&app, "owner2")`:
```rust
let cookie = register(&app, "owner2").await;
with_api_key(&pool, "owner2@test.com").await;
let proj_id = create_project(&app, &cookie, "My Project").await;
```

**`test_non_member_gets_404`** — insert after `register(&app, "owner3")`:
```rust
let owner_cookie = register(&app, "owner3").await;
with_api_key(&pool, "owner3@test.com").await;
let proj_id = create_project(&app, &owner_cookie, "Private Project").await;
```

**`test_unauthenticated_can_access_published_project`** — insert after `register(&app, "owner4")`:
```rust
let cookie = register(&app, "owner4").await;
with_api_key(&pool, "owner4@test.com").await;
let proj_id = create_project(&app, &cookie, "Public Project").await;
```

**`test_unauthenticated_cannot_access_private_project`** — insert after `register(&app, "owner5")`:
```rust
let cookie = register(&app, "owner5").await;
with_api_key(&pool, "owner5@test.com").await;
let proj_id = create_project(&app, &cookie, "Private Project").await;
```

**`test_unauthenticated_can_read_stats_of_published_project`** — insert after `register(&app, "owner_stats")`:
```rust
let cookie = register(&app, "owner_stats").await;
with_api_key(&pool, "owner_stats@test.com").await;
let proj_id = create_project(&app, &cookie, "Stats Project").await;
```

**`test_only_owner_can_delete`** — insert after `register(&app, "owner6")`:
```rust
let owner_cookie = register(&app, "owner6").await;
with_api_key(&pool, "owner6@test.com").await;
let proj_id = create_project(&app, &owner_cookie, "Project").await;
```

**`test_viewer_cannot_add_player`** — insert after `register(&app, "owner_pl")`:
```rust
let owner_cookie = register(&app, "owner_pl").await;
with_api_key(&pool, "owner_pl@test.com").await;
let proj_id = create_project(&app, &owner_cookie, "Player Project").await;
```

**`test_get_project_includes_owner_has_startgg_key`** — this test specifically checks the `false`→`true` transition of `owner_has_startgg_key`. Restructure it to use the `remove key`→`add key` direction instead:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_get_project_includes_owner_has_startgg_key(pool: PgPool) {
    let app = make_app(pool.clone());
    let cookie = register(&app, "keyowner").await;

    // Set key so project creation succeeds
    with_api_key(&pool, "keyowner@test.com").await;
    let proj_id = create_project(&app, &cookie, "Key Project").await;

    // Key is set — owner_has_startgg_key should be true
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/projects/{proj_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body = json_body(resp).await;
    assert_eq!(body["owner_has_startgg_key"], true);

    // Remove key — owner_has_startgg_key should become false
    sqlx::query!(
        "UPDATE users SET startgg_api_key = NULL WHERE email = 'keyowner@test.com'"
    )
    .execute(&pool)
    .await
    .unwrap();

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/projects/{proj_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = json_body(resp).await;
    assert_eq!(body["owner_has_startgg_key"], false);
}
```

- [ ] **Step 7: Fix the inline project creation in `account.rs` tests**

In `backend/crates/api/src/routes/account.rs`, find `test_delete_account_cascades_projects`. It calls `POST /projects` inline. Insert a key-fixture call before the `POST /projects` request:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_delete_account_cascades_projects(pool: PgPool) {
    let app = make_app(pool.clone());
    let cookie = register(&app, "deluser").await;

    // Set key so project creation succeeds
    sqlx::query!(
        "UPDATE users SET startgg_api_key = 'k' WHERE email = 'deluser@test.com'"
    )
    .execute(&pool)
    .await
    .unwrap();

    let resp = app
        .clone()
        .oneshot(
            // ... existing POST /projects request unchanged
```

- [ ] **Step 8: Fix the inline project creation in the e2e crate**

In `backend/crates/e2e/tests/full_flow.rs`, find `test_rename_project`. It calls `post_json(&app, "/projects", ...)` without setting a key first. Insert a `set_startgg_api_key` call before the `POST /projects` call (the helper already exists in that file):

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn test_rename_project(pool: PgPool) {
    let app = make_app(pool, "http://unused");
    let cookie = register(&app, "alice", "password123").await;

    set_startgg_api_key(&pool, &cookie, "test-key").await;

    // Create a project
    let resp = post_json(&app, "/projects", &cookie, json!({"name": "Original"})).await;
    // ... rest of test unchanged
```

- [ ] **Step 9: Run the full backend test suite**

```bash
cd backend && cargo test -p api && cargo test -p e2e
```

Expected: all tests pass. If any other tests fail with 422 on `POST /projects`, apply the same key-fixture pattern.

- [ ] **Step 10: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/api/src/routes/projects.rs \
        backend/crates/api/src/routes/account.rs \
        backend/crates/e2e/tests/full_flow.rs
git commit -m "feat: require start.gg API key to create projects"
```

---

### Task 2: Frontend — callout on the new project page

**Files:**
- Modify: `web/tests/mock-api.js` — add `has_startgg_key` to mock user; add no-key session
- Modify: `web/tests/projects.test.ts` — add e2e test for the callout
- Modify: `web/src/routes/projects/new/+page.server.ts` — add load function
- Modify: `web/src/routes/projects/new/+page.svelte` — add callout

- [ ] **Step 1: Add `has_startgg_key` to the mock user and add a no-key session**

In `web/tests/mock-api.js`, update `MOCK_USER` to include the field and add `MOCK_USER_NO_KEY` directly below it:

```js
const MOCK_USER = { id: 'user-1', email: 'testuser@test.com', display_name: 'testuser', has_startgg_key: true, created_at: '2026-01-01T00:00:00Z' };

const MOCK_USER_NO_KEY = { id: 'user-2', email: 'nokey@test.com', display_name: 'nokey', has_startgg_key: false, created_at: '2026-01-01T00:00:00Z' };
```

Then update the `/auth/me` handler. Find:

```js
if (path === '/auth/me') {
    if (isAuthenticated) respond(res, 200, MOCK_USER);
    else respond(res, 401, { message: 'Unauthorized' });
    return;
}
```

Replace with:

```js
if (path === '/auth/me') {
    if (hasCookie(req, 'session_id', 'nokey-session')) {
        respond(res, 200, MOCK_USER_NO_KEY);
    } else if (isAuthenticated) {
        respond(res, 200, MOCK_USER);
    } else {
        respond(res, 401, { message: 'Unauthorized' });
    }
    return;
}
```

- [ ] **Step 2: Write the failing e2e test**

In `web/tests/projects.test.ts`, add this test after the existing tests. Use `base` (already imported as `import { test as base, expect } from '@playwright/test'`) to set a different session cookie:

```typescript
base('new project page shows callout when user has no start.gg API key', async ({ page }) => {
    await page.context().addCookies([{
        name: 'session_id',
        value: 'nokey-session',
        domain: 'localhost',
        path: '/'
    }]);
    await page.goto('/projects/new');
    await expect(page.getByText('A start.gg API key is required to create projects.')).toBeVisible();
    await expect(page.getByRole('link', { name: 'account settings' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Create' })).not.toBeVisible();
});
```

- [ ] **Step 3: Run the new test to confirm it fails**

```bash
cd web && npm run test:e2e -- --grep "no start.gg API key"
```

Expected: FAIL — the page currently shows the form regardless of API key status.

- [ ] **Step 4: Add the load function to `+page.server.ts`**

Replace the entire contents of `web/src/routes/projects/new/+page.server.ts` with:

```typescript
import { fail, redirect } from "@sveltejs/kit";
import type { Actions, PageServerLoad } from "./$types";
import { makeApi } from "$lib/api";
import { env } from "$env/dynamic/private";

export const load: PageServerLoad = ({ locals }) => {
  return { hasStartggKey: locals.user?.has_startgg_key ?? false };
};

export const actions: Actions = {
  default: async ({ fetch, request, cookies }) => {
    const data = await request.formData();
    const name = (data.get("name") as string)?.trim();
    const game_id_raw = data.get("game_id") as string | null;
    const game_name = (data.get("game_name") as string | null) || null;

    if (!name) return fail(422, { error: "Project name is required" });

    const body: Record<string, unknown> = { name };
    if (game_id_raw) body.game_id = parseInt(game_id_raw, 10);
    if (game_name) body.game_name = game_name;

    const api = makeApi(fetch, env.INTERNAL_API_URL, cookies.get("session_id"));
    const res = await api.post("/projects", body);

    if (!res.ok) {
      const err = await res
        .json()
        .catch(() => ({ message: "Failed to create project" }));
      return fail(res.status, { error: err.message });
    }

    const project = await res.json();
    redirect(303, `/projects/${project.id}/players`);
  },
};
```

- [ ] **Step 5: Update `+page.svelte` to destructure `data` and add the callout**

In `web/src/routes/projects/new/+page.svelte`, make two changes:

**Change 1** — add `* as Card` to imports and add `data` to the props line. The top of the `<script>` block becomes:

```svelte
<script lang="ts">
  import { enhance } from "$app/forms";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { Alert } from "$lib/components/ui/alert";
  import * as Popover from "$lib/components/ui/popover";
  import * as Command from "$lib/components/ui/command";
  import * as Card from "$lib/components/ui/card";
  import { env } from "$env/dynamic/public";
  import type { Game } from "$lib/types";

  let { form, data } = $props();

  // rest of script unchanged (gameQuery, gameResults, etc.)
```

**Change 2** — in the template, wrap the content below `<h1>` with the key check. The full template becomes:

```svelte
<div class="max-w-md space-y-6">
  <h1 class="text-2xl font-bold">New project</h1>

  {#if data.hasStartggKey}
    {#if form?.error}
      <Alert variant="destructive">{form.error}</Alert>
    {/if}

    <form method="POST" use:enhance class="space-y-4">
      <div class="space-y-2">
        <Label for="name">Project name</Label>
        <Input
          id="name"
          name="name"
          required
          placeholder="e.g. NY Smash PR 2025"
        />
      </div>

      <div class="space-y-2">
        <Label for="game-search">Game (optional)</Label>
        <Popover.Root bind:open={gameSearchOpen}>
          <Popover.Trigger>
            {#snippet child({ props })}
              <Button
                variant="outline"
                class="w-full justify-start font-normal"
                {...props}
              >
                {#if selectedGame}
                  {selectedGame.display_name ?? selectedGame.name}
                {:else}
                  <span class="text-muted-foreground">Search start.gg games…</span
                  >
                {/if}
              </Button>
            {/snippet}
          </Popover.Trigger>
          <Popover.Content class="p-0 w-80" align="start">
            <Command.Root shouldFilter={false}>
              <Command.Input
                placeholder="Search start.gg games…"
                value={gameQuery}
                oninput={(e) =>
                  onCommandInput((e.target as HTMLInputElement).value)}
              />
              <Command.List>
                {#if searching}
                  <Command.Loading>Searching…</Command.Loading>
                {:else if gameQuery.length >= 2 && gameResults.length === 0}
                  <Command.Empty>No games found.</Command.Empty>
                {:else}
                  {#each gameResults as g (g.id)}
                    <Command.Item
                      value={g.id.toString()}
                      onSelect={() => selectGame(g)}
                    >
                      {g.display_name ?? g.name}
                    </Command.Item>
                  {/each}
                {/if}
              </Command.List>
            </Command.Root>
          </Popover.Content>
        </Popover.Root>
      </div>

      <input type="hidden" name="game_id" value={selectedGame?.id ?? ""} />
      <input
        type="hidden"
        name="game_name"
        value={selectedGame
          ? (selectedGame.display_name ?? selectedGame.name)
          : ""}
      />

      <div class="flex gap-2">
        <Button type="submit">Create</Button>
        <Button variant="ghost" href="/projects">Cancel</Button>
      </div>
    </form>
  {:else}
    <Card.Root>
      <Card.Content class="p-4 space-y-2">
        <p class="text-sm font-medium">A start.gg API key is required to create projects.</p>
        <p class="text-sm text-muted-foreground">
          Add your key in <a href="/account" class="underline">account settings</a>.
        </p>
      </Card.Content>
    </Card.Root>
  {/if}
</div>
```

- [ ] **Step 6: Run the e2e test to confirm it passes**

```bash
cd web && npm run test:e2e -- --grep "no start.gg API key"
```

Expected: PASS.

- [ ] **Step 7: Run the full e2e suite to check for regressions**

```bash
cd web && npm run test:e2e
```

Expected: all tests pass.

- [ ] **Step 8: Commit**

```bash
cd web && npm run format
git add web/tests/mock-api.js web/tests/projects.test.ts \
        web/src/routes/projects/new/+page.server.ts \
        web/src/routes/projects/new/+page.svelte
git commit -m "feat: show no-key callout on new project page when owner has no start.gg API key"
```
