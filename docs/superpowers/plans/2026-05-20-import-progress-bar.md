# Import Progress Bar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a two-phase progress bar (scanning players / importing tournaments) to the import page, backed by a new `progress JSONB` column on the `jobs` table updated by the worker.

**Architecture:** The worker writes structured progress JSON into the `jobs.progress` column at each loop iteration. The API deserializes this into a typed `ImportProgress` field on `JobResponse`. The frontend reads it on each 1-second poll and renders a labeled progress bar using the shadcn `Progress` component.

**Tech Stack:** Rust (sqlx, axum, serde_json), SvelteKit + TypeScript, shadcn-svelte, PostgreSQL

---

## File Map

| File | Change |
|------|--------|
| `backend/migrations/002_job_progress.sql` | Create: add `progress JSONB` column to `jobs` |
| `backend/crates/common/src/models/mod.rs` | Modify: add `progress` field to `Job` struct |
| `backend/crates/common/src/jobs.rs` | Modify: add `update_progress` function + test |
| `backend/.sqlx/` | Update: regenerate offline query cache |
| `backend/crates/api/src/routes/import.rs` | Modify: add `ImportProgress` struct, expose in `JobResponse` |
| `backend/crates/worker/src/import.rs` | Modify: add `job_id` param, call `update_progress` in both loops |
| `backend/crates/worker/src/main.rs` | Modify: pass `job_id` to `import::run` |
| `web/src/lib/types.ts` | Modify: add `progress` field to `Job` interface |
| `web/src/routes/projects/[id]/(editor)/import/+page.svelte` | Modify: progress bar UI + 1s polling |

---

## Task 1: Add migration

**Files:**
- Create: `backend/migrations/002_job_progress.sql`

- [ ] **Step 1: Create the migration file**

```sql
ALTER TABLE jobs ADD COLUMN progress JSONB;
```

Save to `backend/migrations/002_job_progress.sql`.

- [ ] **Step 2: Commit**

```bash
git add backend/migrations/002_job_progress.sql
git commit -m "feat: add progress column to jobs table"
```

---

## Task 2: Update the `Job` model

**Files:**
- Modify: `backend/crates/common/src/models/mod.rs:1-16`

The `Job` struct currently has no `progress` field. Add it. sqlx maps a nullable JSONB column to `Option<serde_json::Value>`.

- [ ] **Step 1: Add `progress` to the `Job` struct**

In `backend/crates/common/src/models/mod.rs`, change the `Job` struct from:

```rust
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub project_id: Uuid,
    pub params: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

to:

```rust
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub project_id: Uuid,
    pub params: serde_json::Value,
    pub result: Option<serde_json::Value>,
    pub progress: Option<serde_json::Value>,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Update all `sqlx::query_as!` macros in `jobs.rs` that SELECT from `jobs`**

In `backend/crates/common/src/jobs.rs`, every query that returns a `Job` must include `progress` in the SELECT list. There are three: `enqueue`, `latest_for_project`, and `claim`.

Change every `SELECT` in `jobs.rs` that returns `Job` columns. Current pattern:

```rust
r#"SELECT id, kind::text AS "kind!", project_id, params, result,
          status::text AS "status!", error, created_at, updated_at"#
```

New pattern (add `progress` after `result`):

```rust
r#"SELECT id, kind::text AS "kind!", project_id, params, result, progress,
          status::text AS "status!", error, created_at, updated_at"#
```

Apply this to all three queries: `enqueue` (RETURNING clause), `latest_for_project` (SELECT), and `claim` (SELECT).

- [ ] **Step 3: Commit**

```bash
git add backend/crates/common/src/models/mod.rs backend/crates/common/src/jobs.rs
git commit -m "feat: add progress field to Job model and queries"
```

---

## Task 3: Add `update_progress` to `jobs.rs`

**Files:**
- Modify: `backend/crates/common/src/jobs.rs`

- [ ] **Step 1: Write the failing test first**

Add to the `#[cfg(test)]` block at the bottom of `backend/crates/common/src/jobs.rs`:

