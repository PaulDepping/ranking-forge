use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use common::startgg::{EventNode, StartggClient, TournamentNode};

fn ts_to_dt(ts: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(ts, 0).unwrap_or_default()
}

pub async fn run(pool: &PgPool, startgg: &StartggClient, project_id: Uuid) -> anyhow::Result<()> {
    let project = sqlx::query!(
        "SELECT game_id, game_name FROM ranking_projects WHERE id = $1",
        project_id,
    )
    .fetch_one(pool)
    .await?;

    let Some(game_id) = project.game_id else {
        tracing::warn!(%project_id, "project has no game_id set, skipping import");
        return Ok(());
    };

    // Build startgg_user_id → player_id map for this project
    let account_rows = sqlx::query!(
        "SELECT sa.startgg_user_id, sa.player_id
         FROM startgg_accounts sa
         JOIN players p ON p.id = sa.player_id
         WHERE p.project_id = $1",
        project_id,
    )
    .fetch_all(pool)
    .await?;

    let account_map: HashMap<i64, Uuid> = account_rows
        .into_iter()
        .map(|r| (r.startgg_user_id, r.player_id))
        .collect();

    let user_ids: Vec<i64> = account_map.keys().copied().collect();
    if user_ids.is_empty() {
        tracing::info!(%project_id, "no start.gg accounts linked, nothing to import");
        return Ok(());
    }

    for user_id in user_ids {
        import_user_tournaments(
            pool,
            startgg,
            project_id,
            user_id,
            game_id,
            project.game_name.as_deref(),
            &account_map,
        )
        .await?;
    }

    Ok(())
}

