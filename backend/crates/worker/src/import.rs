use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use common::jobs::{ImportParams, update_progress};

#[instrument(skip(pool), fields(%project_id))]
pub async fn run(
    pool: &PgPool,
    project_id: Uuid,
    job_id: Uuid,
    params: ImportParams,
) -> anyhow::Result<()> {
    let project = sqlx::query!("SELECT game_id FROM projects WHERE id = $1", project_id,)
        .fetch_one(pool)
        .await?;

    // Resolve project players → global_players.id via startgg_accounts
    let account_rows = sqlx::query!(
        r#"
        SELECT gp.id AS global_player_id
        FROM startgg_accounts sa
        JOIN players p ON p.id = sa.player_id
        JOIN global_players gp ON gp.startgg_user_id = sa.startgg_user_id
        WHERE p.project_id = $1
        "#,
        project_id,
    )
    .fetch_all(pool)
    .await?;

    if account_rows.is_empty() {
        tracing::info!(%project_id, "no linked accounts with global player entries, nothing to import");
        return Ok(());
    }

    let global_player_ids: Vec<Uuid> = account_rows
        .into_iter()
        .map(|r| r.global_player_id)
        .collect();

    // Discover events via global_event_entries
    let event_rows = sqlx::query!(
        r#"
        SELECT DISTINCT ge.id AS global_event_id
        FROM global_event_entries gee
        JOIN global_events ge ON ge.id = gee.event_id
        JOIN global_tournaments gt ON gt.id = ge.tournament_id
        LEFT JOIN global_games gg ON gg.id = ge.game_id
        WHERE gee.player_id = ANY($1)
          AND ($2::BIGINT IS NULL OR gg.startgg_id = $2)
          AND ($3::BIGINT IS NULL OR EXTRACT(EPOCH FROM gt.start_at) >= $3)
          AND ($4::BIGINT IS NULL OR EXTRACT(EPOCH FROM gt.start_at) <= $4)
        "#,
        &global_player_ids,
        project.game_id,
        params.after_date.map(|t| t as i64),
        params.before_date.map(|t| t as i64),
    )
    .fetch_all(pool)
    .await?;

    tracing::info!(
        event_count = event_rows.len(),
        "discovered events from global mirror"
    );
    update_progress(pool, job_id, "importing", 0, event_rows.len()).await?;

    // Check whether this is the first import for the project
    let is_first_import = sqlx::query_scalar!(
        "SELECT NOT EXISTS (SELECT 1 FROM ranking_events re JOIN rankings r ON r.id = re.ranking_id WHERE r.project_id = $1)",
        project_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(true);

    let mut tx = pool.begin().await?;

    for (i, row) in event_rows.iter().enumerate() {
        // Upsert into project_events
        sqlx::query!(
            "INSERT INTO project_events (project_id, global_event_id)
             VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
            project_id,
            row.global_event_id,
        )
        .execute(&mut *tx)
        .await?;

        // Upsert into ranking_events for every ranking in this project
        sqlx::query!(
            r#"
            INSERT INTO ranking_events (ranking_id, global_event_id, included)
            SELECT r.id, $2, true
            FROM rankings r
            WHERE r.project_id = $1
            ON CONFLICT DO NOTHING
            "#,
            project_id,
            row.global_event_id,
        )
        .execute(&mut *tx)
        .await?;

        update_progress(pool, job_id, "importing", i + 1, event_rows.len()).await?;
    }

    tx.commit().await?;

    if is_first_import && !event_rows.is_empty() {
        seed_ranking_by_winrate(pool, project_id).await?;
        tracing::info!(%project_id, "initial ranking seeded by winrate");
    }

    // Enqueue compute_ranking for all project rankings
    let ranking_ids: Vec<Uuid> =
        sqlx::query_scalar!("SELECT id FROM rankings WHERE project_id = $1", project_id,)
            .fetch_all(pool)
            .await?;

    for rid in ranking_ids {
        if let Err(e) = common::jobs::enqueue_compute_ranking(pool, project_id, rid).await {
            tracing::warn!(%e, %rid, "failed to enqueue compute_ranking after import");
        }
    }

    Ok(())
}

async fn seed_ranking_by_winrate(pool: &PgPool, project_id: Uuid) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        WITH player_global AS (
            SELECT DISTINCT ON (p.id)
                p.id          AS player_id,
                p.created_at,
                gp.id         AS global_player_id
            FROM players p
            JOIN startgg_accounts sa ON sa.player_id = p.id
            JOIN global_players gp   ON gp.startgg_user_id = sa.startgg_user_id
            WHERE p.project_id = $1
            ORDER BY p.id, sa.created_at
        ),
        no_link AS (
            SELECT p.id AS player_id, p.created_at, NULL::UUID AS global_player_id
            FROM players p
            WHERE p.project_id = $1
              AND NOT EXISTS (SELECT 1 FROM startgg_accounts sa WHERE sa.player_id = p.id)
        ),
        all_players AS (SELECT * FROM player_global UNION ALL SELECT * FROM no_link),
        stats AS (
            SELECT
                ap.player_id,
                ap.created_at,
                COUNT(gs.id) FILTER (WHERE gs.winner_player_id = ap.global_player_id) AS wins,
                COUNT(gs.id) AS total
            FROM all_players ap
            LEFT JOIN global_sets gs
                ON ap.global_player_id IS NOT NULL
               AND (gs.winner_player_id = ap.global_player_id OR gs.loser_player_id = ap.global_player_id)
            GROUP BY ap.player_id, ap.created_at
        ),
        ranked AS (
            SELECT
                player_id,
                ROW_NUMBER() OVER (
                    ORDER BY
                        CASE WHEN total > 0 THEN wins::float8 / total ELSE 0 END DESC,
                        created_at ASC
                ) AS new_rank
            FROM stats
        )
        UPDATE ranking_players
        SET rank_position = ranked.new_rank::int4
        FROM ranked
        WHERE ranking_players.player_id = ranked.player_id
          AND ranking_players.ranking_id IN (SELECT id FROM rankings WHERE project_id = $1)
        "#,
        project_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}
