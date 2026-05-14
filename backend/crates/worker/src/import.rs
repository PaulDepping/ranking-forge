use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use common::jobs::ImportParams;
use common::startgg::{EventNode, PhaseNode, StartggClient, StartggError, TournamentNode};

fn ts_to_dt(ts: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(ts, 0).unwrap_or_default()
}

#[instrument(skip(pool, startgg), fields(%project_id))]
pub async fn run(
    pool: &PgPool,
    startgg: &StartggClient,
    project_id: Uuid,
    params: ImportParams,
) -> anyhow::Result<()> {
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

    tracing::info!(player_count = user_ids.len(), "starting import");

    // Phase 1: discover all unique tournaments across all players
    let mut seen: HashMap<i64, TournamentNode> = HashMap::new();
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
    tracing::info!(
        unique_tournament_count = seen.len(),
        "collection complete, starting import"
    );

    // Phase 2: import each unique tournament exactly once
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

    Ok(())
}

#[instrument(skip(startgg, seen), fields(startgg_user_id = user_id, game_id))]
async fn collect_user_tournaments(
    startgg: &StartggClient,
    user_id: i64,
    game_id: i64,
    after_date: Option<i64>,
    before_date: Option<i64>,
    seen: &mut HashMap<i64, TournamentNode>,
) -> anyhow::Result<()> {
    let mut page = 1i32;
    let mut scanned = 0usize;
    let mut newly_added = 0usize;
    loop {
        let tournament_page = startgg
            .tournaments_by_user(user_id, game_id, page, 25)
            .await?;

        for tournament in tournament_page.nodes {
            let start_ts = tournament.start_at.unwrap_or(0);
            if let Some(before) = before_date {
                if start_ts > before {
                    continue;
                }
            }
            if let Some(after) = after_date {
                if start_ts < after {
                    continue;
                }
            }
            scanned += 1;
            seen.entry(tournament.id).or_insert_with(|| {
                newly_added += 1;
                tournament
            });
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
    }

    tracing::info!(scanned, newly_added, "user tournaments scanned");
    Ok(())
}

#[instrument(
    skip(pool, startgg, tournament, account_map),
    fields(
        %project_id,
        tournament_startgg_id = tournament.id,
        tournament_name = %tournament.name,
    )
)]
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
                venue_name, venue_address, timezone, online, num_attendees,
                lat, lng, state, start_at, end_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
           ON CONFLICT (startgg_id) DO UPDATE SET
               name          = EXCLUDED.name,
               num_attendees = EXCLUDED.num_attendees,
               lat           = EXCLUDED.lat,
               lng           = EXCLUDED.lng,
               state         = EXCLUDED.state,
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
        tournament.lat,
        tournament.lng,
        tournament.state,
        start_at,
        end_at,
    )
    .fetch_one(pool)
    .await?;

    let tournament_db_id: Uuid = row.id;
    let events = tournament.events.as_deref().unwrap_or(&[]);

    for event in events {
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

    tracing::info!(event_count = events.len(), "tournament imported");
    Ok(())
}