```rust
#[sqlx::test(migrations = "../../migrations")]
async fn update_progress_stores_phase_step_total(pool: PgPool) {
    let project_id = setup_project(&pool).await;
    let job = enqueue(&pool, project_id, ImportParams::default())
        .await
        .unwrap();

    update_progress(&pool, job.id, "scanning", 2, 5).await.unwrap();

    let row = sqlx::query!(
        "SELECT progress FROM jobs WHERE id = $1",
        job.id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let progress = row.progress.unwrap();
    assert_eq!(progress["phase"], "scanning");
    assert_eq!(progress["step"], 2);
    assert_eq!(progress["total"], 5);
}

#[sqlx::test(migrations = "../../migrations")]
async fn update_progress_overwrites_previous(pool: PgPool) {
    let project_id = setup_project(&pool).await;
    let job = enqueue(&pool, project_id, ImportParams::default())
        .await
        .unwrap();

    update_progress(&pool, job.id, "scanning", 1, 3).await.unwrap();
    update_progress(&pool, job.id, "importing", 2, 7).await.unwrap();

    let row = sqlx::query!(
        "SELECT progress FROM jobs WHERE id = $1",
        job.id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let progress = row.progress.unwrap();
    assert_eq!(progress["phase"], "importing");
    assert_eq!(progress["step"], 2);
    assert_eq!(progress["total"], 7);
}
```

- [ ] **Step 2: Run to verify it fails**

```bash
cd backend && cargo test -p common -- update_progress 2>&1 | tail -20
```

Expected: compilation error — `update_progress` doesn't exist yet.

- [ ] **Step 3: Implement `update_progress`**

Add this function to `backend/crates/common/src/jobs.rs` (before the `#[cfg(test)]` block):

```rust
pub async fn update_progress(
    pool: &PgPool,
    id: Uuid,
    phase: &str,
    step: usize,
    total: usize,
) -> Result<(), sqlx::Error> {
    let progress = serde_json::json!({
        "phase": phase,
        "step": step,
        "total": total,
    });
    sqlx::query!(
        "UPDATE jobs SET progress = $2, updated_at = NOW() WHERE id = $1",
        id,
        progress,
    )
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 4: Update sqlx offline cache**

This step requires Docker (the script spins up an ephemeral Postgres container):

```bash
bash backend/prepare-sqlx.sh
```

Expected: exits 0, updates `.sqlx/` directory with new query hashes.

- [ ] **Step 5: Run the tests**

```bash
cd backend && cargo test -p common -- update_progress 2>&1 | tail -20
```

Expected: `test update_progress_stores_phase_step_total ... ok` and `test update_progress_overwrites_previous ... ok`

- [ ] **Step 6: Run the full common test suite to check for regressions**

```bash
cd backend && cargo test -p common 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/common/src/jobs.rs backend/.sqlx/
git commit -m "feat: add update_progress to jobs queue"
```

---

## Task 4: Expose `progress` in `JobResponse`

**Files:**
- Modify: `backend/crates/api/src/routes/import.rs`

- [ ] **Step 1: Add `ImportProgress` struct and `progress` field to `JobResponse`**

In `backend/crates/api/src/routes/import.rs`, change the top of the file. Current:

```rust
use serde::{Deserialize, Serialize};
```

No change needed to imports — `Deserialize` is already imported.

Add `ImportProgress` struct and update `JobResponse` — change from:

```rust
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
```

to:

```rust
#[derive(Serialize, Deserialize)]
pub struct ImportProgress {
    pub phase: String,
    pub step: u32,
    pub total: u32,
}

