# ADR 002: Real Isolated Schemas in Tests, No DB Mocks

## Context

Tests that exercise database code need a strategy for the DB layer. Options: mock sqlx
query traits, use an in-memory SQLite, or use real Postgres via `#[sqlx::test]`.

## Decision

All tests that touch the database use `#[sqlx::test(migrations = "../../migrations")]`,
which spins up a real Postgres schema per test and tears it down on completion.

## Rationale

- **Compile-time query checking is not enough.** sqlx validates column names and types
  at compile time but does not catch constraint violations, NULL edge cases, or
  transaction behaviour. These bugs only surface against a real schema.
- **Mocks hide the failure modes that matter.** A mock that returns what you tell it to
  return can pass every assertion while the real query silently fails on INSERT conflicts
  or unexpected NULLs.
- **`#[sqlx::test]` is low friction.** The macro handles schema setup and teardown per
  test. Tests are fully isolated — no shared state, no ordering dependencies.

## Consequences

- Tests require a Postgres connection. `backend/test.sh` provides one via Docker.
- Running `cargo test -p api` or `cargo test -p e2e` directly requires `DATABASE_URL`.
- Schema changes are immediately visible in tests; no mock layer to synchronise.
- Do not use in-memory SQLite or mock the `PgPool` — it defeats the purpose.
