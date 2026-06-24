use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde_json::json;
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::api::{
    ComplexityError, EVENT_STANDINGS_QUERY, PHASE_GROUP_GAMES_QUERY, PHASE_GROUP_SETS_QUERY,
    PHASE_GROUP_SETS_QUERY_SLIM, PHASE_GROUPS_QUERY, TOURNAMENT_QUERY, gql_query,
};
use crate::api_types::{
    EventPhaseGroupsData, EventStandingsData, FullPhaseGroupSetsData, GamesPhaseGroupSetsData,
    SlimPhaseGroupSetsData, TournamentsData,
};
use crate::cli::Config;
use crate::db::{
    get_checkpoint, is_checkpointed, set_checkpoint, upsert_event, upsert_event_entry, upsert_game,
    upsert_game_selection, upsert_phase, upsert_phase_group, upsert_player_full,
    upsert_player_slim, upsert_set, upsert_set_game, upsert_tournament,
};

const TOURNAMENTS_PER_PAGE: u32 = 20;
const STANDINGS_PER_PAGE: u32 = 100;

// ---------------------------------------------------------------------------
// Pure helpers — tested below
// ---------------------------------------------------------------------------

pub fn is_dq(state: Option<i64>, display_score: Option<&str>) -> bool {
    if state == Some(7) {
        return true;
    }
    display_score
        .map(|s| s.to_uppercase().contains("DQ"))
        .unwrap_or(false)
}

pub fn extract_scores(display_score: &str) -> Option<(i16, i16)> {
    let parts: Vec<&str> = display_score.splitn(2, " - ").collect();
    if parts.len() != 2 {
        return None;
    }
    // displayScore may be "EntrantTag score - EntrantTag score"
    let a: i16 = parts[0].split_whitespace().last()?.parse().ok()?;
    let b: i16 = parts[1].split_whitespace().last()?.parse().ok()?;
    if a >= b { Some((a, b)) } else { Some((b, a)) }
}

