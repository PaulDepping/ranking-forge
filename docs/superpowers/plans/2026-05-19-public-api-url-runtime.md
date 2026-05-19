# Runtime PUBLIC_API_URL Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move `PUBLIC_API_URL` from a SvelteKit compile-time static env var to a runtime env var so the Docker image is environment-agnostic.

**Architecture:** Swap the import source from `$env/static/public` to `$env/dynamic/public` in all frontend components. With `adapter-node`, SvelteKit reads `PUBLIC_API_URL` from `process.env` at server startup and serializes it into SSR HTML for client access. Update the Vitest alias and Docker config to match.

**Tech Stack:** SvelteKit (adapter-node), Vitest, Docker / docker-compose

---

### Task 1: Update Vitest alias

The unit test config aliases `$env/static/public` to the mock file. This needs to track the new import path, or unit tests will fail to resolve `PUBLIC_API_URL`.

**Files:**
- Modify: `web/vitest.config.ts`

- [ ] **Step 1: Update the alias key**

In `web/vitest.config.ts`, change the alias from `$env/static/public` to `$env/dynamic/public`:

```ts
// web/vitest.config.ts
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { resolve } from 'path';

export default defineConfig({
	plugins: [svelte()],
	resolve: {
		alias: {
			$lib: resolve('./src/lib'),
			'$env/dynamic/public': resolve('./src/__mocks__/env.ts')
		},
		conditions: ['browser']
	},
	test: {
		include: ['src/**/*.{test,spec}.{js,ts}'],
		globals: true,
		environment: 'jsdom',
		setupFiles: ['src/setupTests.ts']
	}
});
```

- [ ] **Step 2: Run unit tests to confirm the alias resolves**

```bash
cd web && npm run test:unit
```

Expected: all tests pass (same result as before this change — the mock still exports `PUBLIC_API_URL`, only the alias key changed).

- [ ] **Step 3: Commit**

```bash
git add web/vitest.config.ts
git commit -m "test: update vitest alias to \$env/dynamic/public"
```

---

### Task 2: Update SvelteKit component imports

Replace the static import with the dynamic one across all nine components.

**Files:**
- Modify: `web/src/routes/+layout.svelte`
- Modify: `web/src/routes/projects/new/+page.svelte`
- Modify: `web/src/routes/projects/[id]/tournaments/+page.svelte`
- Modify: `web/src/routes/projects/[id]/import/+page.svelte`
- Modify: `web/src/routes/projects/[id]/h2h/+page.svelte`
- Modify: `web/src/routes/projects/[id]/ranking/+page.svelte`
- Modify: `web/src/lib/components/NameTab.svelte`
- Modify: `web/src/lib/components/HandleTab.svelte`
- Modify: `web/src/lib/components/TournamentTab.svelte`

- [ ] **Step 1: Replace the import in all nine files**

In each file listed above, find:

```ts
import { PUBLIC_API_URL } from '$env/static/public';
```

Replace with:

```ts
import { PUBLIC_API_URL } from '$env/dynamic/public';
```

The import is on a single line in each file — no other changes are needed. All call sites (`makeApi(fetch, PUBLIC_API_URL)`, template literals, etc.) remain unchanged.

- [ ] **Step 2: Verify no remaining static imports**

```bash
grep -r "env/static/public" web/src/
```

Expected: no output.

- [ ] **Step 3: Run unit tests to confirm nothing broke**

```bash
cd web && npm run test:unit
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/routes/+layout.svelte \
        web/src/routes/projects/new/+page.svelte \
        web/src/routes/projects/[id]/tournaments/+page.svelte \
        web/src/routes/projects/[id]/import/+page.svelte \
        web/src/routes/projects/[id]/h2h/+page.svelte \
        web/src/routes/projects/[id]/ranking/+page.svelte \
        web/src/lib/components/NameTab.svelte \
        web/src/lib/components/HandleTab.svelte \
        web/src/lib/components/TournamentTab.svelte
git commit -m "feat: use \$env/dynamic/public for PUBLIC_API_URL"
```

---

### Task 3: Update Dockerfile

Remove the build-time `ARG PUBLIC_API_URL` — it is no longer consumed during the build.

**Files:**
- Modify: `web/Dockerfile`

- [ ] **Step 1: Remove the ARG line**

In `web/Dockerfile`, remove line 12:

```dockerfile
ARG PUBLIC_API_URL
```

Leave `ARG INTERNAL_API_URL` on the next line — it is still a build-time variable.

After the edit, lines 11–14 of the builder stage should look like:

```dockerfile
COPY . .

ARG INTERNAL_API_URL
RUN npm run build
```

- [ ] **Step 2: Commit**

```bash
git add web/Dockerfile
git commit -m "build: remove compile-time PUBLIC_API_URL ARG from Dockerfile"
```

---

### Task 4: Update docker-compose files

Move `PUBLIC_API_URL` from `build.args` (compile-time) to `environment` (runtime) in both compose files.

**Files:**
- Modify: `docker-compose.yml`
- Modify: `docker-compose.prod.yml`

- [ ] **Step 1: Update docker-compose.yml**

In `docker-compose.yml`, the `web` service currently reads:

```yaml
  web:
    build:
      context: ./web
      args:
        PUBLIC_API_URL: http://localhost:3000
        INTERNAL_API_URL: http://api:3000
    environment:
      ORIGIN: http://localhost:5173
```

Change it to:

```yaml
  web:
    build:
      context: ./web
      args:
        INTERNAL_API_URL: http://api:3000
    environment:
      ORIGIN: http://localhost:5173
      PUBLIC_API_URL: http://localhost:3000
```

- [ ] **Step 2: Update docker-compose.prod.yml**

In `docker-compose.prod.yml`, the `web` service currently reads:

```yaml
  web:
    build:
      context: ./web
      args:
        PUBLIC_API_URL: ${PUBLIC_API_URL:-https://rankingforge.example.com}
        INTERNAL_API_URL: http://api:3000
    environment:
      ORIGIN: ${ORIGIN:-https://rankingforge.example.com}
```

Change it to:

```yaml
  web:
    build:
      context: ./web
      args:
        INTERNAL_API_URL: http://api:3000
    environment:
      ORIGIN: ${ORIGIN:-https://rankingforge.example.com}
      PUBLIC_API_URL: ${PUBLIC_API_URL:-https://rankingforge.example.com}
```

- [ ] **Step 3: Commit**

```bash
git add docker-compose.yml docker-compose.prod.yml
git commit -m "build: move PUBLIC_API_URL from build args to runtime environment"
```

---

### Task 5: Full verification

Run the complete test suite to confirm no regressions.

**Files:** none

- [ ] **Step 1: Run all frontend tests**

```bash
cd web && npm run test:unit && npm run test:e2e
```

Expected: all unit tests pass; all Playwright e2e tests pass (the e2e suite uses a mock API on port 9999, so the runtime env var is not required for them to pass).

- [ ] **Step 2: Confirm no static/public imports remain anywhere**

```bash
grep -r "env/static/public" web/src/
```

Expected: no output.