#[derive(Serialize)]
pub struct JobResponse {
    pub id: Uuid,
    pub status: String,
    pub error: Option<String>,
    pub after_date: Option<NaiveDate>,
    pub before_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub progress: Option<ImportProgress>,
}
```

- [ ] **Step 2: Populate `progress` in the `From<Job>` impl**

Change the `From<Job>` impl from:

```rust
impl From<Job> for JobResponse {
    fn from(j: Job) -> Self {
        let params = ImportParams::from_job(&j);
        let to_date = |ts: i64| Utc.timestamp_opt(ts, 0).single().map(|dt| dt.date_naive());
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

to:

```rust
impl From<Job> for JobResponse {
    fn from(j: Job) -> Self {
        let params = ImportParams::from_job(&j);
        let to_date = |ts: i64| Utc.timestamp_opt(ts, 0).single().map(|dt| dt.date_naive());
        JobResponse {
            id: j.id,
            status: j.status,
            error: j.error,
            after_date: params.after_date.and_then(to_date),
            before_date: params.before_date.and_then(to_date),
            created_at: j.created_at,
            updated_at: j.updated_at,
            progress: j.progress.and_then(|v| serde_json::from_value(v).ok()),
        }
    }
}
```

- [ ] **Step 3: Verify compilation**

```bash
cd backend && cargo build -p api 2>&1 | tail -10
```

Expected: builds with no errors.

- [ ] **Step 4: Run API tests**

```bash
bash backend/test.sh 2>&1 | tail -20
```

Expected: all tests pass. The new `progress` field is nullable so existing test assertions are unaffected.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/api/src/routes/import.rs
git commit -m "feat: expose progress field in JobResponse"
```

---

## Task 5: Update worker to emit progress

**Files:**
- Modify: `backend/crates/worker/src/import.rs`
- Modify: `backend/crates/worker/src/main.rs`

- [ ] **Step 1: Add `job_id` parameter to `import::run`**

In `backend/crates/worker/src/import.rs`, change the `run` signature from:

```rust
pub async fn run(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    params: ImportParams,
) -> anyhow::Result<()> {
```

to:

```rust
pub async fn run(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    job_id: Uuid,
    params: ImportParams,
) -> anyhow::Result<()> {
```

Add `use common::jobs::update_progress;` to the imports at the top of the file (alongside the existing `use common::jobs::ImportParams;`):

```rust
use common::jobs::{ImportParams, update_progress};
```

- [ ] **Step 2: Emit scanning progress in Phase 1**

In `import::run`, the Phase 1 loop currently reads:

```rust
for user_id in user_ids {
    collect_user_tournaments(
        startgg,
        user_id,
        game_id,
        params.after_date,
        params.before_date,
        &mut seen,
    )
    .await?;
}
```

Change it to:

```rust
let total_players = user_ids.len();
for (i, user_id) in user_ids.iter().enumerate() {
    collect_user_tournaments(
        startgg,
        *user_id,
        game_id,
        params.after_date,
        params.before_date,
        &mut seen,
    )
    .await?;
    update_progress(pool, job_id, "scanning", i + 1, total_players).await?;
}
```

- [ ] **Step 3: Emit importing progress in Phase 2**

In `import::run`, the Phase 2 loop currently reads:

```rust
for (_, tournament) in &seen {
    import_tournament(
        pool,
        startgg,
        project_id,
        tournament,
        game_id,
        project.game_name.as_deref(),
        &account_map,
    )
    .await?;
}
```

Change it to:

```rust
let total_tournaments = seen.len();
for (i, (_, tournament)) in seen.iter().enumerate() {
    import_tournament(
        pool,
        startgg,
        project_id,
        tournament,
        game_id,
        project.game_name.as_deref(),
        &account_map,
    )
    .await?;
    update_progress(pool, job_id, "importing", i + 1, total_tournaments).await?;
}
```

- [ ] **Step 4: Pass `job_id` from main.rs**

In `backend/crates/worker/src/main.rs`, the spawn block currently calls:

```rust
match import::run(&pool2, &startgg2, project_id, import_params).await {
```

Change it to:

```rust
match import::run(&pool2, &startgg2, project_id, job_id, import_params).await {
```

- [ ] **Step 5: Verify compilation**

```bash
cd backend && cargo build -p worker 2>&1 | tail -10
```

Expected: builds with no errors.

- [ ] **Step 6: Run full backend test suite**

```bash
bash backend/test.sh 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add backend/crates/worker/src/import.rs backend/crates/worker/src/main.rs
git commit -m "feat: emit scanning and importing progress from worker"
```

---

## Task 6: Install shadcn Progress component

**Files:**
- Creates files under `web/src/lib/components/ui/progress/`

- [ ] **Step 1: Install the component**

```bash
cd web && npx shadcn-svelte@latest add --yes --overwrite progress
```

Expected: files created under `web/src/lib/components/ui/progress/`.

- [ ] **Step 2: Verify the component exists**

```bash
ls web/src/lib/components/ui/progress/
```

Expected: `index.ts` and `progress.svelte` (or similar).

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/components/ui/progress/
git commit -m "feat: install shadcn Progress component"
```

---

## Task 7: Update frontend `Job` type

**Files:**
- Modify: `web/src/lib/types.ts:98-106`

- [ ] **Step 1: Add `progress` to the `Job` interface**

In `web/src/lib/types.ts`, change the `Job` interface from:

```typescript
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

to:

```typescript
export interface ImportProgress {
	phase: 'scanning' | 'importing';
	step: number;
	total: number;
}

export interface Job {
	id: string;
	status: 'pending' | 'running' | 'done' | 'failed';
	error: string | null;
	after_date: string | null;
	before_date: string | null;
	created_at: string;
	updated_at: string;
	progress: ImportProgress | null;
}
```

- [ ] **Step 2: Run unit tests to verify no type errors**

```bash
cd web && npm run test:unit 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/types.ts
git commit -m "feat: add progress type to Job interface"
```

---

## Task 8: Update import page UI

**Files:**
- Modify: `web/src/routes/projects/[id]/(editor)/import/+page.svelte`

- [ ] **Step 1: Update imports and polling interval in the `<script>` block**

In the `<script lang="ts">` block, add the `Progress` import alongside the existing imports:

```typescript
import { Progress } from '$lib/components/ui/progress';
import type { ImportProgress } from '$lib/types';
```

Change the polling interval from `3000` to `1000`:

```typescript
const interval = setInterval(async () => {
    const api = makeApi(fetch, env.PUBLIC_API_URL);
    const res = await api.get(`/projects/${data.project.id}/import`);
    if (res.ok) {
        job = await res.json() as Job;
    }
}, 1000);
```

- [ ] **Step 2: Add a derived label for the progress bar**

In the `<script>` block, add after the `isActiveJob` derived:

```typescript
const progressLabel = $derived((): string | null => {
    if (!job?.progress) return null;
    const { phase, step, total } = job.progress;
    return phase === 'scanning'
        ? `Scanning players (${step} / ${total})`
        : `Importing tournaments (${step} / ${total})`;
});

const progressPercent = $derived((): number => {
    if (!job?.progress || job.progress.total === 0) return 0;
    return (job.progress.step / job.progress.total) * 100;
});
```

- [ ] **Step 3: Add progress bar to the status card**

Inside the `{#if job}` block in the template, the `Card.Content` currently contains:

```svelte
<Card.Content class="p-4 space-y-2">
    <div class="flex items-center gap-2">
        <span class="text-sm font-medium">Status:</span>
        <Badge variant={statusColors[job.status]}>{job.status}</Badge>
        {#if isActiveJob}
            <span class="text-xs text-muted-foreground animate-pulse">updating…</span>
        {/if}
    </div>
    {#if job.error}
        <p class="text-sm text-destructive">{job.error}</p>
    {/if}
    <p class="text-xs text-muted-foreground">
        Started {formatDateTime(job.created_at)}
    </p>
    ...
</Card.Content>
```

Replace with:

```svelte
<Card.Content class="p-4 space-y-2">
    <div class="flex items-center gap-2">
        <span class="text-sm font-medium">Status:</span>
        <Badge variant={statusColors[job.status]}>{job.status}</Badge>
        {#if isActiveJob}
            <span class="text-xs text-muted-foreground animate-pulse">updating…</span>
        {/if}
    </div>
    {#if job.status === 'pending'}
        <p class="text-sm text-muted-foreground">Waiting to start…</p>
    {/if}
    {#if job.status === 'running' && job.progress}
        <div class="space-y-1">
            <p class="text-sm text-muted-foreground">{progressLabel}</p>
            <Progress value={progressPercent} class="h-2" />
        </div>
    {/if}
    {#if job.error}
        <p class="text-sm text-destructive">{job.error}</p>
    {/if}
    <p class="text-xs text-muted-foreground">
        Started {formatDateTime(job.created_at)}
    </p>
    {#if job.status === 'failed'}
        <form
            method="POST"
            use:enhance={() => {
                return ({ result }) => {
                    if (result.type === 'success' && result.data?.job) {
                        job = result.data.job as Job;
                    }
                };
            }}
        >
            <input type="hidden" name="after_date" value={job.after_date ?? ''} />
            <input type="hidden" name="before_date" value={job.before_date ?? ''} />
            <Button type="submit" variant="outline" size="sm">Retry</Button>
        </form>
    {/if}
</Card.Content>
```

- [ ] **Step 4: Run frontend unit tests**

```bash
cd web && npm run test:unit 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/routes/projects/[id]/\(editor\)/import/+page.svelte
git commit -m "feat: add progress bar to import page, poll every 1s"
```

---

## Task 9: Full test suite

- [ ] **Step 1: Run the complete test suite from the repo root**

```bash
bash test.sh 2>&1 | tail -30
```

Expected: `PASS` for all sections (backend, frontend unit, frontend e2e).

- [ ] **Step 2: If frontend e2e tests fail, check mock API**

The Playwright mock API lives in `web/tests/`. If it returns a hard-coded `Job` object without `progress`, add `progress: null` to those fixtures. The `Job` type now has `progress: ImportProgress | null`, so `null` is valid and existing mock responses remain correct without any other changes.
