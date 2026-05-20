# Import Progress Bar — Design Spec

**Date:** 2026-05-20
**Status:** Approved

## Overview

Add a progress bar to the import page so users can see how far along a running import is. The import has two phases — scanning players' tournaments on start.gg, then importing each tournament — and both phases will be surfaced in the UI.

## Approach

Store progress data in a new nullable `progress JSONB` column on the `jobs` table. The worker writes structured progress at each step. The API exposes it in `JobResponse`. The frontend renders it as a labeled progress bar, polling every 1 second while the job is active.

No SSE or WebSockets needed — polling at 1s is sufficient for this use case.

## Schema

New migration:

```sql
ALTER TABLE jobs ADD COLUMN progress JSONB;
```

Progress shape written by the worker:

```json
{ "phase": "scanning",  "step": 2, "total": 5  }
{ "phase": "importing", "step": 3, "total": 12 }
```

- `phase`: `"scanning"` during Phase 1 (player tournament discovery), `"importing"` during Phase 2 (tournament import)
- `step`: how many units have completed so far
- `total`: how many units there are in this phase
- Column is `NULL` when `status = 'pending'` (job not yet started by worker)
- Column retains final values after `done` or `failed` (audit trail, no cleanup needed)
- `result` column is left untouched for future use

## Backend

### `common/src/models/mod.rs`
Add `progress: Option<serde_json::Value>` to the `Job` struct.

### `common/src/jobs.rs`
Add:
```rust
pub async fn update_progress(
    pool: &PgPool,
    id: Uuid,
    phase: &str,
    step: usize,
    total: usize,
) -> Result<(), sqlx::Error>
```
Updates `progress = '{"phase":..., "step":..., "total":...}'` and `updated_at = NOW()`.

### `api/src/routes/import.rs`
Add `ImportProgress` struct:
```rust
#[derive(Serialize, Deserialize)]
pub struct ImportProgress {
    pub phase: String,
    pub step: u32,
    pub total: u32,
}
```

Add `progress: Option<ImportProgress>` to `JobResponse`. Deserialize it from `job.progress` in the `From<Job>` impl.

### `worker/src/import.rs`
- After processing each player in Phase 1: convert the loop to `for (i, user_id) in user_ids.iter().enumerate()` and call `update_progress(pool, job_id, "scanning", i+1, user_ids.len()).await?` after `collect_user_tournaments` returns.
- After importing each tournament in Phase 2: convert `for (_, tournament) in &seen` to `for (i, (_, tournament)) in seen.iter().enumerate()` and call `update_progress(pool, job_id, "importing", i+1, seen.len()).await?` after `import_tournament` returns.

The worker's `run()` function needs to accept `job_id: Uuid` as a new parameter (currently it takes `project_id` and `params`). The caller in `worker/src/main.rs` already has `job_id` available.

### sqlx offline cache
Run `bash backend/prepare-sqlx.sh` after adding the new query.

## Frontend

### `src/lib/types.ts`
Add to the `Job` type:
```ts
progress?: { phase: string; step: number; total: number };
```

### `src/routes/projects/[id]/(editor)/import/+page.svelte`
- Change polling interval from `3000` to `1000` ms.
- Install shadcn Progress component: `npx shadcn-svelte@latest add --yes --overwrite progress`
- When `job.status === 'running'` and `job.progress` is present, render:
  - A human-readable label: `"Scanning players (2 / 5)"` or `"Importing tournaments (3 / 12)"`
  - A `Progress` bar showing `(step / total) * 100`
- When `job.status === 'pending'`, show a `"Waiting to start…"` note in the status card.
- When `done` or `failed`, hide the progress bar; existing badge/error display takes over.

## Testing

- Unit tests for `update_progress` in `common/src/jobs.rs` (via `#[sqlx::test]`).
- Existing import route tests continue to pass; `JobResponse` gains a nullable field (non-breaking).
- No e2e test changes required — the mock API can return `progress: null` for existing tests.
