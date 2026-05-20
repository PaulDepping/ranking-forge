# Editor Route Access Control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restrict `/players` and `/import` project routes to editors and owners, redirect unauthorized visitors to `/login` with a return URL, and send non-editor visitors to `/ranking` by default when they land on a project root.

**Architecture:** A SvelteKit route group `(editor)` under `routes/projects/[id]/` holds all editor-restricted pages. A single `+layout.server.ts` in that group calls `parent()` to read the already-loaded project role (no extra API call) and redirects to `/login?redirect=<pathname>` for anyone without editor/owner access. The root project redirect becomes role-aware. The login page is updated to read the `redirect` param via a hidden form input and honour it after successful login.

**Tech Stack:** SvelteKit (server-side load functions, route groups), TypeScript, Playwright (e2e tests)

**Spec:** `docs/superpowers/specs/2026-05-20-editor-route-access-control-design.md`

---

### Task 1: Add failing e2e tests for restricted page access

**Files:**
- Modify: `web/tests/auth.test.ts`

- [ ] **Step 1: Append two redirect tests to `auth.test.ts`**

The mock API returns `{ user_role: 'viewer' }` for project ID `proj-viewer`. Viewer (and null) are both blocked by the guard we're about to add.

```ts
test('viewer visiting /players is redirected to login with return URL', async ({ page }) => {
    await page.goto('/projects/proj-viewer/players');
    await expect(page).toHaveURL('/login?redirect=%2Fprojects%2Fproj-viewer%2Fplayers');
});

test('viewer visiting /import is redirected to login with return URL', async ({ page }) => {
    await page.goto('/projects/proj-viewer/import');
    await expect(page).toHaveURL('/login?redirect=%2Fprojects%2Fproj-viewer%2Fimport');
});
```

- [ ] **Step 2: Run and confirm both new tests fail**

```bash
cd web && npx playwright test tests/auth.test.ts --reporter=line
```

Expected: the two new tests FAIL (pages load successfully instead of redirecting). All pre-existing tests PASS.

---

### Task 2: Create `(editor)` route group, move pages, add guard

**Files:**
- Create: `web/src/routes/projects/[id]/(editor)/+layout.server.ts`
- Move: `web/src/routes/projects/[id]/players/` → `web/src/routes/projects/[id]/(editor)/players/`
- Move: `web/src/routes/projects/[id]/import/` → `web/src/routes/projects/[id]/(editor)/import/`

Note: `(editor)` is a SvelteKit route group — the parenthetical name does not affect URL paths. `/projects/{id}/players` continues to resolve correctly after the move.

- [ ] **Step 1: Create the `(editor)` directory**

```bash
mkdir -p "web/src/routes/projects/[id]/(editor)"
```

- [ ] **Step 2: Create the layout guard**

Create `web/src/routes/projects/[id]/(editor)/+layout.server.ts` with this content:

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

`viewer` is intentionally excluded — viewers cannot manage players or trigger imports. `url.pathname` (not `url.href`) prevents query strings from being carried into the redirect param.

- [ ] **Step 3: Move `players/` into the route group**

```bash
git mv "web/src/routes/projects/[id]/players" "web/src/routes/projects/[id]/(editor)/players"
```

This moves both `+page.svelte` and `+page.server.ts`, plus the nested `[player_id]/` sub-route. No content changes are needed in any moved file.

- [ ] **Step 4: Move `import/` into the route group**

```bash
git mv "web/src/routes/projects/[id]/import" "web/src/routes/projects/[id]/(editor)/import"
```

- [ ] **Step 5: Run the failing tests from Task 1 to verify they now pass**

```bash
cd web && npx playwright test tests/auth.test.ts --reporter=line
```

Expected: all tests PASS, including the two added in Task 1.

- [ ] **Step 6: Commit**

```bash
git add "web/src/routes/projects/[id]/(editor)/+layout.server.ts"
git commit -m "feat: restrict players and import routes to editors via route group"
```

The `git mv` commands in Steps 3–4 already staged the moved directories; only the new layout file needs explicit staging.

---

### Task 3: Add failing test for default project redirect

**Files:**
- Modify: `web/tests/auth.test.ts`

- [ ] **Step 1: Append one redirect test**

```ts
test('visiting project root as non-editor redirects to ranking', async ({ page }) => {
    await page.goto('/projects/proj-viewer');
    await expect(page).toHaveURL('/projects/proj-viewer/ranking');
});
```

- [ ] **Step 2: Run and confirm it fails**

```bash
cd web && npx playwright test tests/auth.test.ts --reporter=line
```

Expected: new test FAILS (current code unconditionally redirects to `/players`, which then redirects to `/login`). All other tests PASS.

---

### Task 4: Update default project redirect to be role-aware

**Files:**
- Modify: `web/src/routes/projects/[id]/+page.server.ts`

