# Worker Graceful Shutdown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On SIGTERM or SIGINT, abort all in-flight import tasks and mark their jobs `failed` before the worker exits.

**Architecture:** A plain `Vec<(Uuid, JoinHandle<()>)>` in the main task tracks in-flight jobs. Signal handlers (SIGTERM and SIGINT) are added to the outer `tokio::select!`. On signal, all handles are aborted and a single bulk DB update marks the affected jobs `failed`. The main loop reaps finished handles before each claim iteration.

**Tech Stack:** Rust, Tokio (`tokio::signal::unix`), sqlx 0.8, PostgreSQL

---

## File Map

| File | Change |
|---|---|
| `backend/crates/common/src/jobs.rs` | Add `mark_shutdown(pool, ids)` function + tests |
| `backend/crates/worker/src/main.rs` | Add signal handling, in-flight registry, shutdown logic |

---

### Task 1: Add `mark_shutdown` to `common/jobs.rs`

**Files:**
- Modify: `backend/crates/common/src/jobs.rs`

- [ ] **Step 1: Add the failing tests**

  Append this block to the bottom of `backend/crates/common/src/jobs.rs`:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use sqlx::PgPool;

      async fn setup_project(pool: &PgPool) -> Uuid {
          let user_id: Uuid = sqlx::query_scalar!(
              "INSERT INTO users (username, password_hash) VALUES ('alice', 'hash') RETURNING id"
          )
          .fetch_one(pool)
          .await
          .unwrap();

          sqlx::query_scalar!(
              "INSERT INTO ranking_projects (user_id, name) VALUES ($1, 'Test') RETURNING id",
              user_id
          )
          .fetch_one(pool)
          .await
          .unwrap()
      }

      #[sqlx::test(migrations = "../../migrations")]
      async fn mark_shutdown_marks_running_jobs_failed(pool: PgPool) {
          let project_id = setup_project(&pool).await;
          let job = enqueue(&pool, project_id, ImportParams::default()).await.unwrap();
          claim(&pool).await.unwrap(); // moves job to 'running'

          mark_shutdown(&pool, &[job.id]).await.unwrap();

          let row = sqlx::query!(
              r#"SELECT status::text AS "status!", error FROM jobs WHERE id = $1"#,
              job.id
          )
          .fetch_one(&pool)
          .await
          .unwrap();
          assert_eq!(row.status, "failed");
          assert_eq!(row.error.as_deref(), Some("worker shutdown"));
      }

      #[sqlx::test(migrations = "../../migrations")]
      async fn mark_shutdown_with_no_ids_is_noop(pool: PgPool) {
          // Must not error on empty input
          mark_shutdown(&pool, &[]).await.unwrap();
      }
  }
  ```

- [ ] **Step 2: Run tests to confirm they fail**

  ```bash
  cd backend
  DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres cargo test -p common -- mark_shutdown 2>&1 | tail -5
  ```

  Expected: compile error — `cannot find function mark_shutdown in this scope`

  (If you don't have a running Postgres, run `bash backend/test.sh -- -p common mark_shutdown` instead — it spins up a container automatically.)

- [ ] **Step 3: Implement `mark_shutdown`**

  In `backend/crates/common/src/jobs.rs`, add this function after `mark_failed`:

  ```rust
  pub async fn mark_shutdown(pool: &PgPool, ids: &[Uuid]) -> Result<(), sqlx::Error> {
      if ids.is_empty() {
          return Ok(());
      }
      sqlx::query!(
          "UPDATE jobs SET status = 'failed', error = 'worker shutdown', updated_at = NOW() \
           WHERE id = ANY($1)",
          ids as &[Uuid],
      )
      .execute(pool)
      .await?;
      Ok(())
  }
  ```

- [ ] **Step 4: Run tests to confirm they pass**

  ```bash
  bash backend/test.sh -- -p common mark_shutdown
  ```

  Expected output contains:
  ```
  test jobs::tests::mark_shutdown_marks_running_jobs_failed ... ok
  test jobs::tests::mark_shutdown_with_no_ids_is_noop ... ok
  ```

- [ ] **Step 5: Update the sqlx offline query cache**

  ```bash
  bash backend/prepare-sqlx.sh
  ```

  This runs migrations against a fresh container, then `cargo sqlx prepare --workspace -- --all-targets`. The updated `.sqlx/` files must be committed.

- [ ] **Step 6: Commit**

  ```bash
  git add backend/crates/common/src/jobs.rs backend/.sqlx/
  git commit -m "feat(common): add mark_shutdown for bulk-failing in-flight jobs"
  ```

---

### Task 2: Wire signal handling into `worker/src/main.rs`

**Files:**
- Modify: `backend/crates/worker/src/main.rs`

- [ ] **Step 1: Replace the full contents of `main.rs`**

  The changes: add signal imports, declare `in_flight` vec, reap finished handles each iteration, capture `JoinHandle` on spawn, add signal branches to `tokio::select!`, add `shutdown` helper.

  Replace `backend/crates/worker/src/main.rs` with:

  ```rust
  use clap::Parser;
  use sqlx::postgres::PgListener;
  use std::time::Duration;
  use tokio::signal::unix::{signal, SignalKind};
  use tokio::task::JoinHandle;
  use uuid::Uuid;

  mod config;
  mod import;
  use config::Config;

  fn init_tracing(rust_log: &str) {
      tracing_subscriber::fmt()
          .with_env_filter(tracing_subscriber::EnvFilter::new(rust_log))
          .init();
  }

  async fn shutdown(pool: &sqlx::PgPool, in_flight: Vec<(Uuid, JoinHandle<()>)>) {
      let job_ids: Vec<Uuid> = in_flight.iter().map(|(id, _)| *id).collect();
      for (_, handle) in &in_flight {
          handle.abort();
      }
      if job_ids.is_empty() {
          tracing::info!("shutdown: no in-flight jobs");
          return;
      }
      tracing::info!(count = job_ids.len(), "shutdown: aborting in-flight imports");
      if let Err(e) = common::jobs::mark_shutdown(pool, &job_ids).await {
          tracing::error!(%e, "shutdown: failed to mark in-flight jobs as failed");
      }
  }

  #[tokio::main]
  async fn main() {
      dotenvy::dotenv().ok();
      let config = Config::parse();

      init_tracing(&config.rust_log);

      let pool = common::db::connect(&config.database_url)
          .await
          .expect("failed to connect to database");

      sqlx::migrate!("../../migrations")
          .run(&pool)
          .await
          .expect("failed to run migrations");

      let startgg = common::startgg::StartggClient::new(config.startgg_api_key.into());

      let mut listener = PgListener::connect(&config.database_url)
          .await
          .expect("failed to create PgListener");
      listener
          .listen("jobs")
          .await
          .expect("failed to listen on jobs channel");

      let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
      let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");

      tracing::info!("Worker ready, listening for import jobs");

      let mut in_flight: Vec<(Uuid, JoinHandle<()>)> = Vec::new();

      loop {
          // Reap handles for tasks that finished since the last iteration
          in_flight.retain(|(_, h)| !h.is_finished());

          // Drain all pending jobs before waiting
          loop {
              match common::jobs::claim(&pool).await {
                  Ok(Some(job)) => {
                      let pool2 = pool.clone();
                      let startgg2 = startgg.clone();
                      let project_id = job.project_id;
                      let job_id = job.id;
                      let import_params = common::jobs::ImportParams::from_job(&job);
                      tracing::info!(%job_id, %project_id, "starting import");
                      let handle = tokio::spawn(async move {
                          match import::run(&pool2, &startgg2, project_id, import_params).await {
                              Ok(()) => {
                                  tracing::info!(%job_id, "import complete");
                                  if let Err(e) = common::jobs::mark_done(&pool2, job_id).await {
                                      tracing::error!(%e, %job_id, "failed to mark job done");
                                  }
                              }
                              Err(e) => {
                                  tracing::error!(%e, %job_id, "import failed");
                                  if let Err(e2) =
                                      common::jobs::mark_failed(&pool2, job_id, &e.to_string()).await
                                  {
                                      tracing::error!(%e2, %job_id, "failed to mark job failed");
                                  }
                              }
                          }
                      });
                      in_flight.push((job_id, handle));
                  }
                  Ok(None) => break,
                  Err(e) => {
                      tracing::error!(%e, "error claiming job");
                      break;
                  }
              }
          }

          // Wait for a NOTIFY, poll every 30s, or shutdown signal
          tokio::select! {
              result = listener.recv() => {
                  match result {
                      Ok(_) => tracing::debug!("received job notification"),
                      Err(e) => tracing::error!(%e, "PgListener error"),
                  }
              }
              _ = tokio::time::sleep(Duration::from_secs(30)) => {
                  tracing::debug!("polling for jobs");
              }
              _ = sigterm.recv() => {
                  tracing::info!("received SIGTERM, shutting down");
                  shutdown(&pool, in_flight).await;
                  return;
              }
              _ = sigint.recv() => {
                  tracing::info!("received SIGINT, shutting down");
                  shutdown(&pool, in_flight).await;
                  return;
              }
          }
      }
  }
  ```

- [ ] **Step 2: Build to verify**

  ```bash
  cd backend && cargo build -p worker
  ```

  Expected: `Finished` with no errors or warnings.

- [ ] **Step 3: Run the full test suite**

  ```bash
  bash backend/test.sh
  ```

  Expected: all tests pass (the worker binary has no automated tests for signal plumbing — the build passing is the verification here).

- [ ] **Step 4: Commit**

  ```bash
  git add backend/crates/worker/src/main.rs
  git commit -m "feat(worker): graceful shutdown on SIGTERM/SIGINT"
  ```

---

### Task 3: Manual smoke test

- [ ] **Step 1: Start the stack**

  ```bash
  docker compose up -d db
  # In a separate terminal:
  cd backend && cargo run --bin worker
  ```

- [ ] **Step 2: Trigger an import via the API or directly**

  Enqueue a job via the API (POST `/projects/{id}/import`), or insert one directly:

  ```sql
  INSERT INTO jobs (kind, project_id, params, status)
  VALUES ('import_tournaments', '<project-uuid>', '{}', 'pending');
  SELECT pg_notify('jobs', '<job-uuid>');
  ```

- [ ] **Step 3: Send SIGTERM while the import is running**

  ```bash
  kill -TERM $(pgrep -f 'cargo.*worker\|target.*worker')
  ```

- [ ] **Step 4: Confirm job is marked failed**

  ```sql
  SELECT status, error FROM jobs ORDER BY created_at DESC LIMIT 1;
  ```

  Expected:
  ```
   status |     error
  --------+----------------
   failed | worker shutdown
  ```
