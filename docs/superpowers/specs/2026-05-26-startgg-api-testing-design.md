# Design: start.gg API Testing

**Date:** 2026-05-26  
**Status:** Approved

## Problem

All start.gg API calls are tested via wiremock. The wiremock suite is thorough (happy paths, pagination, error codes, edge cases) but it validates nothing about the real API:

- **Schema drift** — a field name typo or invalid argument in a query string goes undetected until it fails in production.
- **Wire-format surprises** — our mock responses may not match what the real API actually returns (missing nulls, type coercions, undocumented quirks).

## Solution: Two-layer testing (Option C)

### Layer 1: Offline schema validation

Unit tests that validate each of the 6 query string constants against the start.gg GraphQL SDL schema. Runs unconditionally in `cargo test -p common` — no API key, no network.

### Layer 2: Live full-pipeline e2e tests

Full import pipeline tests that use the real start.gg API and an ephemeral Postgres DB. Gated on `STARTGG_API_KEY` being set: silently pass when absent, run fully when present. These live in `crates/e2e` alongside the existing wiremock-based e2e tests, and follow the same `#[sqlx::test]` pattern.

## Architecture

```
backend/crates/common/
  src/
    startgg/
      schema.graphql          ← moved here from docs/startgg/
      operations/
        tests.rs              ← new: offline schema validation tests

backend/crates/e2e/
  tests/
    import_live.rs            ← new: live full-pipeline import tests
```

## Layer 1: Offline schema validation

### Schema file location

`docs/startgg/schema.graphql` moves to `backend/crates/common/src/startgg/schema.graphql`. Rationale:

- The schema is a compile-time dependency of the Rust code, not documentation.
- Cargo tracks files under the crate root for recompilation; `docs/` is outside the crate root and would not trigger recompilation on schema change.
- `include_str!("schema.graphql")` from the adjacent test module is clean with no fragile relative path.

`docs/startgg/fetch-schema.sh` is updated to write to the new location. The `docs/startgg/project-notes.md` reference is updated to point to the new path.

### Dependency

Add `graphql-parser` as a dev-dependency in `backend/crates/common/Cargo.toml`.

### Test structure

New submodule `backend/crates/common/src/startgg/operations/tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use graphql_parser::{parse_query, parse_schema, query::Document, schema::Document as Schema};

    fn schema() -> Schema<'static, String> {
        let src = include_str!("../schema.graphql");
        parse_schema::<String>(src).expect("schema.graphql failed to parse")
    }

    fn assert_valid_query(query: &str) {
        let _doc: Document<String> = parse_query(query).expect("query failed to parse");
    }

    #[test] fn game_search_query_is_valid() { assert_valid_query(GAME_SEARCH_QUERY); }
    #[test] fn user_by_slug_query_is_valid() { assert_valid_query(USER_BY_SLUG_QUERY); }
    #[test] fn tournaments_by_user_query_is_valid() { assert_valid_query(TOURNAMENTS_BY_USER_QUERY); }
    #[test] fn tournaments_by_user_all_games_query_is_valid() { assert_valid_query(TOURNAMENTS_BY_USER_ALL_GAMES_QUERY); }
    #[test] fn event_entrants_query_is_valid() { assert_valid_query(EVENT_ENTRANTS_QUERY); }
    #[test] fn event_sets_query_is_valid() { assert_valid_query(EVENT_SETS_QUERY); }
    #[test] fn event_phases_query_is_valid() { assert_valid_query(EVENT_PHASES_QUERY); }
}
```

Note: `graphql-parser` validates query document syntax. Field-level validation against the schema (detecting unknown field names, wrong argument types) requires the `apollo-compiler` crate or similar. The implementation should evaluate whether `graphql-parser` alone is sufficient or whether a schema-aware validator is needed. At minimum, both the schema and each query must parse without error.

## Layer 2: Live full-pipeline e2e tests

### File location

`backend/crates/e2e/tests/import_live.rs` — alongside the existing wiremock-based e2e tests. Runs as part of `cargo test -p e2e`.

### DB

Uses `#[sqlx::test(migrations = "../../migrations")]` — the same pattern as all other e2e tests. Spins up an ephemeral Postgres per test, no persistent DB required.

### Skip gate

Every test function checks for the API key at the start of the body:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn import_hannover_weekly_live(pool: PgPool) {
    let Some(key) = std::env::var("STARTGG_API_KEY").ok() else {
        return; // skip silently — STARTGG_API_KEY not set
    };
    // ... rest of test using real StartggClient::new(key)
}
```

Returning early from `#[sqlx::test]` passes the test and cleans up the ephemeral DB. No `#[ignore]` tag needed.

### What each test exercises

Each test runs a complete import for a known past Smash Hannover Weekly tournament and asserts specific results in the DB. This exercises the full stack: `StartggClient` → real start.gg API → import worker logic → DB writes → assertions on DB state.

A single test covers all 6 operations implicitly: the import worker calls `tournaments_by_user` (or the all-games variant), `event_entrants`, `event_sets`, and `event_phases` for every event it processes.

### Golden dataset

A block of hardcoded constants at the top of the file:

```rust
// ── Golden dataset: Smash Hannover Weekly ────────────────────────────────────
// Source: 2–3 past completed Smash Hannover Weekly tournaments.
// Data is immutable (tournaments are finished).
// IDs and expected values were verified by running the import and
// inspecting the DB output during initial implementation.

const HANNOVER_USER_SLUG: &str = "user/...";   // a known Hannover scene player
const HANNOVER_GAME_ID: i64 = 1;               // Melee

const WEEKLY_1_SLUG: &str = "tournament/smash-hannover-weekly-XX";
const WEEKLY_1_EXPECTED_WINNER: &str = "PlayerHandle";
// ... etc for 2–3 tournaments
```

### Discovery flow (one-time, during implementation)

The golden dataset is populated during implementation without needing the app running:

1. Write a temporary discovery test that calls `StartggClient::new(key).tournaments_by_user(...)` for a known Hannover player slug, and prints tournament slugs, event IDs, and top set results.
2. Run it once with `STARTGG_API_KEY` set.
3. User inspects the output and confirms the data looks correct.
4. Promote 2–3 past completed weeklies to constants; add specific assertions (e.g. known winner of a specific set).
5. Delete the discovery helper.

### Assertions per test

Each per-tournament test asserts:
- The tournament record exists in the DB with the correct slug and name.
- The expected player handles are present in the `users` table.
- At least one event is imported with a non-zero entrant count.
- A specific known set result: correct winner, loser, and scores.

### Rate limits

Each full weekly import calls roughly 3–5 API operations per event, with 2–3 tournaments having 1–2 events each. Total: ~15–30 API calls per test run. Well within the 80 req/60s limit. No throttling logic needed in the tests.

## CI integration

Add `STARTGG_API_KEY` as a repository secret. The existing CI jobs run `cargo test -p common` and `cargo test -p e2e` — both pick up the new live tests automatically when the env var is present.

Local development without a key: all live tests self-skip, CI remains green.

## Out of scope

- Asserting every field we map — we assert a curated set of immutable facts, not exhaustive field coverage.
- Complexity retry behavior against the live API — covered by existing wiremock tests.
- Testing with a persistent DB — ephemeral `#[sqlx::test]` DB is sufficient.
