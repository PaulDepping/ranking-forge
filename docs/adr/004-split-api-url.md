# ADR 004: `PUBLIC_API_URL` and `INTERNAL_API_URL` as Separate Env Vars

## Context

The SvelteKit frontend makes API calls in two contexts: server-side `load` functions
(which run in the Node.js container) and client-side event handlers (which run in the
browser). In production the API is at `https://api.rankingforge.com`, but within the
Docker network the containers can reach each other at `http://api:8080`.

## Decision

Two env vars:

- `PUBLIC_API_URL` — the browser-facing URL, used for client-side fetches
- `INTERNAL_API_URL` — the Docker-internal URL, used for server-side `load` functions

Two separate API client modules handle the split:
- `src/lib/api.ts` — client-side only; always reads `PUBLIC_API_URL`
- `src/lib/server/api.ts` — server-side only; always reads `INTERNAL_API_URL`

SvelteKit's `/server/` path convention prevents the server module from being imported in client code.

## Rationale

- **Performance.** Server-side `load` functions can reach the API via the Docker
  internal network, avoiding a round-trip over the public internet and TLS negotiation.
- **Reliability.** Server-side fetches do not depend on external DNS or the public CDN.
- Using a single public URL for both contexts adds unnecessary latency and an external
  dependency in the server-to-server path.

## Consequences

- Both vars must be set in production. `INTERNAL_API_URL` is typically
  `http://api:8080` (the Docker service name and port).
- In local development, both can point to `http://localhost:8080`.
- When adding new API calls in SvelteKit `load` functions, use `INTERNAL_API_URL`.
  In `+page.svelte` client-side code, the API client automatically uses `PUBLIC_API_URL`.
