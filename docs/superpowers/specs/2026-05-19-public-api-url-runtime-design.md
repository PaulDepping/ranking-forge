# Design: Runtime PUBLIC_API_URL

**Date:** 2026-05-19
**Status:** Approved

## Problem

`PUBLIC_API_URL` is currently imported from `$env/static/public`, which bakes the value into the SvelteKit bundle at build time. This makes the Docker image environment-specific — a separate build is required for each deployment target.

## Goal

Make the Docker image environment-agnostic by moving `PUBLIC_API_URL` to a runtime environment variable, injected when the container starts.

## Approach

Use `$env/dynamic/public` instead of `$env/static/public`. With `adapter-node`, SvelteKit reads `PUBLIC_API_URL` from `process.env` at server startup and serializes it into the SSR HTML, making it available to client-side code as well. The call sites are unchanged — the import path is the only difference.

## Changes

### Web source files (9 files)

In every `.svelte` file that imports `PUBLIC_API_URL`, change the import source:

```ts
// before
import { PUBLIC_API_URL } from '$env/static/public';

// after
import { PUBLIC_API_URL } from '$env/dynamic/public';
```

Files to update:
- `src/routes/+layout.svelte`
- `src/routes/projects/new/+page.svelte`
- `src/routes/projects/[id]/tournaments/+page.svelte`
- `src/routes/projects/[id]/import/+page.svelte`
- `src/routes/projects/[id]/h2h/+page.svelte`
- `src/routes/projects/[id]/ranking/+page.svelte`
- `src/lib/components/NameTab.svelte`
- `src/lib/components/HandleTab.svelte`
- `src/lib/components/TournamentTab.svelte`

`src/__mocks__/env.ts` is unaffected — it's a Vitest mock, not a real SvelteKit env import.

### Dockerfile

Remove the build-time arg (no longer consumed):

```dockerfile
# remove this line:
ARG PUBLIC_API_URL
```

### docker-compose.yml

Move `PUBLIC_API_URL` from `build.args` to `environment`:

```yaml
web:
  build:
    context: ./web
    args:
      INTERNAL_API_URL: http://api:3000  # keep — still a build-time var
  environment:
    ORIGIN: http://localhost:5173
    PUBLIC_API_URL: http://localhost:3000
```

### docker-compose.prod.yml

Same move:

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

## Out of scope

- `INTERNAL_API_URL` remains a build-time arg (it is used server-side only and is not a `PUBLIC_` var).
- No changes to test infrastructure or the Vitest mock.
