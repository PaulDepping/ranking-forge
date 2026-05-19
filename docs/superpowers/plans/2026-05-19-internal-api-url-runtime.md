# Runtime INTERNAL_API_URL Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move `INTERNAL_API_URL` from a build-time Docker ARG to a runtime environment variable so the web Docker image is environment-agnostic.

**Architecture:** Replace `$env/static/private` with `$env/dynamic/private` in all 15 server-side files. Remove `ARG INTERNAL_API_URL` from the Dockerfile. Move the value from `build.args` to `environment` in both docker-compose files. No functional behaviour changes — only when/how the value is resolved.

**Tech Stack:** SvelteKit (adapter-node), Docker, docker-compose

---

### Task 1: Update import paths in all server-side files

**Files:**
- Modify: `web/src/hooks.server.ts`
- Modify: `web/src/routes/login/+page.server.ts`
- Modify: `web/src/routes/register/+page.server.ts`
- Modify: `web/src/routes/projects/+page.server.ts`
- Modify: `web/src/routes/projects/new/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/+layout.server.ts`
- Modify: `web/src/routes/projects/[id]/import/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/tournaments/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/players/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/players/[player_id]/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/h2h/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/ranking/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/stats/+page.server.ts`
- Modify: `web/src/routes/projects/[id]/settings/+page.server.ts`
- Modify: `web/src/routes/invite/[token]/+page.server.ts`

- [ ] **Step 1: Replace the import in every file**

In each file listed above, find:
```ts
import { INTERNAL_API_URL } from '$env/static/private';
```
and replace with:
```ts
import { INTERNAL_API_URL } from '$env/dynamic/private';
```

The fastest way — run from the `web/` directory:
```bash
grep -rl "from '\$env/static/private'" src/ | xargs sed -i "s|from '\$env/static/private'|from '\$env/dynamic/private'|g"
```

- [ ] **Step 2: Verify no static/private imports remain**

```bash
grep -r "env/static/private" web/src/
```

Expected: no output.

- [ ] **Step 3: Run unit tests to confirm nothing broke**

```bash
cd web && npm run test:unit
```

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/
git commit -m "fix: use \$env/dynamic/private for INTERNAL_API_URL"
```

---

### Task 2: Update Dockerfile and docker-compose files

**Files:**
- Modify: `web/Dockerfile`
- Modify: `docker-compose.yml`
- Modify: `docker-compose.prod.yml`

- [ ] **Step 1: Remove the build-time ARG from the Dockerfile**

In `web/Dockerfile`, delete this line (it is on line 12, between `COPY . .` and `RUN npm run build`):
```dockerfile
ARG INTERNAL_API_URL
```

After the edit, the builder stage should look like:
```dockerfile
FROM node:22-slim AS builder
WORKDIR /app

COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci

COPY . .

RUN npm run build
```

- [ ] **Step 2: Update docker-compose.yml**

In `docker-compose.yml`, the `web` service currently has:
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

Replace it with:
```yaml
  web:
    build:
      context: ./web
    environment:
      ORIGIN: http://localhost:5173
      PUBLIC_API_URL: http://localhost:3000
      INTERNAL_API_URL: http://api:3000
```

- [ ] **Step 3: Update docker-compose.prod.yml**

In `docker-compose.prod.yml`, the `web` service currently has:
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

Replace it with:
```yaml
  web:
    build:
      context: ./web
    environment:
      ORIGIN: ${ORIGIN:-https://rankingforge.example.com}
      PUBLIC_API_URL: ${PUBLIC_API_URL:-https://rankingforge.example.com}
      INTERNAL_API_URL: ${INTERNAL_API_URL:-http://api:3000}
```

- [ ] **Step 4: Verify no build.args remain for the web service**

```bash
grep -n "INTERNAL_API_URL" web/Dockerfile docker-compose.yml docker-compose.prod.yml
```

Expected output (all three occurrences are now in `environment`, none in `ARG` or `args`):
```
docker-compose.yml:      INTERNAL_API_URL: http://api:3000
docker-compose.prod.yml:      INTERNAL_API_URL: ${INTERNAL_API_URL:-http://api:3000}
```
(`web/Dockerfile` should produce no output.)

- [ ] **Step 5: Confirm the SvelteKit build succeeds without INTERNAL_API_URL in the environment**

```bash
cd web && npm run build
```

Expected: build completes successfully (no error about missing env var, because `$env/dynamic/private` does not require the value at build time).

- [ ] **Step 6: Run the e2e tests to confirm the full server flow works**

```bash
cd web && npm run test:e2e
```

Expected: all Playwright tests pass. (`playwright.config.ts` already injects `INTERNAL_API_URL: 'http://localhost:9999'` at runtime — no changes needed there.)

- [ ] **Step 7: Commit**

```bash
git add web/Dockerfile docker-compose.yml docker-compose.prod.yml
git commit -m "build: move INTERNAL_API_URL from build arg to runtime environment"
```
