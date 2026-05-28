# ADR 006: Vitest Uses `svelte()` Plugin with `conditions: ['browser']`

## Context

Unit tests for Svelte components need to mount components in jsdom. The app build uses
the `sveltekit()` Vite plugin. When this same plugin is used in Vitest, tests fail
with "mount is not a function".

## Decision

The Vitest configuration (in `vitest.config.ts`) uses the plain `svelte()` Vite plugin
with `resolve: { conditions: ['browser'] }` instead of `sveltekit()`.

## Rationale

`sveltekit()` resolves the `svelte` package to its SSR (server-side) entry point.
The SSR build does not export `mount()`, which Vitest tests use to render components
in jsdom. The plain `svelte()` plugin with `conditions: ['browser']` resolves to
the client-side build, making `mount()` available.

## Consequences

- SvelteKit-specific module aliases (`$app/navigation`, `$env/static/public`, etc.)
  are not automatically available in tests. They must be mocked in `src/__mocks__/`.
  Existing mocks live in `src/__mocks__/app-navigation.ts`, `src/__mocks__/env.ts`,
  and `src/__mocks__/env.private.ts`.
- When adding tests for a component that imports new `$app/` or `$env/` modules, add
  a corresponding mock file in `src/__mocks__/` before writing the test.
- The Vitest config is separate from `vite.config.ts`. When `npm run test:unit` runs,
  it uses the Vitest configuration exclusively.
