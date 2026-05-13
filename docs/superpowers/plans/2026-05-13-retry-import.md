# Retry Failed Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a one-click "Retry" button to the import page that re-enqueues a failed import with its original date filters preserved.

**Architecture:** Extend `JobResponse` with typed `after_date`/`before_date` fields so the frontend receives the original params. The retry button is a separate `<form>` with hidden inputs pre-filled from the failed job; it submits to the existing `?/default` SvelteKit action — no new API endpoint or form action needed.

**Tech Stack:** Rust/Axum (backend), chrono 0.4, SvelteKit + TypeScript (frontend), Playwright (E2E tests), `@sqlx::test` for API integration tests.

---

## File Map

| File | Change |
|------|--------|
| `backend/crates/api/src/routes/import.rs` | Add `after_date`/`before_date` to `JobResponse` and its `From<Job>` impl |
| `backend/crates/api/tests/api.rs` | Add test asserting new fields are present |
| `backend/openapi.yaml` | Add `after_date`/`before_date` to `ImportStatus` schema |
| `web/src/lib/types.ts` | Add `after_date`/`before_date` to `Job` interface |
| `web/tests/mock-api.js` | Add failed-job mock and POST /import handler |
| `web/src/routes/projects/[id]/import/+page.svelte` | Add retry button inside status card |

---

### Task 1: Extend `JobResponse` with date params (backend)

**Files:**
- Modify: `backend/crates/api/src/routes/import.rs`
- Modify: `backend/crates/api/tests/api.rs`

- [ ] **Step 1: Write two failing tests** — one for an import with date params, one without.

  In `backend/crates/api/tests/api.rs`, add after the existing `import_enqueue_returns_202` test:

  ```rust
  #[sqlx::test(migrations = "../../migrations")]
  async fn import_response_includes_date_params(pool: PgPool) {
      let app = make_app(pool, "");
      let cookie = register(&app, "alice", "password123").await;
      let pid = create_project(&app, &cookie).await;

      let resp = post_json(
          &app,
          &format!("/projects/{pid}/import"),
          &cookie,
          json!({ "after_date": "2026-01-15", "before_date": "2026-03-31" }),
      )
      .await;
      assert_eq!(resp.status(), StatusCode::ACCEPTED);
      let body = read_json(resp).await;
      assert_eq!(body["after_date"], "2026-01-15");
      assert_eq!(body["before_date"], "2026-03-31");
  }

  #[sqlx::test(migrations = "../../migrations")]
  async fn import_response_date_params_null_when_unset(pool: PgPool) {
      let app = make_app(pool, "");
      let cookie = register(&app, "alice", "password123").await;
      let pid = create_project(&app, &cookie).await;

      let resp = post_json(&app, &format!("/projects/{pid}/import"), &cookie, json!({})).await;
      assert_eq!(resp.status(), StatusCode::ACCEPTED);
      let body = read_json(resp).await;
      assert!(body["after_date"].is_null());
      assert!(body["before_date"].is_null());
  }
  ```

- [ ] **Step 2: Run tests to confirm they fail**

  From `backend/`:
  ```bash
  DATABASE_URL=postgres://rankingforge:rankingforge@localhost:5432/rankingforge \
    cargo test -p api -- import_response_includes_date_params import_response_date_params_null_when_unset
  ```
  Expected: FAIL — `JobResponse` has no `after_date`/`before_date` fields.

- [ ] **Step 3: Extend `JobResponse` and update `From<Job>`**

  In `backend/crates/api/src/routes/import.rs`, replace the existing `JobResponse` struct and its `From<Job>` impl with:

  ```rust
  use chrono::{DateTime, NaiveDate, NaiveTime, TimeZone, Utc};
  // NaiveDate is already imported; DateTime and Utc are already imported.

  #[derive(Serialize)]
  pub struct JobResponse {
      pub id: Uuid,
      pub status: String,
      pub error: Option<String>,
      pub after_date: Option<NaiveDate>,
      pub before_date: Option<NaiveDate>,
      pub created_at: DateTime<Utc>,
      pub updated_at: DateTime<Utc>,
  }

  impl From<Job> for JobResponse {
      fn from(j: Job) -> Self {
          let params = ImportParams::from_job(&j);
          let to_date = |ts: i64| {
              Utc.timestamp_opt(ts, 0).single().map(|dt| dt.date_naive())
          };
          JobResponse {
              id: j.id,
              status: j.status,
              error: j.error,
              after_date: params.after_date.and_then(to_date),
              before_date: params.before_date.and_then(to_date),
              created_at: j.created_at,
              updated_at: j.updated_at,
          }
      }
  }
  ```

  Note: `Utc.timestamp_opt(ts, 0).single()` returns `Option<DateTime<Utc>>`. `TimeZone` is already imported via the existing `use chrono::{..., TimeZone, ...}`.