- [ ] **Step 1: Replace the unconditional redirect with role-aware logic**

Replace the entire file content:

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

Editors and owners still land on `players`. Viewers and logged-out visitors land on `ranking`.

- [ ] **Step 2: Run the failing test from Task 3 to verify it passes**

```bash
cd web && npx playwright test tests/auth.test.ts --reporter=line
```

Expected: all tests PASS.

- [ ] **Step 3: Commit**

```bash
git add "web/src/routes/projects/[id]/+page.server.ts"
git commit -m "feat: redirect public visitors to ranking tab by default"
```

---

### Task 5: Add failing tests for login redirect support

**Files:**
- Modify: `web/tests/auth.test.ts`

- [ ] **Step 1: Append two tests for the login redirect feature**

```ts
test('login page with ?redirect= passes destination via hidden input', async ({ page }) => {
    await page.goto('/login?redirect=/projects/proj-viewer/ranking');
    await expect(page.locator('input[name="redirect"]')).toHaveValue('/projects/proj-viewer/ranking');
});

test('successful login redirects to the preserved destination', async ({ page }) => {
    await page.goto('/login?redirect=/projects/proj-viewer/ranking');
    await page.getByLabel('Email').fill('testuser@test.com');
    await page.getByLabel('Password').fill('testpass');
    await page.getByRole('button', { name: 'Sign in' }).click();
    await expect(page).toHaveURL('/projects/proj-viewer/ranking');
});
```

- [ ] **Step 2: Run and confirm both fail**

```bash
cd web && npx playwright test tests/auth.test.ts --reporter=line
```

Expected: both new tests FAIL. All other tests PASS.

---

### Task 6: Update login page to honour the redirect param

**Files:**
- Modify: `web/src/routes/login/+page.server.ts`
- Modify: `web/src/routes/login/+page.svelte`

- [ ] **Step 1: Replace `+page.server.ts` with redirect-aware version**

Replace the entire file content:

```ts
import { fail, redirect } from '@sveltejs/kit';
import type { Actions, PageServerLoad } from './$types';
import { env } from '$env/dynamic/private';

export const load: PageServerLoad = ({ locals, url }) => {
    if (locals.user) redirect(303, '/projects');
    const redirectTo = url.searchParams.get('redirect') ?? '/projects';
    return { redirectTo };
};

export const actions: Actions = {
    default: async ({ fetch, request, cookies }) => {
        const data = await request.formData();
        const email = data.get('email') as string;
        const password = data.get('password') as string;
        const redirectTo = (data.get('redirect') as string) ?? '/projects';

        const res = await fetch(`${env.INTERNAL_API_URL}/auth/login`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email, password })
        });

        if (!res.ok) {
            const body = await res.json().catch(() => ({ message: 'Login failed' }));
            return fail(res.status, { error: body.message ?? 'Login failed' });
        }

        const setCookie = res.headers.get('set-cookie');
        const match = setCookie?.match(/session_id=([^;]+)/);
        if (match) {
            cookies.set('session_id', match[1], {
                path: '/',
                httpOnly: true,
                sameSite: 'strict',
                maxAge: 60 * 60 * 24 * 30
            });
        }

        const safe = redirectTo.startsWith('/') ? redirectTo : '/projects';
        redirect(303, safe);
    }
};
```

The `startsWith('/')` guard prevents open redirect attacks (e.g. `?redirect=https://evil.com`).

- [ ] **Step 2: Update `+page.svelte` to expose `data` and add the hidden input**

In `web/src/routes/login/+page.svelte`, make two changes:

Change line 8 from:
```svelte
let { form } = $props();
```
To:
```svelte
let { form, data } = $props();
```

Add a hidden input as the first child of the `<form>` element. The full updated form block:
```svelte
<form method="POST" use:enhance class="space-y-4">
    <input type="hidden" name="redirect" value={data.redirectTo} />
    <div class="space-y-2">
        <Label for="email">Email</Label>
        <Input id="email" name="email" type="email" required autocomplete="email" />
    </div>
    <div class="space-y-2">
        <Label for="password">Password</Label>
        <Input id="password" name="password" type="password" required autocomplete="current-password" />
    </div>
    <Button type="submit" class="w-full">Sign in</Button>
</form>
```

- [ ] **Step 3: Run the failing tests from Task 5 to verify they pass**

```bash
cd web && npx playwright test tests/auth.test.ts --reporter=line
```

Expected: all tests PASS.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/login/+page.server.ts web/src/routes/login/+page.svelte
git commit -m "feat: preserve login destination via hidden redirect input"
```

---

### Task 7: Run the full test suite

- [ ] **Step 1: Run all tests from the repository root**

```bash
bash test.sh
```

Expected: PASS for all sections — backend (common, api, e2e) and frontend (unit + e2e). If any section fails, investigate and fix before declaring complete.