fn ts_to_dt(ts: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(ts, 0).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Complexity-halving macro — mirrors the HCI scraper pattern
// ---------------------------------------------------------------------------

macro_rules! with_complexity_retry {
    ($per_page:expr, $min:expr, $block:expr) => {{
        let mut per_page = $per_page;
        loop {
            match $block(per_page).await {
                Ok(v) => break Ok(v),
                Err(e) => {
                    if e.downcast_ref::<ComplexityError>().is_some() && per_page > $min {
                        per_page = (per_page / 2).max($min);
                        tracing::debug!(per_page, "complexity error — halving perPage");
                        continue;
                    }
                    break Err(e);
                }
            }
        }
    }};
}

// ---------------------------------------------------------------------------
// Main run loop
// ---------------------------------------------------------------------------

pub async fn run(config: &Config, pool: &PgPool, shutdown: &AtomicBool) -> Result<()> {
    let client = Client::builder().timeout(Duration::from_secs(30)).build()?;
    let base_url = config
        .startgg_base_url
        .as_deref()
        .unwrap_or(crate::api::STARTGG_API_URL)
        .to_string();
    let delay = Duration::from_millis(config.delay_ms);

    let range_start = config
        .from_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let window_secs = config.window_days as i64 * 86400;

    // Resume from checkpoint if available
    let mut window_start = if let Some(cp) = get_checkpoint(pool, "window_start").await? {
        cp.as_i64().unwrap_or(range_start)
    } else {
        range_start
    };

    let mut base_filter = json!({});
    if let Some(gid) = config.game_id {
        base_filter["videogameIds"] = json!([gid]);
    }

    loop {
        let range_end = config
            .to_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp()
            .min(Utc::now().timestamp());

        if window_start >= range_end {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }
            // Exit if we've completed a historical backfill (to_date is in the past).
            // Only sleep and poll when running in live mode (to_date is today or future).
            let today = Utc::now().date_naive();
            if config.to_date < today {
                break;
            }
            info!("fully caught up, sleeping 1 hour before polling for new tournaments");
            tokio::time::sleep(Duration::from_secs(3600)).await;
            continue;
        }

        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        let window_end = (window_start + window_secs).min(range_end);
        let mut filter = base_filter.clone();
        filter["afterDate"] = json!(window_start);
        filter["beforeDate"] = json!(window_end);

        info!(
            window_start = %ts_to_dt(window_start),
            window_end   = %ts_to_dt(window_end),
            "starting date window"
        );

        let mut tournament_page = 1u32;
        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            let data: TournamentsData = gql_query(
                &client,
                &base_url,
                &config.startgg_api_key,
                TOURNAMENT_QUERY,
                json!({ "page": tournament_page, "perPage": TOURNAMENTS_PER_PAGE, "filter": filter }),
                delay,
            )
            .await?;

            let total_pages = data.tournaments.page_info.total_pages.unwrap_or(1);

            for t_node in &data.tournaments.nodes {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }

                let t_key = format!("tournament:{}", t_node.id);
                let cp = get_checkpoint(pool, &t_key).await?;
                if is_checkpointed(&cp) {
                    continue;
                }

                let tournament_id = upsert_tournament(pool, t_node).await?;

                for e_node in &t_node.events {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }

                    let e_key = format!("event:{}", e_node.id);
                    let e_cp = get_checkpoint(pool, &e_key).await?;
                    if is_checkpointed(&e_cp) {
                        continue;
                    }

                    let game_id = if let Some(vg) = &e_node.videogame {
                        Some(upsert_game(pool, vg.id, &vg.name).await?)
                    } else {
                        None
                    };

                    let event_id = upsert_event(pool, e_node, tournament_id, game_id).await?;

                    process_event(
                        &client,
                        &base_url,
                        pool,
                        &config.startgg_api_key,
                        e_node.id,
                        event_id,
                        config.sets_per_page,
                        delay,
                        shutdown,
                    )
                    .await?;

                    set_checkpoint(pool, &e_key, json!(true)).await?;
                    tokio::time::sleep(delay).await;
                }

                set_checkpoint(pool, &t_key, json!(true)).await?;
            }

            if tournament_page >= total_pages as u32 {
                break;
            }
            tournament_page += 1;
            tokio::time::sleep(delay).await;
        }

        // Advance window and persist checkpoint
        window_start = window_end;
        set_checkpoint(pool, "window_start", json!(window_start)).await?;
    } // end outer loop

    info!("crawler shutting down");
    Ok(())
}

// ---------------------------------------------------------------------------
// Per-event processing
// ---------------------------------------------------------------------------