- [ ] **Step 4: Run tests to confirm they pass**

  ```bash
  DATABASE_URL=postgres://rankingforge:rankingforge@localhost:5432/rankingforge \
    cargo test -p api -- import_response_includes_date_params import_response_date_params_null_when_unset
  ```
  Expected: PASS (both tests).

- [ ] **Step 5: Run full API test suite to check for regressions**

  ```bash
  DATABASE_URL=postgres://rankingforge:rankingforge@localhost:5432/rankingforge \
    cargo test -p api
  ```
  Expected: all tests pass.

- [ ] **Step 6: Commit**

  ```bash
  git add backend/crates/api/src/routes/import.rs backend/crates/api/tests/api.rs
  git commit -m "feat(api): expose after_date/before_date in import job response"
  ```

---

### Task 2: Update OpenAPI spec

**Files:**
- Modify: `backend/openapi.yaml`

- [ ] **Step 1: Add the new fields to `ImportStatus`**

  Find the `ImportStatus` schema (around line 152). It currently reads:

  ```yaml
      ImportStatus:
        type: object
        required: [status]
        properties:
          status:
            type: string
            enum: [pending, running, done, failed]
          error:
            type: string
            nullable: true
          updated_at:
            type: string
            format: date-time
  ```

  Replace it with:

  ```yaml
      ImportStatus:
        type: object
        required: [status]
        properties:
          status:
            type: string
            enum: [pending, running, done, failed]
          error:
            type: string
            nullable: true
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
          updated_at:
            type: string
            format: date-time
  ```

- [ ] **Step 2: Commit**

  ```bash
  git add backend/openapi.yaml
  git commit -m "docs(openapi): add after_date/before_date to ImportStatus schema"
  ```

---

### Task 3: Update frontend Job type and mock, add Playwright test, implement retry button

**Files:**
- Modify: `web/src/lib/types.ts`
- Modify: `web/tests/mock-api.js`
- Modify: `web/tests/projects.test.ts`
- Modify: `web/src/routes/projects/[id]/import/+page.svelte`

- [ ] **Step 1: Add `after_date`/`before_date` to the `Job` interface**

  In `web/src/lib/types.ts`, replace the `Job` interface:

  ```ts
  export interface Job {
  	id: string;
  	status: 'pending' | 'running' | 'done' | 'failed';
  	error: string | null;
  	after_date: string | null;
  	before_date: string | null;
  	created_at: string;
  	updated_at: string;
  }
  ```

- [ ] **Step 2: Add failed-job mock data and POST handler to mock-api.js**

  In `web/tests/mock-api.js`, add these constants after `MOCK_PLAYERS`:

  ```js
  const MOCK_FAILED_JOB = {
  	id: 'job-1',
  	status: 'failed',
  	error: 'start.gg API error: rate limit exceeded',
  	after_date: '2026-01-01',
  	before_date: '2026-03-31',
  	created_at: '2026-05-01T10:00:00Z',
  	updated_at: '2026-05-01T10:01:00Z'
  };

  const MOCK_PENDING_JOB = {
  	id: 'job-2',
  	status: 'pending',
  	error: null,
  	after_date: '2026-01-01',
  	before_date: '2026-03-31',
  	created_at: '2026-05-01T10:05:00Z',
  	updated_at: '2026-05-01T10:05:00Z'
  };
  ```

  Then replace the existing import handler block:

  ```js
  // Before (GET only):
  const importMatch = path.match(/^\/projects\/([^/]+)\/import$/);
  if (importMatch && req.method === 'GET') {
      respond(res, 200, null);
      return;
  }
  ```

  With:

  ```js
  const importMatch = path.match(/^\/projects\/([^/]+)\/import$/);
  if (importMatch) {
      const projectId = importMatch[1];
      if (req.method === 'GET') {
          respond(res, 200, projectId === 'proj-failed' ? MOCK_FAILED_JOB : null);
          return;
      }
      if (req.method === 'POST') {
          respond(res, 202, MOCK_PENDING_JOB);
          return;
      }
  }
  ```

