# Architecture Decision Records

This directory records non-obvious architectural decisions: what was chosen, why, and
what it means in practice.

Each ADR has four sections:
- **Context** — the situation or problem that prompted the decision
- **Decision** — what was chosen
- **Rationale** — why this option over the alternatives
- **Consequences** — what becomes easier, harder, or constrained as a result

## Index

| # | Decision |
|---|---|
| [001](001-postgres-job-queue.md) | Postgres NOTIFY/LISTEN for the job queue |
| [002](002-no-db-mocks.md) | Real isolated schemas in tests, no DB mocks |
| [003](003-startgg-client-injection.md) | All start.gg calls through `StartggClient` |
| [004](004-split-api-url.md) | `PUBLIC_API_URL` and `INTERNAL_API_URL` as separate env vars |
| [005](005-samesite-strict-cookie.md) | `SameSite=Strict` on the cross-subdomain session cookie |
| [006](006-vitest-svelte-plugin.md) | Vitest uses `svelte()` plugin, not `sveltekit()` |
