# start.gg Query Complexity Handling

**Date:** 2026-05-14
**Status:** Approved

## Problem

start.gg enforces a maximum of 1000 objects per GraphQL request. The `TOURNAMENTS_BY_USER_QUERY` nests 25 tournaments × N events × N phases × 100 phase groups in a single request. When a page contains tournaments with dense phase data, the actual object count exceeds 1000 and the import job fails with a generic `GraphQL` error.

The other paginated queries (`event_entrants`, `event_sets`) are flat and safe at current page sizes, but could theoretically hit the limit under unusual data.

## Solution Overview

Three independent, composable changes:

1. **Error detection** — parse complexity errors in `gql()` and surface them as a distinct `StartggError::ComplexityTooHigh` variant.
2. **Query restructure** — remove `phases` from `TOURNAMENTS_BY_USER_QUERY`; add a new `event_phases` query called once per event during import.
3. **Complexity retry** — add a complexity error arm to each paginated loop in `import.rs` that halves `per_page` and restarts pagination from page 1.

## Architecture

### 1. Error layer (`common/src/startgg/mod.rs`)

Add a new variant to `StartggError`:

```rust
#[error("query complexity too high (limit: {limit}, actual: {actual})")]
ComplexityTooHigh { limit: u32, actual: u32 },
```

Add a `parse_complexity_error` helper that uses a compiled regex (via `OnceLock`) to extract `limit` and `actual` from the start.gg error message format:

```
Your query complexity is too high. A maximum of 1000 objects may be returned by each request. (actual: 1203)
```

Regex: `r"A maximum of (\d+) objects may be returned.*\(actual: (\d+)\)"`

In `gql()`, before falling through to the generic `GraphQL` error path, call `parse_complexity_error` on the joined error message. If it matches, return `ComplexityTooHigh` instead. All other errors continue to surface as `GraphQL`. The `gql()` retry logic (HTTP 429 backoff) is unchanged.

### 2. Query restructure (`common/src/startgg/operations.rs` + `queries.rs`)

**`TOURNAMENTS_BY_USER_QUERY`** — remove the `phases { ... }` block from the `events` selection. The query becomes a flat `tournaments → events` shape. Complexity at `perPage=25` drops from ~1200+ to ~100 in the worst case.

**New `EVENT_PHASES_QUERY`** — fetches phases and phase groups for a single event:

```graphql
query($eventId: ID!) {
    event(id: $eventId) {
        phases {
            id name bracketType phaseOrder
            numSeeds groupCount state isExhibition
            phaseGroups(query: { perPage: 100 }) {
                nodes {
                    id displayIdentifier bracketType bracketUrl
                    numRounds startAt firstRoundTime state
                }
            }
        }
    }
}
```

A single event with 4 phases × 100 phase groups = 400 objects — well under the limit.

**New `event_phases(event_id: i64) -> Result<Vec<PhaseNode>, StartggError>`** method on `StartggClient`.

**`EventNode`** — remove the `phases: Option<Vec<PhaseNode>>` field. `PhaseNode` and `PhaseGroupNode` types are unchanged.

Add `regex` as a dependency to the `common` crate.

### 3. Complexity retry (`worker/src/import.rs`)

Each paginated loop (`collect_user_tournaments`, `import_entrants`, `import_sets`) gains a complexity error arm. The structure uses a labeled outer loop for restart:

```rust
let mut per_page = 25i32;
'pages: loop {
    let mut page = 1i32;
    loop {
        let page_data = match startgg.some_query(id, page, per_page).await {
            Err(StartggError::ComplexityTooHigh { actual, limit }) if per_page > 1 => {
                tracing::warn!(per_page, actual, limit, "complexity too high, halving perPage");
                per_page /= 2;
                continue 'pages;
            }
            other => other?,
        };
        // existing processing unchanged
        let total_pages = page_data.page_info.as_ref().and_then(|p| p.total_pages).unwrap_or(1);
        if page >= total_pages { break 'pages; }
        page += 1;
    }
}
```

On complexity error: `per_page` is halved and pagination restarts from page 1. Since all DB writes use `ON CONFLICT ... DO UPDATE`, re-processing already-imported pages is safe. If `per_page == 1` and complexity still fires, the guard `if per_page > 1` does not match, the error falls through to `other?`, and the job fails with a clear `ComplexityTooHigh` error.

**`import_event`** — replace `event.phases.as_deref().unwrap_or(&[])` with a live fetch:

```rust
let phases = startgg.event_phases(event.id).await?;
let phase_group_map = upsert_phases(pool, event_db_id, &phases).await?;
```

## Data Flow

Before:
```
collect_user_tournaments()
  → tournaments_by_user(page, per_page)
    → returns: tournaments + events + phases + phaseGroups

import_event()
  → uses event.phases (fetched above)
```

After:
```
collect_user_tournaments()
  → tournaments_by_user(page, per_page)
    → returns: tournaments + events only

import_event()
  → event_phases(event_id)   ← new: one call per event
  → import_entrants()        ← complexity retry added
  → import_sets()            ← complexity retry added
```

## Testing

All new tests use the existing wiremock pattern in `common/src/startgg/mod.rs`:

1. **Complexity error parsed correctly** — mock returns the complexity GraphQL error message; assert `StartggError::ComplexityTooHigh { limit: 1000, actual: 1203 }`.
2. **Non-complexity GraphQL errors still surface as `GraphQL`** — assert a different error message does not match the complexity regex.
3. **`event_phases` returns phases** — mock returns a well-formed phases + phase groups response; assert fields deserialize correctly.
4. **`tournaments_by_user` deserializes without `phases` field** — assert the response is valid after removing `phases` from `EventNode`.

The retry + restart behaviour (halving `per_page`, restarting from page 1) is covered by the e2e test suite which exercises the full worker pipeline.

## Files Changed

| File | Change |
|---|---|
| `backend/crates/common/Cargo.toml` | Add `regex` dependency |
| `backend/crates/common/src/startgg/mod.rs` | Add `ComplexityTooHigh` variant, `parse_complexity_error`, tests |
| `backend/crates/common/src/startgg/queries.rs` | Remove `phases` from `EventNode`; add `EventPhasesData` response type |
| `backend/crates/common/src/startgg/operations.rs` | Trim `TOURNAMENTS_BY_USER_QUERY`; add `EVENT_PHASES_QUERY` + `event_phases()` |
| `backend/crates/worker/src/import.rs` | Add complexity retry arm to 3 loops; update `import_event` to call `event_phases` |
