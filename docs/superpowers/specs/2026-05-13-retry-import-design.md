# Retry Failed Import

**Date:** 2026-05-13  
**Status:** Approved

## Problem

When an import job fails, the user must scroll to the date-filter form and click "Re-import" to try again. There is no quick one-click retry directly next to the failure message, and the original date filters are not preserved.

## Approach

Extend `JobResponse` with typed `after_date`/`before_date` fields so the frontend can round-trip the failed job's params. Add a "Retry" button that appears inline with the failed status badge and submits a pre-filled form to the existing import action.

No new API endpoint or SvelteKit form action is needed.

## Backend changes

**File:** `backend/crates/api/src/routes/import.rs`

Add two optional fields to `JobResponse`:

```rust
pub after_date: Option<NaiveDate>,
pub before_date: Option<NaiveDate>,
```

In `From<Job> for JobResponse`, decode the stored params using the existing `ImportParams::from_job()` helper, then convert each unix timestamp to `NaiveDate`:

```rust
let params = ImportParams::from_job(&j);
let to_date = |ts: i64| DateTime::from_timestamp(ts, 0).map(|dt| dt.date_naive());
JobResponse {
    // ...existing fields...
    after_date: params.after_date.and_then(to_date),
    before_date: params.before_date.and_then(to_date),
}
```

Serializes as `"YYYY-MM-DD"` strings (Rust `NaiveDate` default). Both fields are `null` when the import had no date filter.

**File:** `backend/openapi.yaml`

Add to the `ImportStatus` schema:

```yaml
after_date:
  type: string
  format: date
  nullable: true
  description: Lower bound of the import date filter (inclusive), if set.
before_date:
  type: string
  format: date
  nullable: true
  description: Upper bound of the import date filter (inclusive), if set.
```

## Frontend changes

**File:** `web/src/lib/types.ts`

Extend the `Job` interface:

```ts
after_date: string | null;   // "YYYY-MM-DD"
before_date: string | null;
```

**File:** `web/src/routes/projects/[id]/import/+page.svelte`

When `job.status === 'failed'`, render a "Retry" button inside the status card. The button lives in a small `<form method="POST">` with two hidden inputs pre-populated from the failed job's params. It submits to the existing `?/default` action, which already parses `after_date`/`before_date` and enqueues a new job.

On success the `enhance` callback updates `job` state and calls `startPolling()`, identical to the main form's success path.

The existing date-filter form and "Re-import" button at the bottom of the page are unchanged.

## What is not changing

- No new API endpoint.
- No new SvelteKit form action.
- No change to job storage or worker logic.
- The date-filter form remains as-is for users who want to change filters before retrying.

## Testing

- Existing API tests cover `JobResponse` serialization; add assertions for the new fields.
- Existing E2E test `full_flow` exercises the import pipeline; no retry-specific E2E needed since retry is just a re-enqueue.
- Manual: trigger a failed import (e.g. with a bad API key), verify the retry button appears and re-enqueues with the correct params.