async fn import_user_tournaments(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    user_id: i64,
    game_id: i64,
    game_name: Option<&str>,
    account_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<()> {
    let mut page = 1i32;
    loop {
        let tournament_page = startgg
            .tournaments_by_user(user_id, game_id, page, 25)
            .await?;

        for tournament in &tournament_page.nodes {
            import_tournament(
                pool,
                startgg,
                project_id,
                tournament,
                game_id,
                game_name,
                account_map,
            )
            .await?;
        }

        let total_pages = tournament_page
            .page_info
            .as_ref()
            .and_then(|p| p.total_pages)
            .unwrap_or(1);
        if page >= total_pages {
            break;
        }
        page += 1;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(())
}

async fn import_tournament(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    tournament: &TournamentNode,
    game_id: i64,
    game_name: Option<&str>,
    account_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<()> {
    let start_at = tournament.start_at.map(ts_to_dt);
    let end_at = tournament.end_at.map(ts_to_dt);

    let row = sqlx::query!(
        r#"INSERT INTO tournaments
               (startgg_id, name, slug, city, addr_state, country_code,
                venue_name, venue_address, timezone, online, num_attendees, start_at, end_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
           ON CONFLICT (startgg_id) DO UPDATE SET
               name          = EXCLUDED.name,
               num_attendees = EXCLUDED.num_attendees,
               start_at      = EXCLUDED.start_at,
               end_at        = EXCLUDED.end_at
           RETURNING id"#,
        tournament.id,
        tournament.name,
        tournament.slug,
        tournament.city,
        tournament.addr_state,
        tournament.country_code,
        tournament.venue_name,
        tournament.venue_address,
        tournament.timezone,
        tournament.is_online.unwrap_or(false),
        tournament.num_attendees,
        start_at,
        end_at,
    )
    .fetch_one(pool)
    .await?;

    let tournament_db_id: Uuid = row.id;

    for event in tournament.events.as_deref().unwrap_or(&[]) {
        import_event(
            pool,
            startgg,
            project_id,
            tournament_db_id,
            event,
            game_id,
            game_name,
            account_map,
        )
        .await?;
    }

    Ok(())
}

async fn import_event(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    tournament_db_id: Uuid,
    event: &EventNode,
    game_id: i64,
    game_name: Option<&str>,
    account_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<()> {
    let start_at = event.start_at.map(ts_to_dt);

    let row = sqlx::query!(
        r#"INSERT INTO events (tournament_id, startgg_id, name, game_id, game_name, num_entrants, start_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           ON CONFLICT (startgg_id) DO UPDATE SET
               name         = EXCLUDED.name,
               num_entrants = EXCLUDED.num_entrants,
               start_at     = EXCLUDED.start_at
           RETURNING id"#,
        tournament_db_id,
        event.id,
        event.name,
        game_id,
        game_name,
        event.num_entrants,
        start_at,
    )
    .fetch_one(pool)
    .await?;

    let event_db_id: Uuid = row.id;

    // Register event in this project (included by default, don't overwrite existing choice)
    sqlx::query!(
        "INSERT INTO project_events (project_id, event_id, included)
         VALUES ($1, $2, TRUE)
         ON CONFLICT (project_id, event_id) DO NOTHING",
        project_id,
        event_db_id,
    )
    .execute(pool)
    .await?;

    // Import entrants, build startgg_entrant_id → DB uuid map for set resolution
    let entrant_map = import_entrants(pool, startgg, event_db_id, event.id, account_map).await?;

    // Import sets
    import_sets(pool, startgg, event_db_id, event.id, &entrant_map).await?;

    Ok(())
}

async fn import_entrants(
    pool: &PgPool,
    startgg: &StartggClient,
    event_db_id: Uuid,
    event_startgg_id: i64,
    account_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<HashMap<i64, Uuid>> {
    let mut entrant_map: HashMap<i64, Uuid> = HashMap::new();
    let mut page = 1i32;

    loop {
        let entrant_page = startgg.event_entrants(event_startgg_id, page, 25).await?;

        for entrant in &entrant_page.nodes {
            let player_id: Option<Uuid> = entrant
                .startgg_user_id()
                .and_then(|uid| account_map.get(&uid))
                .copied();

            let row = sqlx::query!(
                r#"INSERT INTO entrants
                       (event_id, player_id, startgg_entrant_id, startgg_user_id,
                        seed, display_name, is_disqualified, final_placement)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                   ON CONFLICT (event_id, startgg_entrant_id) DO UPDATE SET
                       player_id       = COALESCE(EXCLUDED.player_id, entrants.player_id),
                       seed            = EXCLUDED.seed,
                       display_name    = EXCLUDED.display_name,
                       is_disqualified = EXCLUDED.is_disqualified,
                       final_placement = EXCLUDED.final_placement
                   RETURNING id"#,
                event_db_id,
                player_id,
                entrant.id,
                entrant.startgg_user_id(),
                entrant.initial_seed_num,
                entrant.display_name(),
                entrant.is_disqualified.unwrap_or(false),
                entrant.standing.as_ref().and_then(|s| s.placement),
            )
            .fetch_one(pool)
            .await?;

            entrant_map.insert(entrant.id, row.id);
        }

        let total_pages = entrant_page
            .page_info
            .as_ref()
            .and_then(|p| p.total_pages)
            .unwrap_or(1);
        if page >= total_pages {
            break;
        }
        page += 1;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(entrant_map)
}

async fn import_sets(
    pool: &PgPool,
    startgg: &StartggClient,
    event_db_id: Uuid,
    event_startgg_id: i64,
    entrant_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<()> {
    let mut page = 1i32;

    loop {
        let set_page = startgg.event_sets(event_startgg_id, page, 25).await?;

        for set in &set_page.nodes {
            let (Some(winner_sg_id), Some(loser_sg_id)) = (set.winner_id, set.loser_id()) else {
                continue; // in-progress or bye
            };
            let (Some(&winner_uuid), Some(&loser_uuid)) = (
                entrant_map.get(&winner_sg_id),
                entrant_map.get(&loser_sg_id),
            ) else {
                tracing::warn!(set_id = set.id, "entrant not found for set, skipping");
                continue;
            };

            let (winner_score, loser_score) = set.scores();
            let completed_at = set.completed_at.map(ts_to_dt);

            sqlx::query!(
                r#"INSERT INTO sets
                       (event_id, startgg_set_id, winner_entrant_id, loser_entrant_id,
                        round, round_name, best_of, winner_score, loser_score, is_dq, vod_url, completed_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                   ON CONFLICT (event_id, startgg_set_id) DO UPDATE SET
                       winner_entrant_id = EXCLUDED.winner_entrant_id,
                       loser_entrant_id  = EXCLUDED.loser_entrant_id,
                       round             = EXCLUDED.round,
                       round_name        = EXCLUDED.round_name,
                       best_of           = EXCLUDED.best_of,
                       winner_score      = EXCLUDED.winner_score,
                       loser_score       = EXCLUDED.loser_score,
                       is_dq             = EXCLUDED.is_dq,
                       vod_url           = EXCLUDED.vod_url,
                       completed_at      = EXCLUDED.completed_at"#,
                event_db_id,
                set.id,
                winner_uuid,
                loser_uuid,
                set.round,
                set.full_round_text.as_deref(),
                set.best_of.map(|b| b as i16),
                winner_score,
                loser_score,
                set.is_dq(),
                set.vod_url.as_deref(),
                completed_at,
            )
            .execute(pool)
            .await?;
        }

        let total_pages = set_page
            .page_info
            .as_ref()
            .and_then(|p| p.total_pages)
            .unwrap_or(1);
        if page >= total_pages {
            break;
        }
        page += 1;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Ok(())
}
