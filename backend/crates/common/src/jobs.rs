use sqlx::PgPool;
use uuid::Uuid;

use crate::models::Job;

pub async fn enqueue(pool: &PgPool, project_id: Uuid) -> Result<Job, sqlx::Error> {
    let job = sqlx::query_as!(
        Job,
        r#"INSERT INTO jobs (kind, project_id, status)
           VALUES ('import_tournaments', $1, 'pending')
           RETURNING id, kind::text AS "kind!", project_id,
                     status::text AS "status!", error, created_at, updated_at"#,
        project_id,
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
        r#"SELECT id, kind::text AS "kind!", project_id,
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
        r#"SELECT id, kind::text AS "kind!", project_id,
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
