# Design: Runtime INTERNAL_API_URL

**Date:** 2026-05-19
**Status:** Approved

## Problem

`INTERNAL_API_URL` is imported from `$env/static/private` in 15 server-side files. This bakes the value into the SvelteKit build artifact at compile time, making the Docker image environment-specific — a separate build is required for each deployment target.

## Goal

Make the Docker image environment-agnostic by moving `INTERNAL_API_URL` to a runtime environment variable, injected when the container starts.

## Approach

Use `$env/dynamic/private` instead of `$env/static/private`. With `adapter-node`, SvelteKit reads private env vars from `process.env` at server startup. This is server-only (never serialized to the client), which is correct for an internal backend URL.

## Changes

### Web source files (15 files)

In every server-side file that imports `INTERNAL_API_URL`, switch to the `$env/dynamic/private` env object (`$env/dynamic/private` does not support named exports):

```ts
// before
import { INTERNAL_API_URL } from '$env/static/private';
// usage: INTERNAL_API_URL

// after
import { env } from '$env/dynamic/private';
// usage: env.INTERNAL_API_URL
```

Files to update:
- `src/hooks.server.ts`
- `src/routes/login/+page.server.ts`
- `src/routes/register/+page.server.ts`
- `src/routes/projects/+page.server.ts`
- `src/routes/projects/new/+page.server.ts`
- `src/routes/projects/[id]/+layout.server.ts`
- `src/routes/projects/[id]/import/+page.server.ts`
- `src/routes/projects/[id]/tournaments/+page.server.ts`
- `src/routes/projects/[id]/players/+page.server.ts`
- `src/routes/projects/[id]/players/[player_id]/+page.server.ts`
- `src/routes/projects/[id]/h2h/+page.server.ts`
- `src/routes/projects/[id]/ranking/+page.server.ts`
- `src/routes/projects/[id]/stats/+page.server.ts`
- `src/routes/projects/[id]/settings/+page.server.ts`
- `src/routes/invite/[token]/+page.server.ts`

### Dockerfile

Remove the build-time arg (no longer consumed at build time):

```dockerfile
# remove this line:
ARG INTERNAL_API_URL
```

### docker-compose.yml

Move `INTERNAL_API_URL` from `build.args` to `environment`:

```yaml
web:
  build:
    context: ./web
    # args block removed entirely (INTERNAL_API_URL was the only arg)
  environment:
    ORIGIN: http://localhost:5173
    PUBLIC_API_URL: http://localhost:3000
    INTERNAL_API_URL: http://api:3000
```

### docker-compose.prod.yml

Same move:

```yaml
web:
  build:
    context: ./web
    # args block removed entirely
  environment:
    ORIGIN: ${ORIGIN:-https://rankingforge.example.com}
    PUBLIC_API_URL: ${PUBLIC_API_URL:-https://rankingforge.example.com}
    INTERNAL_API_URL: ${INTERNAL_API_URL:-http://api:3000}
```

## Out of scope

- No changes to `playwright.config.ts` — it already passes `INTERNAL_API_URL` as a runtime env var to the dev server.
- No changes to test mocks or Vitest setup.