#[instrument(
    skip(pool, startgg, event, account_map),
    fields(
        %project_id,
        event_startgg_id = event.id,
        event_name = %event.name,
    )
)]
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
    let min_team_size = event.team_roster_size.as_ref().and_then(|r| r.min_players);
    let max_team_size = event.team_roster_size.as_ref().and_then(|r| r.max_players);

    let row = sqlx::query!(
        r#"INSERT INTO events
               (tournament_id, startgg_id, name, slug, state, is_online, event_type,
                min_team_size, max_team_size, game_id, game_name, num_entrants, start_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
           ON CONFLICT (startgg_id) DO UPDATE SET
               name          = EXCLUDED.name,
               slug          = EXCLUDED.slug,
               state         = EXCLUDED.state,
               is_online     = EXCLUDED.is_online,
               event_type    = EXCLUDED.event_type,
               min_team_size = EXCLUDED.min_team_size,
               max_team_size = EXCLUDED.max_team_size,
               num_entrants  = EXCLUDED.num_entrants,
               start_at      = EXCLUDED.start_at
           RETURNING id"#,
        tournament_db_id,
        event.id,
        event.name,
        event.slug,
        event.state,
        event.is_online,
        event.event_type,
        min_team_size,
        max_team_size,
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

    // Upsert phases and phase groups, building startgg_phase_group_id → UUID map
    let phase_group_map = upsert_phases(
        pool,
        event_db_id,
        event.phases.as_deref().unwrap_or(&[]),
    )
    .await?;

    // Import entrants, build startgg_entrant_id → DB uuid map for set resolution
    let entrant_map = import_entrants(pool, startgg, event_db_id, event.id, account_map).await?;
    let entrant_count = entrant_map.len();

    // Import sets
    let set_count =
        import_sets(pool, startgg, event_db_id, event.id, &entrant_map, &phase_group_map).await?;

    tracing::info!(entrant_count, set_count, "event imported");
    Ok(())
}

#[instrument(skip(pool, startgg, account_map), fields(event_startgg_id))]
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

        tracing::debug!(
            page,
            entrant_count = entrant_page.nodes.len(),
            "entrants page imported"
        );

        let total_pages = entrant_page
            .page_info
            .as_ref()
            .and_then(|p| p.total_pages)
            .unwrap_or(1);
        if page >= total_pages {
            break;
        }
        page += 1;
    }

    Ok(entrant_map)
}

#[instrument(skip(pool, phases), fields(event_db_id = %event_db_id, phase_count = phases.len()))]
async fn upsert_phases(
    pool: &PgPool,
    event_db_id: Uuid,
    phases: &[PhaseNode],
) -> anyhow::Result<HashMap<i64, Uuid>> {
    let mut phase_group_map: HashMap<i64, Uuid> = HashMap::new();

    for phase in phases {
        let phase_row = sqlx::query!(
            r#"INSERT INTO phases
                   (startgg_id, event_id, name, bracket_type, phase_order,
                    num_seeds, group_count, state, is_exhibition)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               ON CONFLICT (startgg_id) DO UPDATE SET
                   name         = EXCLUDED.name,
                   bracket_type = EXCLUDED.bracket_type,
                   phase_order  = EXCLUDED.phase_order,
                   state        = EXCLUDED.state
               RETURNING id"#,
            phase.id,
            event_db_id,
            phase.name,
            phase.bracket_type,
            phase.phase_order,
            phase.num_seeds,
            phase.group_count,
            phase.state,
            phase.is_exhibition,
        )
        .fetch_one(pool)
        .await?;

        let phase_db_id: Uuid = phase_row.id;

        for pg in phase
            .phase_groups
            .as_ref()
            .map(|p| p.nodes.as_slice())
            .unwrap_or(&[])
        {
            let first_round_time = pg.first_round_time.map(ts_to_dt);
            let start_at = pg.start_at.map(ts_to_dt);

            let pg_row = sqlx::query!(
                r#"INSERT INTO phase_groups
                       (startgg_id, phase_id, display_identifier, bracket_type, bracket_url,
                        num_rounds, start_at, first_round_time, state)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                   ON CONFLICT (startgg_id) DO UPDATE SET
                       display_identifier = EXCLUDED.display_identifier,
                       bracket_url        = EXCLUDED.bracket_url,
                       num_rounds         = EXCLUDED.num_rounds,
                       state              = EXCLUDED.state
                   RETURNING id"#,
                pg.id,
                phase_db_id,
                pg.display_identifier,
                pg.bracket_type,
                pg.bracket_url,
                pg.num_rounds,
                start_at,
                first_round_time,
                pg.state,
            )
            .fetch_one(pool)
            .await?;

            phase_group_map.insert(pg.id, pg_row.id);
        }
    }

    Ok(phase_group_map)
}

