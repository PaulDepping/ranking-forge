# Worker Graceful Shutdown Design

**Date:** 2026-05-14
**Status:** Approved

## Problem

When the worker process is stopped (SIGTERM from Docker or SIGINT from Ctrl+C) while an import job is in progress, the job remains in `running` status indefinitely. There is no signal handling and no cleanup path in the current worker.

## Goal

On SIGTERM or SIGINT, abort all in-flight import tasks and mark their jobs `failed` before the process exits. Cleanup must complete well within Docker's default 10-second SIGTERM → SIGKILL window.

## Out of Scope

Hard-crash / SIGKILL recovery (stale `running` jobs from a killed worker) is not addressed here. A heartbeat mechanism would be the right solution for that and can be added later.

## Design

### No schema changes

The existing `failed` status and `error` column are sufficient. Jobs interrupted by shutdown will be marked `failed` with `error = 'worker shutdown'`.

### Shutdown signal listener

A `tokio::select!` races `tokio::signal::ctrl_c()` (SIGINT) and `tokio::signal::unix::signal(SignalKind::terminate())` (SIGTERM). Either signal triggers the same shutdown path.

### In-flight job registry

A plain `Vec<(Uuid, JoinHandle<()>)>` local to the main task. Spawned tasks never touch it — only the main loop does. Before each job-claim iteration, the main loop reaps completed entries via `handle.is_finished()`. No `Arc<Mutex<>>` is needed since the registry is never accessed from a spawned task.

### Shutdown handler

On signal:

1. Set a shutdown flag so the main loop stops claiming new jobs.
2. Drain the registry, calling `.abort()` on every `JoinHandle`. Tokio cancels aborted tasks at the next `.await` point — effectively immediate.
3. Issue a single bulk update: `UPDATE jobs SET status = 'failed', error = 'worker shutdown', updated_at = NOW() WHERE id = ANY($1)` for the collected job IDs.
4. Exit.

If the bulk update itself fails (e.g., DB is unreachable), log the error and exit anyway — we cannot recover further.

### Normal completion path unchanged

The existing `mark_done` / `mark_failed` calls inside each spawned task are unaffected. Any task that finishes before the signal arrives completes normally; the main loop reaps its handle from the registry on the next iteration via `is_finished()`.

### Main loop adjustment

The main event loop checks the shutdown flag before claiming the next job. Once set, it breaks out of the inner drain loop and exits without waiting for the PgListener.

## Testing

Signal plumbing is not practical to unit-test directly. The behavior should be verified by:

- A manual smoke test: start the worker, trigger an import, send SIGTERM, confirm the job is `failed` in the DB.
- Optionally an e2e integration test that sends SIGTERM to the worker process mid-import and asserts job status.
