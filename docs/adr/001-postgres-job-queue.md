# ADR 001: Postgres NOTIFY/LISTEN for the Job Queue

## Context

The API needs to hand off import work to the worker process asynchronously. Options
considered: a dedicated queue service (Redis/Sidekiq, RabbitMQ, SQS) or leveraging
the existing Postgres database.

## Decision

Use a `jobs` table in Postgres. The API inserts a row and sends `NOTIFY jobs`. The
worker listens with `LISTEN jobs` (via sqlx `PgListener`) and claims work with
`SELECT ... FOR UPDATE SKIP LOCKED`.

## Rationale

- **No new infrastructure.** Postgres is already a hard dependency. A separate queue
  service adds a third container, credentials, and an operational surface to monitor.
- **Correct delivery semantics.** `SELECT ... FOR UPDATE SKIP LOCKED` gives at-most-once
  job delivery without a distributed lock.
- **Instant wakeup.** `NOTIFY/LISTEN` wakes the worker immediately — no polling.
- **Horizontal scaling.** Multiple worker containers each independently claim jobs;
  `SKIP LOCKED` prevents double-processing without any coordination layer.

## Consequences

- All job state is in Postgres — inspectable and queryable with standard SQL.
- Adding a new job type is a code change only; no queue configuration needed.
- If a worker crashes mid-job, the job stays in `running` state indefinitely — there is
  no automatic recovery. Graceful shutdown (SIGTERM/SIGINT) marks in-flight jobs `failed`
  so they can be retried on the next run.
- Throughput is bounded by Postgres NOTIFY rate, which is not a concern at this scale.
