# Design: Live Test Stats and H2H Assertions

**Date:** 2026-05-27

## Problem

The existing live tests (`import_hannover_weekly_100` and `import_hannover_weekly_88_and_84`) verify that tournament data is imported and stored, but stop short of asserting whether stats and H2H calculations are correct against real start.gg data. The two-player test only checks `total_sets > 0` — a loose signal that does not verify who won, upset factors, or H2H counts.

## Goal

Extend `import_hannover_weekly_88_and_84` to:

1. Filter the imported events down to exactly two Melee Singles events (one from #88, one from #84) using start.gg's immutable event IDs
2. Assert exact per-player stats: win/loss counts, opponent names, upset factors, and full tournament/event/round context
3. Assert exact H2H summary counts between the two golden players
4. Assert the H2H sets drilldown

## Approach: Discovery-first, then extend

Two-step implementation. The expected values are not known before running the import against the real API, so a temporary discovery test collects them first.

**Step 1 — Discovery:** Add `discover_hannover_stats`, run it once with `--nocapture` to read the full stats/H2H payloads and all event `startgg_id` values.

**Step 2 — Extend:** Hardcode the discovered values as constants, extend `import_hannover_weekly_88_and_84` with event filtering and precise assertions, delete the discovery function.

## New constants (filled after discovery run)

```rust
// start.gg's immutable integer IDs for the Melee Singles events to keep.
// Fill these in from the discover_hannover_stats output before step 2.
const KEEP_EVENT_STARTGG_ID_88: i64 = /* TBD */;
const KEEP_EVENT_STARTGG_ID_84: i64 = /* TBD */;
```

Per-player win/loss counts, upset factors, seedings, and H2H totals are also hardcoded from discovery output. They are not listed here because they are unknown until the discovery run.

## Discovery test (`discover_hannover_stats`)

New temporary function in `backend/crates/e2e/tests/import_live.rs`, gated by `#[cfg(feature = "live-tests")]`.

### Steps

1. Same setup as `import_hannover_weekly_88_and_84`: register a user, create a Melee project, add both players (King / Player2), link their start.gg accounts
2. Trigger the import with the same date window (2025-10-27 to 2025-12-10) and poll until done (same 120 s loop)
3. `GET /projects/{id}/tournaments` → `eprintln!` each tournament name and, for each of its events, the `startgg_id`, `name`, and `included` flag
4. `GET /projects/{id}/stats` → `eprintln!` the full JSON array
5. `GET /projects/{id}/head-to-head` → `eprintln!` the full JSON array
6. `GET /projects/{id}/head-to-head/{king_id}/{player2_id}/sets` → `eprintln!` the full JSON array
7. End with `panic!("discovery complete — review output above, fill in golden constants, extend import_hannover_weekly_88_and_84, then delete this function")`

The panic makes the test visibly fail rather than silently pass with no assertions, ensuring it is never accidentally left in.

### Run command

```
DATABASE_URL=<url> STARTGG_API_KEY=<key> SQLX_OFFLINE=true \
cargo test -p e2e --features live-tests -- discover_hannover_stats --nocapture
```

## Extended test (`import_hannover_weekly_88_and_84`)

The existing assertions (tournament names, handles) are unchanged. After them, three new blocks are added.

### Event filtering

Reuse the `tournaments` Value already fetched above. Walk every event in every tournament: for any event whose `startgg_id` is not in `[KEEP_EVENT_STARTGG_ID_88, KEEP_EVENT_STARTGG_ID_84]`, send:

```
PATCH /projects/{id}/events/{event_uuid}   {"included": false}
```

The existing `total_sets > 0` assertion is removed — it is fully subsumed by the precise assertions below.

### Stats assertions (`GET /projects/{id}/stats`)

For each of King and Player2:

- Assert `wins` array length (exact count from discovery)
- Assert `losses` array length (exact count from discovery)
- For each win/loss entry assert: `opponent_name`, `upset_factor`, `tournament_name`, `event_name`, `round_name`, `winner_seed`, `loser_seed`, `is_dq`

This mirrors the assertion style in `full_import_flow` (`full_flow.rs`).

### H2H summary (`GET /projects/{id}/head-to-head`)

Assert the King-vs-Player2 and Player2-vs-King entries for exact `wins` and `losses` integers. Values hardcoded unconditionally from discovery output — if they never met, both values are 0 and we assert 0.

### H2H sets drilldown (`GET /projects/{id}/head-to-head/{king_id}/{player2_id}/sets`)

Assert array length matches expected set count (from discovery). Spot-check one entry for `is_win`, `tournament_name`, `event_name`, `round_name`, `opponent_name`.

## Files changed

| File | Change |
|------|--------|
| `backend/crates/e2e/tests/import_live.rs` | Add `discover_hannover_stats` (step 1 commit); extend `import_hannover_weekly_88_and_84` + delete discovery function (step 2 commit) |

No new files, no schema changes, no new API surface.

## Error handling

No new error cases. Any API failure panics the test with the existing helper pattern. A failing import job fails the test at the poll loop with the job's error message — unchanged from today.
