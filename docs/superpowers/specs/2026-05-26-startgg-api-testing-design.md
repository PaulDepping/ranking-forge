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

### Layer 2: Live golden-dataset tests

Integration tests that call the real start.gg API using a hardcoded past tournament dataset from the Smash Hannover Weekly local scene. Gated on `STARTGG_API_KEY` being set: silently pass when absent, run fully when present.

## Architecture

No new crate. Both layers live in `common`.

```
backend/crates/common/
  src/
    startgg/
      schema.graphql          ← moved here from docs/startgg/
      operations/
        tests.rs              ← new: offline schema validation tests
  tests/
    startgg_live.rs           ← new: live golden-dataset integration tests
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
        // graphql-parser validates syntax; structural field validation against the schema
        // requires the schema document to be used for validation.
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

Note: `graphql-parser` validates query document syntax. Field-level validation against the schema (detecting unknown field names, wrong argument types) requires the `apollo-compiler` crate or similar. The implementation should evaluate whether `graphql-parser` alone is sufficient or whether a schema-aware validator is needed. At minimum, both the schema and each query must parse without error — this already catches typos in field names if the parser enforces schema conformance, or serves as a syntax-only guard if it does not.

## Layer 2: Live golden-dataset tests

### File location

`backend/crates/common/tests/startgg_live.rs` — a standard Rust integration test file. Runs as part of `cargo test -p common`.

### Skip gate

Every test function begins:

```rust
let Some(key) = std::env::var("STARTGG_API_KEY").ok() else {
    eprintln!("STARTGG_API_KEY not set — skipping live test");
    return;
};
let client = StartggClient::new(key);
```

Silently passes (exit 0) when the key is absent. No `#[ignore]` tag — these are normal tests that self-skip.

### Golden dataset

A block of hardcoded constants at the top of the file, clearly marked:

```rust
// ── Golden dataset: Smash Hannover Weekly ────────────────────────────────────
// Source: past completed tournaments from the Hannover Melee scene.
// Data is immutable (tournaments are finished). Verify against the app's
// import results before committing these values.

const TOURNAMENT_1_SLUG: &str = "tournament/smash-hannover-weekly-XX";
const TOURNAMENT_1_EVENT_ID: i64 = /* fill in */;
// ... etc for 2–3 tournaments
```

The dataset is populated during implementation by:
1. Running a real import of 2–3 past Smash Hannover Weeklies via the app.
2. User verifies the imported results are correct.
3. Tournament slugs, event IDs, and 2–3 known set results (winner handle, loser handle, scores) are hardcoded as constants.

### Tests per operation covered

| Test | Operation | Asserts |
|---|---|---|
| `user_by_slug_returns_known_user` | `user_by_slug` | Correct numeric start.gg user ID for a known scene player slug |
| `tournaments_by_user_includes_known_weekly` | `tournaments_by_user` | A known tournament slug appears in results for the right user+game |
| `event_entrants_returns_known_players` | `event_entrants` | Known player handles appear in entrant list for a past event |
| `event_sets_returns_known_result` | `event_sets` | A specific set has the correct winner ID, loser ID, and scores |
| `event_phases_returns_phases` | `event_phases` | At least one phase is returned with a non-empty phase group list |
| `search_games_finds_melee` | `search_games` | Melee appears in results with a stable non-zero ID |

All assertions use immutable facts from past completed events.

### Rate limits

6 operations × 2–3 tournaments ≈ 20–30 API calls total. Well within the 80 req/60s limit. No throttling logic needed in the tests.

## CI integration

Add `STARTGG_API_KEY` as a repository secret. No new workflow file needed — `cargo test -p common` already runs integration tests. The existing CI job gains the env var and the live suite runs automatically.

Local development without a key: all live tests self-skip, CI remains green.

## Out of scope

- Testing the worker's import logic end-to-end against the live API (covered by existing e2e tests with wiremock).
- Asserting all fields we map — we assert a curated set of immutable facts, not exhaustive field coverage.
- Complexity retry behavior against the live API — covered by existing wiremock tests.
