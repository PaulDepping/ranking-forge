# Worker Transient HTTP Error Retry — Design Spec

**Date:** 2026-05-21
**Status:** Approved

## Problem

The worker aborts an entire import job on any `StartggError::Http` that isn't a 429. The specific triggering case was HTTP 520 (Cloudflare "Unknown Error"), a transient server-side blip that has nothing to do with the job's correctness. Because the existing retry only matches `TOO_MANY_REQUESTS`, 5xx errors propagate immediately to `import::run`, causing a full job failure.

## Scope

Expand `StartggClient` to retry transient 5xx HTTP errors with bounded backoff, while keeping the existing unlimited 429 retry unchanged. No changes outside `common/src/startgg/mod.rs`.

## Architecture

### Two-layer retry in `StartggClient::gql`

The current `gql` method is split into two cooperating layers:

**Layer 1 — `gql_once` (private method)**

Contains all existing logic:
- Build and send the HTTP request
- Call `error_for_status()`
- Parse response body as `GqlResponse`
- Check for GraphQL errors (complexity, general)
- Decode the typed data field

Wraps itself with the existing 429 retry:
- Builder: `ExponentialBuilder`, min 1s (`retry_min_delay`), max 60s, unlimited attempts, jitter
- Predicate: `status == TOO_MANY_REQUESTS`
- Notify: `tracing::debug!` (rate limiting is routine)

A 5xx error inside `gql_once` is not matched by the 429 predicate and surfaces immediately to the caller.

**Layer 2 — `gql` (public)**

Calls `gql_once` via `|| self.gql_once(query, &vars)` and wraps it with the new 5xx retry:
- Builder: `ExponentialBuilder`, min 2s, max 30s, max 5 attempts, jitter
- Predicate: `status.is_server_error()` (covers 500, 502, 503, 504, 520, etc.)
- Notify: `tracing::warn!` — 5xx retries are notable, not routine

**Why nested instead of a single predicate?**

A single builder can't have different `max_times` per error class. Nesting lets 429 remain unbounded while 5xx is capped at 5 attempts. The two layers are cleanly independent: a 429 that occurs during a 5xx retry attempt is fully handled by the inner layer before the outer one ever sees it.

### Retry parameter rationale

| Parameter | 429 (inner) | 5xx (outer) |
|---|---|---|
| min delay | 1s (`retry_min_delay`) | 2s |
| max delay | 60s | 30s |
| max attempts | unlimited | 5 |
| jitter | yes | yes |

5xx: ~1+2+4+8+16 ≈ 31s total max wait. Long enough for a Cloudflare blip to clear; short enough that a persistent outage doesn't hold a job open indefinitely.

### Error propagation after exhaustion

If all 5 outer attempts fail, `StartggError::Http` propagates unchanged to `import::run` → `main.rs`, which logs it and calls `mark_failed`. No changes to the error type or job-failure path.

## Files Changed

- `backend/crates/common/src/startgg/mod.rs` — split `gql` into `gql_once` + `gql`, add outer 5xx retry

## Files Not Changed

- `backend/crates/worker/src/import.rs` — no changes; error propagation is identical
- `backend/crates/worker/src/main.rs` — no changes
- `backend/crates/common/src/jobs.rs` — no changes

## Testing

Extend the existing wiremock-based tests in `startgg/mod.rs`:

- `server_error_once_then_succeeds` — mock returns 520 once, then 200; assert success
- `server_error_exhausts_retries` — mock always returns 503; assert `StartggError::Http` is returned (not a panic or hang)
- `rate_limited_during_server_error_retry` — mock returns 503, then 429, then 200; assert success and that both retry paths fired

No changes to `import.rs` or e2e tests — behavior at that layer is unchanged.