- [ ] **Step 3: Write the Playwright tests**

  In `web/tests/projects.test.ts`, add after the existing import test:

  ```ts
  test('import page shows retry button when last import failed', async ({ page }) => {
  	await page.goto('/projects/proj-failed/import');
  	await expect(page.getByRole('button', { name: 'Retry' })).toBeVisible();
  	// Main form submit button should not be "Retry" — it should say Re-import
  	await expect(page.getByRole('button', { name: 'Re-import' })).toBeVisible();
  });

  test('retry button re-enqueues import with same params', async ({ page }) => {
  	await page.goto('/projects/proj-failed/import');
  	await page.getByRole('button', { name: 'Retry' }).click();
  	await expect(page.getByText('pending')).toBeVisible();
  });
  ```

- [ ] **Step 4: Run tests to confirm they fail**

  From `web/`:
  ```bash
  npx playwright test --grep "retry"
  ```
  Expected: FAIL — no "Retry" button exists yet.

- [ ] **Step 5: Add the retry button to `+page.svelte`**

  In `web/src/routes/projects/[id]/import/+page.svelte`, find the `{#if job}` status card block:

  ```svelte
  {#if job}
      <div class="rounded-md border border-border p-4 space-y-2">
          <div class="flex items-center gap-2">
              <span class="text-sm font-medium">Status:</span>
              <Badge variant={statusColors[job.status]}>{job.status}</Badge>
              {#if polling}
                  <span class="text-xs text-muted-foreground animate-pulse">updating…</span>
              {/if}
          </div>
          {#if job.error}
              <p class="text-sm text-destructive">{job.error}</p>
          {/if}
          <p class="text-xs text-muted-foreground">
              Started {new Date(job.created_at).toLocaleString()}
          </p>
      </div>
  {/if}
  ```

  Replace it with:

  ```svelte
  {#if job}
      <div class="rounded-md border border-border p-4 space-y-2">
          <div class="flex items-center gap-2">
              <span class="text-sm font-medium">Status:</span>
              <Badge variant={statusColors[job.status]}>{job.status}</Badge>
              {#if polling}
                  <span class="text-xs text-muted-foreground animate-pulse">updating…</span>
              {/if}
          </div>
          {#if job.error}
              <p class="text-sm text-destructive">{job.error}</p>
          {/if}
          <p class="text-xs text-muted-foreground">
              Started {new Date(job.created_at).toLocaleString()}
          </p>
          {#if job.status === 'failed'}
              <form
                  method="POST"
                  use:enhance={() => {
                      return ({ result }) => {
                          if (result.type === 'success' && result.data?.job) {
                              job = result.data.job as Job;
                              startPolling();
                          }
                      };
                  }}
              >
                  <input type="hidden" name="after_date" value={job.after_date ?? ''} />
                  <input type="hidden" name="before_date" value={job.before_date ?? ''} />
                  <Button type="submit" variant="outline" size="sm">Retry</Button>
              </form>
          {/if}
      </div>
  {/if}
  ```

- [ ] **Step 6: Run Playwright tests to confirm they pass**

  From `web/`:
  ```bash
  npx playwright test --grep "retry"
  ```
  Expected: both tests PASS.

- [ ] **Step 7: Run full Playwright suite to check for regressions**

  ```bash
  npx playwright test
  ```
  Expected: all tests pass.

- [ ] **Step 8: Commit**

  ```bash
  git add web/src/lib/types.ts web/tests/mock-api.js web/tests/projects.test.ts \
          web/src/routes/projects/[id]/import/+page.svelte
  git commit -m "feat(web): add retry button for failed imports"
  ```
