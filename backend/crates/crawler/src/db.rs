use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api_types::{EventNode, PhaseGroupInfo, PhaseInfo, TournamentNode};

pub async fn upsert_game(pool: &PgPool, startgg_id: i64, name: &str) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_games (startgg_id, name)
        VALUES ($1, $2)
        ON CONFLICT (startgg_id) DO UPDATE SET name = EXCLUDED.name
        RETURNING id
        "#,
        startgg_id,
        name,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_player_full(
    pool: &PgPool,
    startgg_user_id: i64,
    startgg_player_id: Option<i64>,
    handle: &str,
    display_name: Option<&str>,
    profile_image_url: Option<&str>,
    startgg_slug: Option<&str>,
    bio: Option<&str>,
    pronouns: Option<&str>,
    location_city: Option<&str>,
    location_state: Option<&str>,
    location_country: Option<&str>,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_players
            (startgg_user_id, startgg_player_id, handle, display_name,
             profile_image_url, startgg_slug, bio, pronouns,
             location_city, location_state, location_country)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        ON CONFLICT (startgg_user_id) DO UPDATE SET
            handle            = EXCLUDED.handle,
            display_name      = EXCLUDED.display_name,
            startgg_player_id = COALESCE(EXCLUDED.startgg_player_id, global_players.startgg_player_id),
            profile_image_url = COALESCE(EXCLUDED.profile_image_url, global_players.profile_image_url),
            startgg_slug      = COALESCE(EXCLUDED.startgg_slug,      global_players.startgg_slug),
            bio               = COALESCE(EXCLUDED.bio,               global_players.bio),
            pronouns          = COALESCE(EXCLUDED.pronouns,          global_players.pronouns),
            location_city     = EXCLUDED.location_city,
            location_state    = EXCLUDED.location_state,
            location_country  = EXCLUDED.location_country,
            updated_at        = NOW()
        RETURNING id
        "#,
        startgg_user_id,
        startgg_player_id,
        handle,
        display_name,
        profile_image_url,
        startgg_slug,
        bio,
        pronouns,
        location_city,
        location_state,
        location_country,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_player_slim(
    pool: &PgPool,
    startgg_player_id: i64,
    handle: &str,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_players (startgg_player_id, handle)
        VALUES ($1, $2)
        ON CONFLICT (startgg_player_id) DO UPDATE SET
            handle     = EXCLUDED.handle,
            updated_at = NOW()
        RETURNING id
        "#,
        startgg_player_id,
        handle,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_tournament(pool: &PgPool, node: &TournamentNode) -> Result<Uuid> {
    let start_at = node.start_at.and_then(|ts| DateTime::from_timestamp(ts, 0));
    let end_at = node.end_at.and_then(|ts| DateTime::from_timestamp(ts, 0));
    let row = sqlx::query!(
        r#"
        INSERT INTO global_tournaments
            (startgg_id, name, slug, start_at, end_at, country_code, city,
             addr_state, online, num_attendees, lat, lng, timezone)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        ON CONFLICT (startgg_id) DO UPDATE SET
            name          = EXCLUDED.name,
            slug          = EXCLUDED.slug,
            start_at      = EXCLUDED.start_at,
            end_at        = EXCLUDED.end_at,
            country_code  = EXCLUDED.country_code,
            city          = EXCLUDED.city,
            addr_state    = EXCLUDED.addr_state,
            online        = EXCLUDED.online,
            num_attendees = EXCLUDED.num_attendees,
            lat           = EXCLUDED.lat,
            lng           = EXCLUDED.lng,
            timezone      = EXCLUDED.timezone
        RETURNING id
        "#,
        node.id,
        node.name,
        node.slug,
        start_at,
        end_at,
        node.country_code,
        node.city,
        node.addr_state,
        node.is_online,
        node.num_attendees.map(|n| n as i32),
        node.lat,
        node.lng,
        node.timezone,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_event(
    pool: &PgPool,
    node: &EventNode,
    tournament_id: Uuid,
    game_id: Option<Uuid>,
) -> Result<Uuid> {
    let start_at = node.start_at.and_then(|ts| DateTime::from_timestamp(ts, 0));
    let row = sqlx::query!(
        r#"
        INSERT INTO global_events
            (startgg_id, tournament_id, game_id, name, slug,
             start_at, num_entrants, is_online, competition_tier)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (startgg_id) DO UPDATE SET
            tournament_id    = EXCLUDED.tournament_id,
            game_id          = EXCLUDED.game_id,
            name             = EXCLUDED.name,
            slug             = EXCLUDED.slug,
            start_at         = EXCLUDED.start_at,
            num_entrants     = EXCLUDED.num_entrants,
            is_online        = EXCLUDED.is_online,
            competition_tier = EXCLUDED.competition_tier
        RETURNING id
        "#,
        node.id,
        tournament_id,
        game_id,
        node.name,
        node.slug,
        start_at,
        node.num_entrants.map(|n| n as i32),
        node.is_online,
        node.competition_tier.map(|n| n as i32),
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_phase(
    pool: &PgPool,
    startgg_id: i64,
    event_id: Uuid,
    info: &PhaseInfo,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_phases
            (startgg_id, event_id, name, phase_order, bracket_type, is_exhibition)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (startgg_id) DO UPDATE SET
            name         = EXCLUDED.name,
            phase_order  = EXCLUDED.phase_order,
            bracket_type = EXCLUDED.bracket_type,
            is_exhibition = EXCLUDED.is_exhibition
        RETURNING id
        "#,
        startgg_id,
        event_id,
        info.name,
        info.phase_order.map(|n| n as i32),
        info.bracket_type,
        info.is_exhibition.unwrap_or(false),
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_phase_group(
    pool: &PgPool,
    startgg_id: i64,
    phase_id: Uuid,
    info: &PhaseGroupInfo,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_phase_groups
            (startgg_id, phase_id, display_identifier, bracket_type)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (startgg_id) DO UPDATE SET
            display_identifier = EXCLUDED.display_identifier,
            bracket_type       = EXCLUDED.bracket_type
        RETURNING id
        "#,
        startgg_id,
        phase_id,
        info.display_identifier,
        info.bracket_type,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_set(
    pool: &PgPool,
    startgg_id: i64,
    event_id: Uuid,
    phase_group_id: Option<Uuid>,
    winner_player_id: Option<Uuid>,
    loser_player_id: Option<Uuid>,
    round: Option<i64>,
    round_name: Option<&str>,
    winner_score: Option<i16>,
    loser_score: Option<i16>,
    is_dq: bool,
    vod_url: Option<&str>,
    completed_at: Option<DateTime<Utc>>,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_sets
            (startgg_id, event_id, phase_group_id, winner_player_id, loser_player_id,
             round, round_name, winner_score, loser_score, is_dq, vod_url, completed_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (startgg_id) DO UPDATE SET
            winner_player_id = EXCLUDED.winner_player_id,
            loser_player_id  = EXCLUDED.loser_player_id,
            winner_score     = EXCLUDED.winner_score,
            loser_score      = EXCLUDED.loser_score,
            is_dq            = EXCLUDED.is_dq,
            vod_url          = EXCLUDED.vod_url,
            completed_at     = EXCLUDED.completed_at
        RETURNING id
        "#,
        startgg_id,
        event_id,
        phase_group_id,
        winner_player_id,
        loser_player_id,
        round.map(|n| n as i32),
        round_name,
        winner_score,
        loser_score,
        is_dq,
        vod_url,
        completed_at,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_set_game(
    pool: &PgPool,
    set_uuid: Uuid,
    order_num: i32,
    winner_player_id: Option<Uuid>,
    stage_id: Option<i64>,
    stage_name: Option<&str>,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_set_games (set_id, order_num, winner_player_id, stage_id, stage_name)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (set_id, order_num) DO UPDATE SET
            winner_player_id = EXCLUDED.winner_player_id,
            stage_id         = EXCLUDED.stage_id,
            stage_name       = EXCLUDED.stage_name
        RETURNING id
        "#,
        set_uuid,
        order_num,
        winner_player_id,
        stage_id,
        stage_name,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_game_selection(
    pool: &PgPool,
    game_uuid: Uuid,
    player_id: Option<Uuid>,
    selection_type: &str,
    character_id: Option<i64>,
    character_name: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO global_game_selections
            (game_id, player_id, selection_type, character_id, character_name)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (game_id, player_id, selection_type) DO UPDATE SET
            character_id   = EXCLUDED.character_id,
            character_name = EXCLUDED.character_name
        "#,
        game_uuid,
        player_id,
        selection_type,
        character_id,
        character_name,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn upsert_event_entry(
    pool: &PgPool,
    event_id: Uuid,
    player_id: Uuid,
    seed: Option<i32>,
    placement: Option<i32>,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO global_event_entries (event_id, player_id, seed, placement)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (event_id, player_id) DO UPDATE SET
            seed      = COALESCE(EXCLUDED.seed, global_event_entries.seed),
            placement = COALESCE(EXCLUDED.placement, global_event_entries.placement)
        "#,
        event_id,
        player_id,
        seed,
        placement,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_checkpoint(pool: &PgPool, key: &str) -> Result<Option<serde_json::Value>> {
    let row = sqlx::query!("SELECT value FROM crawler_checkpoints WHERE key = $1", key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.value))
}

pub async fn set_checkpoint(pool: &PgPool, key: &str, value: serde_json::Value) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO crawler_checkpoints (key, value)
        VALUES ($1, $2)
        ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
        "#,
        key,
        value,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub fn is_checkpointed(cp: &Option<serde_json::Value>) -> bool {
    matches!(cp, Some(v) if v.as_bool() == Some(true))
}