async fn process_event(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    event_startgg_id: i64,
    event_id: Uuid,
    sets_per_page: u32,
    delay: Duration,
    shutdown: &AtomicBool,
) -> Result<()> {
    // Fetch all phase group IDs for this event
    let pg_data: EventPhaseGroupsData = gql_query(
        client,
        base_url,
        token,
        PHASE_GROUPS_QUERY,
        json!({ "eventId": event_startgg_id }),
        delay,
    )
    .await?;

    let phases = pg_data.event.map(|e| e.phases).unwrap_or_default();

    // Accumulate entrant → player UUID across all phase groups for standings resolution
    let mut event_entrant_map: HashMap<i64, Uuid> = HashMap::new();

    for phase in &phases {
        if shutdown.load(Ordering::SeqCst) {
            return Ok(());
        }
        for pg_id_node in &phase.phase_groups.nodes {
            if shutdown.load(Ordering::SeqCst) {
                return Ok(());
            }
            process_phase_group(
                client,
                base_url,
                pool,
                token,
                pg_id_node.id,
                phase.id,
                event_id,
                sets_per_page,
                delay,
                &mut event_entrant_map,
            )
            .await?;
            tokio::time::sleep(delay).await;
        }
    }

    // Fetch standings for final placements and populate global_event_entries
    let mut standings_page = 1u32;
    loop {
        let data: EventStandingsData = gql_query(
            client, base_url, token, EVENT_STANDINGS_QUERY,
            json!({ "eventId": event_startgg_id, "page": standings_page, "perPage": STANDINGS_PER_PAGE }),
            delay,
        )
        .await?;

        let standings_node = match data.event {
            Some(e) => e,
            None => break,
        };
        let total_pages = standings_node.standings.page_info.total_pages.unwrap_or(1);

        for standing in &standings_node.standings.nodes {
            if let Some(entrant) = &standing.entrant {
                if let Some(&player_uuid) = event_entrant_map.get(&entrant.id) {
                    upsert_event_entry(
                        pool,
                        event_id,
                        player_uuid,
                        None,
                        standing.placement.map(|p| p as i32),
                    )
                    .await?;
                }
            }
        }

        if standings_page >= total_pages as u32 {
            break;
        }
        standings_page += 1;
        tokio::time::sleep(delay).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Per-phase-group processing — full path then two-pass fallback
// ---------------------------------------------------------------------------

async fn process_phase_group(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    pg_startgg_id: i64,
    phase_startgg_id: i64,
    event_id: Uuid,
    sets_per_page: u32,
    delay: Duration,
    event_entrant_map: &mut HashMap<i64, Uuid>,
) -> Result<()> {
    // Track set_startgg_id → set UUID for games pass
    let mut set_id_to_uuid: HashMap<i64, Uuid> = HashMap::new();

    // Attempt full query with complexity halving
    let full_result: Result<HashMap<i64, Uuid>> =
        with_complexity_retry!(sets_per_page, 1, |per_page| async move {
            fetch_full_path(
                client,
                base_url,
                pool,
                token,
                pg_startgg_id,
                phase_startgg_id,
                event_id,
                per_page,
                delay,
            )
            .await
        });

    match full_result {
        Ok(map) => {
            event_entrant_map.extend(map);
            return Ok(());
        }
        Err(err) => {
            let is_complexity = err.downcast_ref::<ComplexityError>().is_some();
            if !is_complexity {
                return Err(err);
            }
            warn!(
                pg_startgg_id,
                "full query failed at perPage=1, falling back to two-pass"
            );
        }
    }

    // Pass 1: slim identity pass — populates event_entrant_map directly
    let slim_result = fetch_slim_pass(
        client,
        base_url,
        pool,
        token,
        pg_startgg_id,
        phase_startgg_id,
        event_id,
        sets_per_page,
        delay,
        event_entrant_map,
        &mut set_id_to_uuid,
    )
    .await;

    if let Err(e) = &slim_result {
        warn!(pg_startgg_id, error = %e, "slim pass also failed, skipping phase group");
        return Ok(());
    }

    // Pass 2: games pass
    let games_result = fetch_games_pass(
        client,
        base_url,
        pool,
        token,
        pg_startgg_id,
        sets_per_page,
        delay,
        event_entrant_map,
        &set_id_to_uuid,
    )
    .await;

    if let Err(e) = &games_result {
        warn!(pg_startgg_id, error = %e, "games pass failed, game data unavailable for this phase group");
    }

    Ok(())
}

async fn fetch_full_path(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    pg_startgg_id: i64,
    phase_startgg_id: i64,
    event_id: Uuid,
    sets_per_page: u32,
    delay: Duration,
) -> Result<HashMap<i64, Uuid>> {
    let mut page = 1u32;
    // Phase/phase_group upserted on first page to avoid extra query
    let mut phase_uuid: Option<Uuid> = None;
    let mut phase_group_uuid: Option<Uuid> = None;
    // Accumulate entrant → player UUID across all pages for caller use
    let mut all_entrant_map: HashMap<i64, Uuid> = HashMap::new();

    loop {
        let data: FullPhaseGroupSetsData = gql_query(
            client,
            base_url,
            token,
            PHASE_GROUP_SETS_QUERY,
            json!({ "phaseGroupId": pg_startgg_id, "page": page, "perPage": sets_per_page }),
            delay,
        )
        .await?;

        let pg_node = match data.phase_group {
            Some(n) => n,
            None => break,
        };
        let total_pages = pg_node.sets.page_info.total_pages.unwrap_or(1);

        // Build entrant_id → player_uuid map for this page's sets
        let mut local_entrant_map: HashMap<i64, Uuid> = HashMap::new();

        for set_node in &pg_node.sets.nodes {
            let Some(set_startgg_id) = set_node.id else { continue };
            // Upsert phase + phase_group from first set that has them
            if phase_uuid.is_none() {
                if let Some(pg_info) = &set_node.phase_group {
                    if let Some(phase_info) = &pg_info.phase {
                        let pu = upsert_phase(pool, phase_startgg_id, event_id, phase_info).await?;
                        phase_uuid = Some(pu);
                        let pgu = upsert_phase_group(pool, pg_startgg_id, pu, pg_info).await?;
                        phase_group_uuid = Some(pgu);
                    }
                }
            }

            // Resolve players from participants
            for slot in &set_node.slots {
                if let Some(entrant) = &slot.entrant {
                    if let Some(participant) = entrant.participants.first() {
                        let player_uuid = if let Some(user) = &participant.user {
                            let player_id = participant.player.as_ref().map(|p| p.id);
                            let image_url = user
                                .images
                                .iter()
                                .find(|img| img.image_type.as_deref() == Some("profile"))
                                .or_else(|| user.images.first())
                                .and_then(|img| img.url.as_deref());
                            let loc = user.location.as_ref();
                            upsert_player_full(
                                pool,
                                user.id,
                                player_id,
                                participant
                                    .player
                                    .as_ref()
                                    .and_then(|p| p.gamer_tag.as_deref())
                                    .unwrap_or("Unknown"),
                                user.name.as_deref(),
                                image_url,
                                user.slug.as_deref(),
                                user.bio.as_deref(),
                                user.gender_pronoun.as_deref(),
                                loc.and_then(|l| l.city.as_deref()),
                                loc.and_then(|l| l.state.as_deref()),
                                loc.and_then(|l| l.country.as_deref()),
                            )
                            .await?
                        } else if let Some(player) = &participant.player {
                            upsert_player_slim(
                                pool,
                                player.id,
                                player.gamer_tag.as_deref().unwrap_or("Unknown"),
                            )
                            .await?
                        } else {
                            continue;
                        };
                        local_entrant_map.insert(entrant.id, player_uuid);
                        all_entrant_map.insert(entrant.id, player_uuid);
                    }
                }
            }

            // Determine winner/loser players from slots
            let (winner_uuid, loser_uuid) =
                resolve_winner_loser(set_node.winner_id, &set_node.slots, &local_entrant_map);

            let (winner_score, loser_score) = set_node
                .display_score
                .as_deref()
                .and_then(extract_scores)
                .map(|(w, l)| (Some(w), Some(l)))
                .unwrap_or((None, None));

            let completed_at = set_node.completed_at.map(ts_to_dt);
            let dq = is_dq(set_node.state, set_node.display_score.as_deref());

            let set_uuid = upsert_set(
                pool,
                set_startgg_id,
                event_id,
                phase_group_uuid,
                winner_uuid,
                loser_uuid,
                set_node.round,
                set_node.full_round_text.as_deref(),
                winner_score,
                loser_score,
                dq,
                set_node.vod_url.as_deref(),
                completed_at,
            )
            .await?;

            // Upsert games + selections
            for game in &set_node.games {
                let order_num = game.order_num.unwrap_or(0) as i32;
                let game_winner_uuid = game
                    .winner_id
                    .and_then(|wid| local_entrant_map.get(&wid).copied());
                let stage_id = game.stage.as_ref().map(|s| s.id);
                let stage_name = game.stage.as_ref().and_then(|s| s.name.as_deref());

                let game_uuid = upsert_set_game(
                    pool,
                    set_uuid,
                    order_num,
                    game_winner_uuid,
                    stage_id,
                    stage_name,
                )
                .await?;

                for sel in &game.selections {
                    if let Some(sel_type) = &sel.selection_type {
                        let player_uuid = sel
                            .entrant
                            .as_ref()
                            .and_then(|e| local_entrant_map.get(&e.id).copied());
                        let char_id = sel.character.as_ref().map(|c| c.id);
                        let char_name = sel.character.as_ref().and_then(|c| c.name.as_deref());
                        upsert_game_selection(
                            pool,
                            game_uuid,
                            player_uuid,
                            sel_type,
                            char_id,
                            char_name,
                        )
                        .await?;
                    }
                }
            }
        }

        if page >= total_pages as u32 {
            break;
        }
        page += 1;
        tokio::time::sleep(delay).await;
    }

    Ok(all_entrant_map)
}

async fn fetch_slim_pass(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    pg_startgg_id: i64,
    phase_startgg_id: i64,
    event_id: Uuid,
    sets_per_page: u32,
    delay: Duration,
    entrant_to_player: &mut HashMap<i64, Uuid>,
    set_id_to_uuid: &mut HashMap<i64, Uuid>,
) -> Result<()> {
    let mut page = 1u32;
    let mut phase_uuid: Option<Uuid> = None;
    let mut phase_group_uuid: Option<Uuid> = None;

    loop {
        let data: SlimPhaseGroupSetsData = with_complexity_retry!(sets_per_page, 1, |per_page| {
            let client = client.clone();
            let base_url = base_url.to_string();
            let token = token.to_string();
            async move {
                gql_query::<SlimPhaseGroupSetsData>(
                    &client,
                    &base_url,
                    &token,
                    PHASE_GROUP_SETS_QUERY_SLIM,
                    json!({ "phaseGroupId": pg_startgg_id, "page": page, "perPage": per_page }),
                    delay,
                )
                .await
            }
        })?;

        let pg_node = match data.phase_group {
            Some(n) => n,
            None => break,
        };
        let total_pages = pg_node.sets.page_info.total_pages.unwrap_or(1);

        for set_node in &pg_node.sets.nodes {
            let Some(set_startgg_id) = set_node.id else { continue };
            if phase_uuid.is_none() {
                if let Some(pg_info) = &set_node.phase_group {
                    if let Some(phase_info) = &pg_info.phase {
                        let pu = upsert_phase(pool, phase_startgg_id, event_id, phase_info).await?;
                        phase_uuid = Some(pu);
                        let pgu = upsert_phase_group(pool, pg_startgg_id, pu, pg_info).await?;
                        phase_group_uuid = Some(pgu);
                    }
                }
            }

            for slot in &set_node.slots {
                if let Some(entrant) = &slot.entrant {
                    if let Some(participant) = entrant.participants.first() {
                        if let Some(player) = &participant.player {
                            let uuid = upsert_player_slim(
                                pool,
                                player.id,
                                player.gamer_tag.as_deref().unwrap_or("Unknown"),
                            )
                            .await?;
                            entrant_to_player.insert(entrant.id, uuid);
                        }
                    }
                }
            }

            let (winner_uuid, loser_uuid) =
                resolve_winner_loser_slim(set_node.winner_id, &set_node.slots, entrant_to_player);

            let (winner_score, loser_score) = set_node
                .display_score
                .as_deref()
                .and_then(extract_scores)
                .map(|(w, l)| (Some(w), Some(l)))
                .unwrap_or((None, None));

            let set_uuid = upsert_set(
                pool,
                set_startgg_id,
                event_id,
                phase_group_uuid,
                winner_uuid,
                loser_uuid,
                set_node.round,
                set_node.full_round_text.as_deref(),
                winner_score,
                loser_score,
                is_dq(set_node.state, set_node.display_score.as_deref()),
                set_node.vod_url.as_deref(),
                set_node.completed_at.map(ts_to_dt),
            )
            .await?;

            set_id_to_uuid.insert(set_startgg_id, set_uuid);
        }

        if page >= total_pages as u32 {
            break;
        }
        page += 1;
        tokio::time::sleep(delay).await;
    }

    Ok(())
}

async fn fetch_games_pass(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    pg_startgg_id: i64,
    sets_per_page: u32,
    delay: Duration,
    entrant_to_player: &HashMap<i64, Uuid>,
    set_id_to_uuid: &HashMap<i64, Uuid>,
) -> Result<()> {
    let mut page = 1u32;

    loop {
        let data: GamesPhaseGroupSetsData = with_complexity_retry!(sets_per_page, 1, |per_page| {
            let client = client.clone();
            let base_url = base_url.to_string();
            let token = token.to_string();
            async move {
                gql_query::<GamesPhaseGroupSetsData>(
                    &client,
                    &base_url,
                    &token,
                    PHASE_GROUP_GAMES_QUERY,
                    json!({ "phaseGroupId": pg_startgg_id, "page": page, "perPage": per_page }),
                    delay,
                )
                .await
            }
        })?;

        let pg_node = match data.phase_group {
            Some(n) => n,
            None => break,
        };
        let total_pages = pg_node.sets.page_info.total_pages.unwrap_or(1);

        for set_node in &pg_node.sets.nodes {
            let Some(set_startgg_id) = set_node.id else { continue };
            let set_uuid = match set_id_to_uuid.get(&set_startgg_id) {
                Some(u) => *u,
                None => continue,
            };

            for game in &set_node.games {
                let order_num = game.order_num.unwrap_or(0) as i32;
                let game_winner_uuid = game
                    .winner_id
                    .and_then(|wid| entrant_to_player.get(&wid).copied());
                let stage_id = game.stage.as_ref().map(|s| s.id);
                let stage_name = game.stage.as_ref().and_then(|s| s.name.as_deref());

                let game_uuid = upsert_set_game(
                    pool,
                    set_uuid,
                    order_num,
                    game_winner_uuid,
                    stage_id,
                    stage_name,
                )
                .await?;

                for sel in &game.selections {
                    if let Some(sel_type) = &sel.selection_type {
                        let player_uuid = sel
                            .entrant
                            .as_ref()
                            .and_then(|e| entrant_to_player.get(&e.id).copied());
                        let char_id = sel.character.as_ref().map(|c| c.id);
                        let char_name = sel.character.as_ref().and_then(|c| c.name.as_deref());
                        upsert_game_selection(
                            pool,
                            game_uuid,
                            player_uuid,
                            sel_type,
                            char_id,
                            char_name,
                        )
                        .await?;
                    }
                }
            }
        }

        if page >= total_pages as u32 {
            break;
        }
        page += 1;
        tokio::time::sleep(delay).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Winner/loser resolution helpers
// ---------------------------------------------------------------------------

fn resolve_winner_loser(
    winner_entrant_id: Option<i64>,
    slots: &[crate::api_types::SlotNode],
    entrant_map: &HashMap<i64, Uuid>,
) -> (Option<Uuid>, Option<Uuid>) {
    let mut winner = None;
    let mut loser = None;
    for slot in slots {
        if let Some(entrant) = &slot.entrant {
            let uuid = entrant_map.get(&entrant.id).copied();
            if Some(entrant.id) == winner_entrant_id {
                winner = uuid;
            } else {
                loser = uuid;
            }
        }
    }
    (winner, loser)
}

fn resolve_winner_loser_slim(
    winner_entrant_id: Option<i64>,
    slots: &[crate::api_types::SlimSlotNode],
    entrant_map: &HashMap<i64, Uuid>,
) -> (Option<Uuid>, Option<Uuid>) {
    let mut winner = None;
    let mut loser = None;
    for slot in slots {
        if let Some(entrant) = &slot.entrant {
            let uuid = entrant_map.get(&entrant.id).copied();
            if Some(entrant.id) == winner_entrant_id {
                winner = uuid;
            } else {
                loser = uuid;
            }
        }
    }
    (winner, loser)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dq_detected_by_state() {
        assert!(is_dq(Some(7), None));
    }

    #[test]
    fn dq_detected_by_display_score() {
        assert!(is_dq(Some(3), Some("DQ")));
        assert!(is_dq(None, Some("Player DQ - 0")));
    }

    #[test]
    fn no_dq_for_normal_set() {
        assert!(!is_dq(Some(3), Some("3 - 1")));
    }

    #[test]
    fn scores_extracted_from_display_score() {
        let (w, l) = extract_scores("3 - 1").unwrap();
        assert_eq!(w, 3);
        assert_eq!(l, 1);
        let (w, l) = extract_scores("2 - 0").unwrap();
        assert_eq!(w, 2);
        assert_eq!(l, 0);
    }

    #[test]
    fn scores_extracted_from_name_bearing_display_score() {
        let (w, l) = extract_scores("Mang0 3 - Zain 1").unwrap();
        assert_eq!(w, 3);
        assert_eq!(l, 1);
    }

    #[test]
    fn scores_none_for_dq_display() {
        assert!(extract_scores("DQ").is_none());
        assert!(extract_scores("").is_none());
    }
}
