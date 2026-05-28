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

`src/lib/api.ts` selects the correct URL based on whether the code is running in the
browser or on the server.

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