#[instrument(skip(pool, startgg, entrant_map, phase_group_map), fields(event_startgg_id))]
async fn import_sets(
    pool: &PgPool,
    startgg: &StartggClient,
    event_db_id: Uuid,
    event_startgg_id: i64,
    entrant_map: &HashMap<i64, Uuid>,
    phase_group_map: &HashMap<i64, Uuid>,
) -> anyhow::Result<usize> {
    let mut page = 1i32;
    let mut total_sets = 0usize;

    loop {
        let set_page = match startgg.event_sets(event_startgg_id, page, 25).await {
            Ok(p) => p,
            Err(StartggError::Decode(msg)) => {
                tracing::error!(event_startgg_id, page, "set page decode error: {msg}");
                break;
            }
            Err(e) => return Err(e.into()),
        };
        let mut page_sets = 0usize;

        for set in &set_page.nodes {
            // Skip bye sets (one slot is a placeholder, not a real match)
            if set.has_placeholder.unwrap_or(false) {
                continue;
            }

            let (Some(winner_sg_id), Some(loser_sg_id)) = (set.winner_id, set.loser_id()) else {
                continue; // in-progress or unresolvable
            };
            let (Some(&winner_uuid), Some(&loser_uuid)) = (
                entrant_map.get(&winner_sg_id),
                entrant_map.get(&loser_sg_id),
            ) else {
                tracing::warn!(set_id = set.id, "entrant not found for set, skipping");
                continue;
            };

            let phase_group_id: Option<Uuid> = set.phase_group.as_ref().and_then(|pg| {
                let uuid = phase_group_map.get(&pg.id).copied();
                if uuid.is_none() {
                    tracing::warn!(set_id = set.id, pg_id = pg.id, "phase_group not in map, storing NULL");
                }
                uuid
            });

            let (winner_score, loser_score) = set.scores();
            let completed_at = set.completed_at.map(ts_to_dt);

            sqlx::query!(
                r#"INSERT INTO sets
                       (event_id, phase_group_id, startgg_set_id,
                        winner_entrant_id, loser_entrant_id,
                        round, round_name, total_games,
                        winner_score, loser_score,
                        is_dq, has_placeholder, state, identifier,
                        vod_url, completed_at)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
                   ON CONFLICT (event_id, startgg_set_id) DO UPDATE SET
                       phase_group_id    = EXCLUDED.phase_group_id,
                       winner_entrant_id = EXCLUDED.winner_entrant_id,
                       loser_entrant_id  = EXCLUDED.loser_entrant_id,
                       round             = EXCLUDED.round,
                       round_name        = EXCLUDED.round_name,
                       total_games       = EXCLUDED.total_games,
                       winner_score      = EXCLUDED.winner_score,
                       loser_score       = EXCLUDED.loser_score,
                       is_dq             = EXCLUDED.is_dq,
                       has_placeholder   = EXCLUDED.has_placeholder,
                       state             = EXCLUDED.state,
                       identifier        = EXCLUDED.identifier,
                       vod_url           = EXCLUDED.vod_url,
                       completed_at      = EXCLUDED.completed_at"#,
                event_db_id,
                phase_group_id,
                set.id,
                winner_uuid,
                loser_uuid,
                set.round,
                set.full_round_text.as_deref(),
                set.total_games.map(|b| b as i16),
                winner_score,
                loser_score,
                set.is_dq(),
                set.has_placeholder.unwrap_or(false),
                set.state,
                set.identifier.as_deref(),
                set.vod_url.as_deref(),
                completed_at,
            )
            .execute(pool)
            .await?;

            page_sets += 1;
        }

        total_sets += page_sets;
        tracing::debug!(page, set_count = page_sets, "sets page imported");

        let total_pages = set_page
            .page_info
            .as_ref()
            .and_then(|p| p.total_pages)
            .unwrap_or(1);
        if page >= total_pages {
            break;
        }
        page += 1;
    }

    Ok(total_sets)
}
