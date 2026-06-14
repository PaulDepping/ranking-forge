use sqlx::PgPool;
use uuid::Uuid;

use common::algorithms::{AlgorithmRegistry, ScoredSet};
use common::upset::set_upset_factor;

pub async fn run(pool: &PgPool, ranking_id: Uuid) -> anyhow::Result<()> {
    let ranking = sqlx::query!(
        r#"SELECT algorithm, algorithm_config, include_external_results
           FROM rankings WHERE id = $1"#,
        ranking_id,
    )
    .fetch_optional(pool)
    .await?;

    let Some(ranking) = ranking else {
        anyhow::bail!("ranking {ranking_id} not found");
    };

    phase1_set_results(pool, ranking_id).await?;

    if let Some(ref algo_name) = ranking.algorithm {
        phase2_algorithm_scores(pool, ranking_id, algo_name, &ranking.algorithm_config).await?;

        seed_rank_position_from_scores(pool, ranking_id).await?;
    }

    Ok(())
}

async fn phase1_set_results(pool: &PgPool, ranking_id: Uuid) -> anyhow::Result<()> {
    struct SetRow {
        set_id: Uuid,
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        event_id: Uuid,
        winner_seed: Option<i32>,
        loser_seed: Option<i32>,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let sets = sqlx::query_as!(
        SetRow,
        r#"
        SELECT
            s.id            AS set_id,
            we.player_id    AS "winner_player_id!: Uuid",
            le.player_id    AS "loser_player_id!: Uuid",
            s.event_id,
            we.seed         AS winner_seed,
            le.seed         AS loser_seed,
            s.completed_at
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN ranking_players rwp ON rwp.player_id = we.player_id AND rwp.ranking_id = $1
        JOIN ranking_players rlp ON rlp.player_id = le.player_id AND rlp.ranking_id = $1
        JOIN ranking_events re ON re.event_id = s.event_id AND re.ranking_id = $1
        WHERE re.included       = true
          AND s.is_dq           = false
          AND s.has_placeholder = false
          AND we.player_id IS NOT NULL
          AND le.player_id IS NOT NULL
        ORDER BY s.completed_at ASC NULLS LAST
        "#,
        ranking_id,
    )
    .fetch_all(pool)
    .await?;

    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM ranking_set_results WHERE ranking_id = $1",
        ranking_id,
    )
    .execute(&mut *tx)
    .await?;

    for row in &sets {
        let upset_factor = match (row.winner_seed, row.loser_seed) {
            (Some(ws), Some(ls)) => Some(set_upset_factor(ws, ls) as f64),
            _ => None,
        };

        sqlx::query!(
            r#"
            INSERT INTO ranking_set_results
                (ranking_id, set_id, winner_player_id, loser_player_id, event_id, upset_factor, completed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            ranking_id,
            row.set_id,
            row.winner_player_id,
            row.loser_player_id,
            row.event_id,
            upset_factor,
            row.completed_at,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    tracing::info!(%ranking_id, count = sets.len(), "phase1: wrote ranking_set_results");
    Ok(())
}

async fn phase2_algorithm_scores(
    pool: &PgPool,
    ranking_id: Uuid,
    algo_name: &str,
    config: &serde_json::Value,
) -> anyhow::Result<()> {
    let registry = AlgorithmRegistry::new();
    let algo = registry
        .get(algo_name)
        .ok_or_else(|| anyhow::anyhow!("unknown algorithm: {}", algo_name))?;

    struct SetRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let rows = sqlx::query_as!(
        SetRow,
        r#"
        SELECT
            we.player_id AS "winner_player_id!: Uuid",
            le.player_id AS "loser_player_id!: Uuid",
            s.completed_at
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN ranking_players rwp ON rwp.player_id = we.player_id AND rwp.ranking_id = $1
        JOIN ranking_players rlp ON rlp.player_id = le.player_id AND rlp.ranking_id = $1
        JOIN ranking_events re ON re.event_id = s.event_id AND re.ranking_id = $1
        WHERE re.included       = true
          AND s.is_dq           = false
          AND s.has_placeholder = false
          AND we.player_id IS NOT NULL
          AND le.player_id IS NOT NULL
        ORDER BY s.completed_at ASC NULLS LAST
        "#,
        ranking_id,
    )
    .fetch_all(pool)
    .await?;

    let scored_sets: Vec<ScoredSet> = rows
        .into_iter()
        .map(|r| ScoredSet {
            winner_id: r.winner_player_id,
            loser_id: r.loser_player_id,
            completed_at: r.completed_at.unwrap_or_default(),
            winner_global_rating: None,
            loser_global_rating: None,
            is_external_winner: false,
            is_external_loser: false,
        })
        .collect();

    let scores = algo
        .compute(config, &scored_sets)
        .map_err(|e| anyhow::anyhow!("algorithm error: {e}"))?;

    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM ranking_player_scores WHERE ranking_id = $1",
        ranking_id,
    )
    .execute(&mut *tx)
    .await?;

    for score in &scores {
        sqlx::query!(
            r#"
            INSERT INTO ranking_player_scores
                (ranking_id, player_id, computed_rating, display_data, algorithm_state)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            ranking_id,
            score.player_id,
            score.computed_rating,
            score.display_data,
            score.algorithm_state,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    tracing::info!(%ranking_id, count = scores.len(), "phase2: wrote ranking_player_scores");
    Ok(())
}

async fn seed_rank_position_from_scores(pool: &PgPool, ranking_id: Uuid) -> anyhow::Result<()> {
    // Only seed if all rank_positions are still 0 (first compute only)
    let all_zero: bool = sqlx::query_scalar!(
        "SELECT NOT EXISTS (SELECT 1 FROM ranking_players WHERE ranking_id = $1 AND rank_position != 0)",
        ranking_id,
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(true);

    if !all_zero {
        return Ok(());
    }

    // Assign rank_position = ROW_NUMBER ordered by computed_rating DESC
    sqlx::query!(
        r#"
        UPDATE ranking_players rp
        SET rank_position = ranked.new_rank::int4
        FROM (
            SELECT player_id,
                   ROW_NUMBER() OVER (ORDER BY computed_rating DESC NULLS LAST) AS new_rank
            FROM ranking_player_scores
            WHERE ranking_id = $1
        ) ranked
        WHERE rp.player_id = ranked.player_id
          AND rp.ranking_id = $1
        "#,
        ranking_id,
    )
    .execute(pool)
    .await?;

    tracing::info!(%ranking_id, "algorithmic ranking: seeded rank_position from computed_rating");
    Ok(())
}
