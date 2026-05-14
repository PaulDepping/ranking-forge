# Tournament Import Deduplication

**Date:** 2026-05-14  
**Status:** Approved

## Problem

The worker iterates every player in a project and, for each player, fetches all their tournaments from start.gg. For every tournament found it calls `import_event` → `import_entrants` + `import_sets`. When multiple players attended the same tournament, `event_entrants` and `event_sets` are called once per player — all redundant after the first. The DB upserts (ON CONFLICT) silently discard the duplicates, but the API calls and their pagination overhead are fully wasted. start.gg enforces an 80 req/60s rate limit, so this duplication directly reduces the project sizes and date ranges that can be imported within quota.

## Scope

One file: `backend/crates/worker/src/import.rs`. No schema changes, no new GraphQL queries, no changes to `common`.

Parallelism (importing multiple tournaments concurrently) is explicitly out of scope for this change. The two-phase structure chosen here is a natural foundation for adding a parallel executor in Phase 2 later — a `JoinSet` or `FuturesUnordered` with a rate-limiting semaphore can wrap the per-tournament import loop without restructuring the dedup design.

## Design

### Two-Phase Collect-Then-Import

**Phase 1 — Collect unique tournaments**

Iterate all players as before, calling `tournaments_by_user` (paginated). Instead of immediately importing each tournament, insert it into a `HashMap<i64, TournamentNode>` keyed by `tournament.startgg_id`. Use `entry().or_insert()` so the first occurrence wins and subsequent duplicates are discarded. Date-window filtering (`after_date` early-exit, `before_date` skip) applies during collection exactly as it does today.

**Phase 2 — Import each unique tournament once**

Iterate the collected map and call `import_tournament` for each entry. The functions `import_tournament`, `import_event`, `import_entrants`, and `import_sets` are unchanged — they already take `&PgPool` and `&StartggClient`, so they are structurally ready for concurrent execution later.

### Function changes

- `import_user_tournaments` is renamed to `collect_user_tournaments`. It accepts a `&mut HashMap<i64, TournamentNode>` to accumulate into (or returns one and the caller merges — implementation detail). It no longer calls `import_tournament`.
- `run` gains a `seen: HashMap<i64, TournamentNode>` local, loops players into `collect_user_tournaments`, then loops the map calling `import_tournament`.

### Data flow (before vs. after)

Before:
```
for each player:
  for each tournament page:
    for each tournament:
      import_tournament → import_event → import_entrants + import_sets
```

After:
```
Phase 1:
  seen: HashMap<i64, TournamentNode>
  for each player:
    for each tournament page:
      for each tournament in date window:
        seen.entry(id).or_insert(tournament)

Phase 2:
  for each (_, tournament) in seen:
    import_tournament → import_event → import_entrants + import_sets
```

## Edge Cases

| Case | Behavior |
|------|----------|
| Two players attended the same tournament | First player's `TournamentNode` is kept; second is discarded. Metadata is identical so this is correct. |
| No players linked | Early return before Phase 1 — unchanged. |
| Phase 1 yields zero unique tournaments | Phase 2 iterates an empty map — no-op. |
| `after_date` early-exit | Applied per-player during Phase 1 (start.gg returns newest-first, so breaking is still correct). |
| `before_date` skip | Applied per-player during Phase 1 — unchanged. |
| API error during Phase 1 or 2 | `?` propagates the error and the job fails — same behavior as today. |
| Rate-limit sleeps | 200ms sleep between `tournaments_by_user` pages stays in Phase 1. Sleeps within `import_entrants` / `import_sets` are unchanged in Phase 2. |

## Testing

The existing e2e test (`backend/crates/e2e/tests/full_flow.rs`) uses wiremock stubs and `#[sqlx::test]`. The test change required:

- Set up two players who both attended the same mocked tournament.
- Run the import.
- Assert that the wiremock stub for `event_entrants` was hit **exactly once** (not twice), and likewise for `event_sets`.

WireMock's `verify` supports exact call-count assertions. No new test infrastructure is needed.
