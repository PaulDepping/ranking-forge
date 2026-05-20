use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Job;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ImportParams {
    pub after_date: Option<i64>,
    pub before_date: Option<i64>,
}

impl ImportParams {
    pub fn from_job(job: &Job) -> Self {
        serde_json::from_value(job.params.clone()).unwrap_or_default()
    }
}

pub async fn enqueue(
    pool: &PgPool,
    project_id: Uuid,
    params: ImportParams,
) -> Result<Job, sqlx::Error> {
    let params_json = serde_json::to_value(&params).unwrap_or_default();
    let job = sqlx::query_as!(
        Job,
        r#"INSERT INTO jobs (kind, project_id, params, status)
           VALUES ('import_tournaments', $1, $2, 'pending')
           RETURNING id, kind::text AS "kind!", project_id, params, result, progress,
                     status::text AS "status!", error, created_at, updated_at"#,
        project_id,
        params_json,
    )
    .fetch_one(pool)
    .await?;

    sqlx::query!("SELECT pg_notify('jobs', $1)", job.id.to_string())
        .execute(pool)
        .await?;

    Ok(job)
}

pub async fn latest_for_project(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<Job>, sqlx::Error> {
    sqlx::query_as!(
        Job,
        r#"SELECT id, kind::text AS "kind!", project_id, params, result, progress,
                  status::text AS "status!", error, created_at, updated_at
           FROM jobs
           WHERE project_id = $1
           ORDER BY created_at DESC
           LIMIT 1"#,
        project_id,
    )
    .fetch_optional(pool)
    .await
}

pub async fn claim(pool: &PgPool) -> Result<Option<Job>, sqlx::Error> {
    let mut tx = pool.begin().await?;

    let job = sqlx::query_as!(
        Job,
        r#"SELECT id, kind::text AS "kind!", project_id, params, result, progress,
                  status::text AS "status!", error, created_at, updated_at
           FROM jobs
           WHERE status = 'pending'
           ORDER BY created_at
           LIMIT 1
           FOR UPDATE SKIP LOCKED"#,
    )
    .fetch_optional(&mut *tx)
    .await?;

    if let Some(ref job) = job {
        sqlx::query!(
            "UPDATE jobs SET status = 'running', updated_at = NOW() WHERE id = $1",
            job.id,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(job)
}

pub async fn mark_done(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE jobs SET status = 'done', updated_at = NOW() WHERE id = $1",
        id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_failed(pool: &PgPool, id: Uuid, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE jobs SET status = 'failed', error = $2, updated_at = NOW() WHERE id = $1",
        id,
        error,
    )
    .execute(pool)
    .await?;
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    async fn setup_project(pool: &PgPool) -> Uuid {
        let user_id: Uuid = sqlx::query_scalar!(
            "INSERT INTO users (email, display_name, password_hash) VALUES ('alice@test.com', 'Alice', 'hash') RETURNING id"
        )
        .fetch_one(pool)
        .await
        .unwrap();

        let project_id: Uuid = sqlx::query_scalar!(
            "INSERT INTO ranking_projects (owner_id, name) VALUES ($1, 'Test') RETURNING id",
            user_id,
        )
        .fetch_one(pool)
        .await
        .unwrap();

        project_id
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn mark_shutdown_marks_running_jobs_failed(pool: PgPool) {
        let project_id = setup_project(&pool).await;
        let job = enqueue(&pool, project_id, ImportParams::default())
            .await
            .unwrap();
        claim(&pool).await.unwrap();

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
        mark_shutdown(&pool, &[]).await.unwrap();
    }

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
}
