use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::{AuthUser, OptionalAuthUser},
    routes::rankings::{RankingPath, require_ranking_access, require_ranking_read_access},
    state::AppState,
};
use common::jobs::enqueue_compute_ranking;
use common::models::UserRole;
use common::upset::set_upset_factor;

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProjectEventResponse {
    pub id: Uuid,
    pub startgg_id: i64,
    pub name: String,
    pub game_name: Option<String>,
    pub num_entrants: Option<i32>,
    pub start_at: Option<DateTime<Utc>>,
    pub included: bool,
    pub event_type: Option<i32>,
    pub bracket_types: Vec<String>,
}

#[derive(Serialize)]
pub struct TournamentResponse {
    pub id: Uuid,
    pub startgg_id: i64,
    pub name: String,
    pub handle: String,
    pub city: Option<String>,
    pub addr_state: Option<String>,
    pub country_code: Option<String>,
    pub venue_name: Option<String>,
    pub online: bool,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub events: Vec<ProjectEventResponse>,
}

fn compute_location(
    online: bool,
    city: Option<&str>,
    state: Option<&str>,
    country: Option<&str>,
) -> Option<String> {
    if online {
        return Some("Online".to_string());
    }
    match (city, state, country) {
        (Some(c), Some(s), _) => Some(format!("{c}, {s}")),
        (Some(c), None, Some(cc)) => Some(format!("{c}, {cc}")),
        (Some(c), None, None) => Some(c.to_string()),
        _ => None,
    }
}

#[derive(Serialize)]
pub struct SetRecord {
    pub opponent_id: Option<Uuid>,
    pub opponent_name: String,
    pub upset_factor: i64,
    pub winner_score: Option<i16>,
    pub loser_score: Option<i16>,
    pub tournament_name: String,
    pub tournament_handle: String,
    pub event_name: String,
    pub round_name: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub is_dq: bool,
    pub vod_url: Option<String>,
    pub startgg_set_id: i64,
    pub winner_seed: Option<i32>,
    pub loser_seed: Option<i32>,
    pub phase_name: Option<String>,
    pub pool_identifier: Option<String>,
    pub winner_placement: Option<i32>,
    pub loser_placement: Option<i32>,
    pub location: Option<String>,
    pub num_entrants: Option<i32>,
    pub event_handle: Option<String>,
}

#[derive(Serialize)]
pub struct PlayerStatsResponse {
    pub player_id: Uuid,
    pub name: String,
    pub wins: Vec<SetRecord>,
    pub losses: Vec<SetRecord>,
}

#[derive(Serialize)]
pub struct HeadToHeadEntry {
    pub player_id: Uuid,
    pub opponent_id: Uuid,
    pub wins: i64,
    pub losses: i64,
}

// ── Path param structs ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RankingH2HPath {
    pub id: Uuid,
    pub rid: Uuid,
    pub pid_a: Uuid,
    pub pid_b: Uuid,
}

#[derive(Deserialize)]
pub struct RankingPlayerStatPath {
    pub id: Uuid,
    pub rid: Uuid,
    pub player_id: Uuid,
}

#[derive(Serialize)]
pub struct H2HSet {
    #[serde(flatten)]
    pub set: SetRecord,
    pub is_win: bool,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

pub async fn list_tournaments(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    #[derive(Debug)]
    struct Row {
        tournament_id: Uuid,
        tournament_startgg_id: i64,
        tournament_name: String,
        tournament_handle: String,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        venue_name: Option<String>,
        online: bool,
        tournament_start_at: Option<DateTime<Utc>>,
        end_at: Option<DateTime<Utc>>,
        event_id: Uuid,
        event_startgg_id: i64,
        event_name: String,
        game_name: Option<String>,
        num_entrants: Option<i32>,
        event_start_at: Option<DateTime<Utc>>,
        included: bool,
        event_type: Option<i32>,
        bracket_types: Vec<String>,
    }

    let rows = sqlx::query_as!(
        Row,
        r#"
        SELECT
            gt.id            AS tournament_id,
            gt.startgg_id    AS tournament_startgg_id,
            gt.name          AS tournament_name,
            gt.slug          AS tournament_handle,
            gt.city,
            gt.addr_state,
            gt.country_code,
            gt.venue_name,
            gt.online        AS "online!: bool",
            gt.start_at      AS tournament_start_at,
            gt.end_at,
            ge.id            AS event_id,
            ge.startgg_id    AS event_startgg_id,
            ge.name          AS event_name,
            CASE WHEN ge.game_id IS NOT NULL THEN gg.name END AS game_name,
            ge.num_entrants,
            ge.start_at      AS event_start_at,
            re.included,
            NULL::INTEGER    AS event_type,
            ARRAY(
                SELECT gp.bracket_type
                FROM global_phases gp
                WHERE gp.event_id = ge.id
                  AND gp.bracket_type IS NOT NULL
                ORDER BY gp.phase_order ASC NULLS LAST
            ) AS "bracket_types!: Vec<String>"
        FROM ranking_events re
        JOIN global_events ge      ON ge.id = re.global_event_id
        JOIN global_tournaments gt ON gt.id = ge.tournament_id
        LEFT JOIN global_games gg  ON gg.id = ge.game_id
        WHERE re.ranking_id = $1
        ORDER BY gt.start_at DESC NULLS LAST, gt.name ASC, ge.name ASC
        "#,
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    // Group events under their tournament, preserving ORDER BY order.
    let mut tournament_order: Vec<Uuid> = Vec::new();
    let mut tournament_map: HashMap<Uuid, TournamentResponse> = HashMap::new();

    for row in rows {
        let t_entry = tournament_map.entry(row.tournament_id).or_insert_with(|| {
            tournament_order.push(row.tournament_id);
            TournamentResponse {
                id: row.tournament_id,
                startgg_id: row.tournament_startgg_id,
                name: row.tournament_name.clone(),
                handle: row.tournament_handle.clone(),
                city: row.city.clone(),
                addr_state: row.addr_state.clone(),
                country_code: row.country_code.clone(),
                venue_name: row.venue_name.clone(),
                online: row.online,
                start_at: row.tournament_start_at,
                end_at: row.end_at,
                events: Vec::new(),
            }
        });

        t_entry.events.push(ProjectEventResponse {
            id: row.event_id,
            startgg_id: row.event_startgg_id,
            name: row.event_name,
            game_name: row.game_name,
            num_entrants: row.num_entrants,
            start_at: row.event_start_at,
            included: row.included,
            event_type: row.event_type,
            bracket_types: row.bracket_types,
        });
    }

    let resp: Vec<TournamentResponse> = tournament_order
        .into_iter()
        .filter_map(|id| tournament_map.remove(&id))
        .collect();

    Ok(Json(resp))
}

#[derive(Deserialize)]
pub struct EventInclusionItem {
    pub event_id: Uuid,
    pub included: bool,
}

pub async fn put_events(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
    Json(body): Json<Vec<EventInclusionItem>>,
) -> Result<impl IntoResponse> {
    let (project, _, _) =
        require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;

    let mut tx = state.db.begin().await?;
    for item in &body {
        sqlx::query!(
            "UPDATE ranking_events SET included = $1
             WHERE ranking_id = $2 AND global_event_id = $3",
            item.included,
            path.rid,
            item.event_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let _ = enqueue_compute_ranking(&state.db, project.id, path.rid).await;

    Ok(StatusCode::ACCEPTED)
}

pub async fn get_stats(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    struct PlayerRow {
        id: Uuid,
        name: String,
    }
    let players = sqlx::query_as!(
        PlayerRow,
        r#"
        SELECT rp.player_id AS id, pl.name
        FROM ranking_players rp
        JOIN players pl ON pl.id = rp.player_id
        WHERE rp.ranking_id = $1
        ORDER BY rp.rank_position ASC, pl.created_at ASC
        "#,
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    struct SetRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        upset_factor: Option<f64>,
        completed_at: Option<DateTime<Utc>>,
        winner_score: Option<i16>,
        loser_score: Option<i16>,
        round_name: Option<String>,
        is_dq: bool,
        vod_url: Option<String>,
        startgg_set_id: i64,
        event_name: String,
        tournament_name: String,
        tournament_handle: String,
        winner_seed: Option<i32>,
        loser_seed: Option<i32>,
        winner_placement: Option<i32>,
        loser_placement: Option<i32>,
        winner_handle: String,
        loser_handle: String,
        phase_name: Option<String>,
        pool_identifier: Option<String>,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        online: bool,
        num_entrants: Option<i32>,
        event_handle: Option<String>,
    }

    let rows = sqlx::query_as!(
        SetRow,
        r#"
        SELECT
            rsr.winner_player_id,
            rsr.loser_player_id,
            rsr.upset_factor,
            rsr.completed_at,
            gs.winner_score,
            gs.loser_score,
            gs.round_name,
            gs.is_dq,
            gs.vod_url,
            gs.startgg_id   AS startgg_set_id,
            ge.name         AS event_name,
            gt.name         AS tournament_name,
            gt.slug         AS tournament_handle,
            wee.seed        AS winner_seed,
            lee.seed        AS loser_seed,
            wee.placement   AS winner_placement,
            lee.placement   AS loser_placement,
            gwp.handle      AS winner_handle,
            glp.handle      AS loser_handle,
            gph.name        AS phase_name,
            gpg.display_identifier AS pool_identifier,
            gt.city,
            gt.addr_state,
            gt.country_code,
            gt.online AS "online!: bool",
            ge.num_entrants,
            ge.slug         AS event_handle
        FROM ranking_set_results rsr
        JOIN global_sets gs        ON gs.id  = rsr.global_set_id
        JOIN global_events ge      ON ge.id  = rsr.global_event_id
        JOIN global_tournaments gt ON gt.id  = ge.tournament_id
        JOIN global_players gwp    ON gwp.id = gs.winner_player_id
        JOIN global_players glp    ON glp.id = gs.loser_player_id
        LEFT JOIN global_event_entries wee ON wee.event_id = ge.id AND wee.player_id = gwp.id
        LEFT JOIN global_event_entries lee ON lee.event_id = ge.id AND lee.player_id = glp.id
        LEFT JOIN global_phase_groups gpg ON gpg.id = gs.phase_group_id
        LEFT JOIN global_phases gph       ON gph.id = gpg.phase_id
        WHERE rsr.ranking_id = $1
        ORDER BY rsr.completed_at DESC NULLS LAST
        "#,
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    let player_order: Vec<Uuid> = players.iter().map(|p| p.id).collect();
    let mut stats: HashMap<Uuid, (String, Vec<SetRecord>, Vec<SetRecord>)> = players
        .into_iter()
        .map(|p| (p.id, (p.name, Vec::new(), Vec::new())))
        .collect();

    for row in rows {
        let uf = match (row.winner_seed, row.loser_seed) {
            (Some(ws), Some(ls)) => set_upset_factor(ws, ls) as i64,
            _ => 0,
        };
        let location = compute_location(
            row.online,
            row.city.as_deref(),
            row.addr_state.as_deref(),
            row.country_code.as_deref(),
        );
        let make_record = |opponent_id: Option<Uuid>, opponent_name: String| SetRecord {
            opponent_id,
            opponent_name,
            upset_factor: uf,
            winner_score: row.winner_score,
            loser_score: row.loser_score,
            tournament_name: row.tournament_name.clone(),
            tournament_handle: row.tournament_handle.clone(),
            event_name: row.event_name.clone(),
            round_name: row.round_name.clone(),
            completed_at: row.completed_at,
            is_dq: row.is_dq,
            vod_url: row.vod_url.clone(),
            startgg_set_id: row.startgg_set_id,
            winner_seed: row.winner_seed,
            loser_seed: row.loser_seed,
            phase_name: row.phase_name.clone(),
            pool_identifier: row.pool_identifier.clone(),
            winner_placement: row.winner_placement,
            loser_placement: row.loser_placement,
            location: location.clone(),
            num_entrants: row.num_entrants,
            event_handle: row.event_handle.clone(),
        };
        let winner_id = row.winner_player_id;
        if let Some(entry) = stats.get_mut(&winner_id) {
            entry.1.push(make_record(
                Some(row.loser_player_id),
                row.loser_handle.clone(),
            ));
        }
        let loser_id = row.loser_player_id;
        if let Some(entry) = stats.get_mut(&loser_id) {
            entry.2.push(make_record(
                Some(row.winner_player_id),
                row.winner_handle.clone(),
            ));
        }
    }

    for entry in stats.values_mut() {
        entry.1.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));
        entry.2.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));
    }

    let resp: Vec<PlayerStatsResponse> = player_order
        .iter()
        .filter_map(|&id| {
            stats
                .remove(&id)
                .map(|(name, wins, losses)| PlayerStatsResponse {
                    player_id: id,
                    name,
                    wins,
                    losses,
                })
        })
        .collect();

    Ok(Json(resp))
}

pub async fn get_player_stats(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPlayerStatPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    let name: Option<String> = sqlx::query_scalar!(
        r#"SELECT pl.name FROM ranking_players rp
           JOIN players pl ON pl.id = rp.player_id
           WHERE rp.ranking_id = $1 AND rp.player_id = $2"#,
        path.rid,
        path.player_id,
    )
    .fetch_optional(&state.db)
    .await?;
    let name = name.ok_or(AppError::NotFound)?;

    struct SetRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        upset_factor: Option<f64>,
        completed_at: Option<DateTime<Utc>>,
        winner_score: Option<i16>,
        loser_score: Option<i16>,
        round_name: Option<String>,
        is_dq: bool,
        vod_url: Option<String>,
        startgg_set_id: i64,
        event_name: String,
        tournament_name: String,
        tournament_handle: String,
        winner_seed: Option<i32>,
        loser_seed: Option<i32>,
        winner_placement: Option<i32>,
        loser_placement: Option<i32>,
        winner_handle: String,
        loser_handle: String,
        phase_name: Option<String>,
        pool_identifier: Option<String>,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        online: bool,
        num_entrants: Option<i32>,
        event_handle: Option<String>,
    }

    let rows = sqlx::query_as!(
        SetRow,
        r#"
        SELECT
            rsr.winner_player_id,
            rsr.loser_player_id,
            rsr.upset_factor,
            rsr.completed_at,
            gs.winner_score,
            gs.loser_score,
            gs.round_name,
            gs.is_dq,
            gs.vod_url,
            gs.startgg_id   AS startgg_set_id,
            ge.name         AS event_name,
            gt.name         AS tournament_name,
            gt.slug         AS tournament_handle,
            wee.seed        AS winner_seed,
            lee.seed        AS loser_seed,
            wee.placement   AS winner_placement,
            lee.placement   AS loser_placement,
            gwp.handle      AS winner_handle,
            glp.handle      AS loser_handle,
            gph.name        AS phase_name,
            gpg.display_identifier AS pool_identifier,
            gt.city,
            gt.addr_state,
            gt.country_code,
            gt.online AS "online!: bool",
            ge.num_entrants,
            ge.slug         AS event_handle
        FROM ranking_set_results rsr
        JOIN global_sets gs        ON gs.id  = rsr.global_set_id
        JOIN global_events ge      ON ge.id  = rsr.global_event_id
        JOIN global_tournaments gt ON gt.id  = ge.tournament_id
        JOIN global_players gwp    ON gwp.id = gs.winner_player_id
        JOIN global_players glp    ON glp.id = gs.loser_player_id
        LEFT JOIN global_event_entries wee ON wee.event_id = ge.id AND wee.player_id = gwp.id
        LEFT JOIN global_event_entries lee ON lee.event_id = ge.id AND lee.player_id = glp.id
        LEFT JOIN global_phase_groups gpg ON gpg.id = gs.phase_group_id
        LEFT JOIN global_phases gph       ON gph.id = gpg.phase_id
        WHERE rsr.ranking_id = $1
          AND (rsr.winner_player_id = $2 OR rsr.loser_player_id = $2)
        ORDER BY rsr.completed_at DESC NULLS LAST
        "#,
        path.rid,
        path.player_id,
    )
    .fetch_all(&state.db)
    .await?;

    let mut wins: Vec<SetRecord> = Vec::new();
    let mut losses: Vec<SetRecord> = Vec::new();

    for row in rows {
        let uf = match (row.winner_seed, row.loser_seed) {
            (Some(ws), Some(ls)) => set_upset_factor(ws, ls) as i64,
            _ => 0,
        };
        let location = compute_location(
            row.online,
            row.city.as_deref(),
            row.addr_state.as_deref(),
            row.country_code.as_deref(),
        );
        let rec = |opponent_id: Option<Uuid>, opponent_name: String| SetRecord {
            opponent_id,
            opponent_name,
            upset_factor: uf,
            winner_score: row.winner_score,
            loser_score: row.loser_score,
            tournament_name: row.tournament_name.clone(),
            tournament_handle: row.tournament_handle.clone(),
            event_name: row.event_name.clone(),
            round_name: row.round_name.clone(),
            completed_at: row.completed_at,
            is_dq: row.is_dq,
            vod_url: row.vod_url.clone(),
            startgg_set_id: row.startgg_set_id,
            winner_seed: row.winner_seed,
            loser_seed: row.loser_seed,
            phase_name: row.phase_name.clone(),
            pool_identifier: row.pool_identifier.clone(),
            winner_placement: row.winner_placement,
            loser_placement: row.loser_placement,
            location,
            num_entrants: row.num_entrants,
            event_handle: row.event_handle.clone(),
        };
        if row.winner_player_id == path.player_id {
            wins.push(rec(Some(row.loser_player_id), row.loser_handle));
        } else {
            losses.push(rec(Some(row.winner_player_id), row.winner_handle));
        }
    }

    wins.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));
    losses.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));

    Ok(Json(PlayerStatsResponse {
        player_id: path.player_id,
        name,
        wins,
        losses,
    }))
}

#[derive(Serialize)]
pub struct TournamentAttendance {
    pub tournament_name: String,
    pub tournament_slug: String,
    pub event_name: String,
    pub placement: Option<i32>,
    pub num_entrants: Option<i32>,
    pub start_at: Option<DateTime<Utc>>,
    pub location: Option<String>,
}

pub async fn get_player_tournaments(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path((project_id, player_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse> {
    use crate::routes::projects::require_project_read_access;
    require_project_read_access(&state.db, project_id, user.map(|u| u.id)).await?;

    let exists: Option<Uuid> = sqlx::query_scalar!(
        "SELECT id FROM players WHERE id = $1 AND project_id = $2",
        player_id,
        project_id,
    )
    .fetch_optional(&state.db)
    .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    struct Row {
        tournament_id: Uuid,
        tournament_startgg_id: i64,
        tournament_name: String,
        tournament_handle: String,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        online: bool,
        start_at: Option<DateTime<Utc>>,
        end_at: Option<DateTime<Utc>>,
        num_attendees: Option<i32>,
        event_id: Uuid,
        event_name: String,
        num_entrants: Option<i32>,
        placement: Option<i32>,
        seed: Option<i32>,
    }

    let rows = sqlx::query_as!(
        Row,
        r#"
        SELECT DISTINCT
            gt.id           AS tournament_id,
            gt.startgg_id   AS tournament_startgg_id,
            gt.name         AS tournament_name,
            gt.slug         AS tournament_handle,
            gt.city,
            gt.addr_state,
            gt.country_code,
            gt.online       AS "online!: bool",
            gt.start_at,
            gt.end_at,
            gt.num_attendees,
            ge.id           AS event_id,
            ge.name         AS event_name,
            ge.num_entrants,
            gee.placement,
            gee.seed
        FROM global_event_entries gee
        JOIN global_events ge          ON ge.id  = gee.event_id
        JOIN global_tournaments gt     ON gt.id  = ge.tournament_id
        JOIN global_players gp         ON gp.id  = gee.player_id
        JOIN startgg_accounts sa       ON sa.startgg_user_id = gp.startgg_user_id
        WHERE sa.player_id = $1
          AND EXISTS (
              SELECT 1 FROM project_events pe
              JOIN projects pr ON pr.id = pe.project_id
              WHERE pe.global_event_id = ge.id
                AND pr.id = $2
          )
        ORDER BY gt.start_at DESC NULLS LAST
        "#,
        player_id,
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<TournamentAttendance> = rows
        .into_iter()
        .map(|r| TournamentAttendance {
            tournament_name: r.tournament_name,
            tournament_slug: r.tournament_handle,
            event_name: r.event_name,
            placement: r.placement,
            num_entrants: r.num_entrants,
            start_at: r.start_at,
            location: compute_location(
                r.online,
                r.city.as_deref(),
                r.addr_state.as_deref(),
                r.country_code.as_deref(),
            ),
        })
        .collect();

    Ok(Json(resp))
}

pub async fn get_head_to_head(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    struct H2HRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        count: i64,
    }

    let rows = sqlx::query_as!(
        H2HRow,
        r#"
        SELECT
            winner_player_id AS "winner_player_id!: Uuid",
            loser_player_id  AS "loser_player_id!: Uuid",
            COUNT(*)         AS "count!: i64"
        FROM ranking_set_results
        WHERE ranking_id = $1
        GROUP BY winner_player_id, loser_player_id
        "#,
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    // Build wins map: (winner, loser) → count.
    let mut wins: HashMap<(Uuid, Uuid), i64> = HashMap::new();
    let mut pairs: std::collections::HashSet<(Uuid, Uuid)> = std::collections::HashSet::new();

    for row in rows {
        wins.insert((row.winner_player_id, row.loser_player_id), row.count);
        // Canonical pair: smaller UUID first, so each pair is visited once.
        let pair = if row.winner_player_id < row.loser_player_id {
            (row.winner_player_id, row.loser_player_id)
        } else {
            (row.loser_player_id, row.winner_player_id)
        };
        pairs.insert(pair);
    }

    let mut resp: Vec<HeadToHeadEntry> = Vec::with_capacity(pairs.len() * 2);
    for (a, b) in pairs {
        let a_wins = *wins.get(&(a, b)).unwrap_or(&0);
        let b_wins = *wins.get(&(b, a)).unwrap_or(&0);
        resp.push(HeadToHeadEntry {
            player_id: a,
            opponent_id: b,
            wins: a_wins,
            losses: b_wins,
        });
        resp.push(HeadToHeadEntry {
            player_id: b,
            opponent_id: a,
            wins: b_wins,
            losses: a_wins,
        });
    }

    // Stable sort: player_id, then opponent_id.
    resp.sort_by(|x, y| {
        x.player_id
            .cmp(&y.player_id)
            .then(x.opponent_id.cmp(&y.opponent_id))
    });

    Ok(Json(resp))
}

pub async fn get_h2h_sets(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingH2HPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    struct H2HSetRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        upset_factor: Option<f64>,
        completed_at: Option<DateTime<Utc>>,
        winner_score: Option<i16>,
        loser_score: Option<i16>,
        round_name: Option<String>,
        is_dq: bool,
        vod_url: Option<String>,
        startgg_set_id: i64,
        event_name: String,
        tournament_name: String,
        tournament_handle: String,
        winner_seed: Option<i32>,
        loser_seed: Option<i32>,
        winner_placement: Option<i32>,
        loser_placement: Option<i32>,
        winner_handle: String,
        loser_handle: String,
        phase_name: Option<String>,
        pool_identifier: Option<String>,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        online: bool,
        num_entrants: Option<i32>,
        event_handle: Option<String>,
    }

    let rows = sqlx::query_as!(
        H2HSetRow,
        r#"
        SELECT
            rsr.winner_player_id,
            rsr.loser_player_id,
            rsr.upset_factor,
            rsr.completed_at,
            gs.winner_score,
            gs.loser_score,
            gs.round_name,
            gs.is_dq,
            gs.vod_url,
            gs.startgg_id   AS startgg_set_id,
            ge.name         AS event_name,
            gt.name         AS tournament_name,
            gt.slug         AS tournament_handle,
            wee.seed        AS winner_seed,
            lee.seed        AS loser_seed,
            wee.placement   AS winner_placement,
            lee.placement   AS loser_placement,
            gwp.handle      AS winner_handle,
            glp.handle      AS loser_handle,
            gph.name        AS phase_name,
            gpg.display_identifier AS pool_identifier,
            gt.city,
            gt.addr_state,
            gt.country_code,
            gt.online AS "online!: bool",
            ge.num_entrants,
            ge.slug         AS event_handle
        FROM ranking_set_results rsr
        JOIN global_sets gs        ON gs.id  = rsr.global_set_id
        JOIN global_events ge      ON ge.id  = rsr.global_event_id
        JOIN global_tournaments gt ON gt.id  = ge.tournament_id
        JOIN global_players gwp    ON gwp.id = gs.winner_player_id
        JOIN global_players glp    ON glp.id = gs.loser_player_id
        LEFT JOIN global_event_entries wee ON wee.event_id = ge.id AND wee.player_id = gwp.id
        LEFT JOIN global_event_entries lee ON lee.event_id = ge.id AND lee.player_id = glp.id
        LEFT JOIN global_phase_groups gpg ON gpg.id = gs.phase_group_id
        LEFT JOIN global_phases gph       ON gph.id = gpg.phase_id
        WHERE rsr.ranking_id = $1
          AND (
              (rsr.winner_player_id = $2 AND rsr.loser_player_id = $3)
           OR (rsr.winner_player_id = $3 AND rsr.loser_player_id = $2)
          )
        ORDER BY rsr.completed_at DESC NULLS LAST
        "#,
        path.rid,
        path.pid_a,
        path.pid_b,
    )
    .fetch_all(&state.db)
    .await?;

    let sets: Vec<H2HSet> = rows
        .into_iter()
        .map(|row| {
            let uf = match (row.winner_seed, row.loser_seed) {
                (Some(ws), Some(ls)) => set_upset_factor(ws, ls) as i64,
                _ => 0,
            };
            let is_win = row.winner_player_id == path.pid_a;
            let (opponent_id, opponent_name) = if is_win {
                (Some(row.loser_player_id), row.loser_handle)
            } else {
                (Some(row.winner_player_id), row.winner_handle)
            };
            let location = compute_location(
                row.online,
                row.city.as_deref(),
                row.addr_state.as_deref(),
                row.country_code.as_deref(),
            );
            H2HSet {
                is_win,
                set: SetRecord {
                    opponent_id,
                    opponent_name,
                    upset_factor: uf,
                    winner_score: row.winner_score,
                    loser_score: row.loser_score,
                    tournament_name: row.tournament_name,
                    tournament_handle: row.tournament_handle,
                    event_name: row.event_name,
                    round_name: row.round_name,
                    completed_at: row.completed_at,
                    is_dq: row.is_dq,
                    vod_url: row.vod_url,
                    startgg_set_id: row.startgg_set_id,
                    winner_seed: row.winner_seed,
                    loser_seed: row.loser_seed,
                    phase_name: row.phase_name,
                    pool_identifier: row.pool_identifier,
                    winner_placement: row.winner_placement,
                    loser_placement: row.loser_placement,
                    location,
                    num_entrants: row.num_entrants,
                    event_handle: row.event_handle,
                },
            }
        })
        .collect();

    Ok(Json(sets))
}

pub async fn delete_tournament(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((project_id, tournament_startgg_id)): Path<(Uuid, i64)>,
) -> Result<impl IntoResponse> {
    use crate::routes::projects::require_project_access;
    require_project_access(&state.db, project_id, user.id, UserRole::Editor).await?;

    let gt = sqlx::query!(
        "SELECT id FROM global_tournaments WHERE startgg_id = $1",
        tournament_startgg_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    sqlx::query!(
        "DELETE FROM project_events
         WHERE project_id = $1
           AND global_event_id IN (SELECT id FROM global_events WHERE tournament_id = $2)",
        project_id,
        gt.id,
    )
    .execute(&state.db)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM ranking_events re
        USING rankings r
        WHERE re.ranking_id = r.id
          AND r.project_id = $1
          AND re.global_event_id IN (SELECT id FROM global_events WHERE tournament_id = $2)
        "#,
        project_id,
        gt.id,
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_ranking_player_tournaments(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPlayerStatPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    let exists: Option<Uuid> = sqlx::query_scalar!(
        "SELECT player_id FROM ranking_players WHERE ranking_id = $1 AND player_id = $2",
        path.rid,
        path.player_id,
    )
    .fetch_optional(&state.db)
    .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    struct Row {
        tournament_id: Uuid,
        tournament_startgg_id: i64,
        tournament_name: String,
        tournament_handle: String,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        online: bool,
        start_at: Option<DateTime<Utc>>,
        end_at: Option<DateTime<Utc>>,
        num_attendees: Option<i32>,
        event_id: Uuid,
        event_name: String,
        num_entrants: Option<i32>,
        placement: Option<i32>,
        seed: Option<i32>,
    }

    let rows = sqlx::query_as!(
        Row,
        r#"
        SELECT DISTINCT
            gt.id           AS tournament_id,
            gt.startgg_id   AS tournament_startgg_id,
            gt.name         AS tournament_name,
            gt.slug         AS tournament_handle,
            gt.city,
            gt.addr_state,
            gt.country_code,
            gt.online       AS "online!: bool",
            gt.start_at,
            gt.end_at,
            gt.num_attendees,
            ge.id           AS event_id,
            ge.name         AS event_name,
            ge.num_entrants,
            gee.placement,
            gee.seed
        FROM global_event_entries gee
        JOIN global_events ge          ON ge.id  = gee.event_id
        JOIN global_tournaments gt     ON gt.id  = ge.tournament_id
        JOIN global_players gp         ON gp.id  = gee.player_id
        JOIN startgg_accounts sa       ON sa.startgg_user_id = gp.startgg_user_id
        WHERE sa.player_id = $1
          AND EXISTS (
              SELECT 1 FROM ranking_events re
              WHERE re.global_event_id = ge.id
                AND re.ranking_id = $2
                AND re.included = true
          )
        ORDER BY gt.start_at DESC NULLS LAST
        "#,
        path.player_id,
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    let resp: Vec<TournamentAttendance> = rows
        .into_iter()
        .map(|r| TournamentAttendance {
            tournament_name: r.tournament_name,
            tournament_slug: r.tournament_handle,
            event_name: r.event_name,
            placement: r.placement,
            num_entrants: r.num_entrants,
            start_at: r.start_at,
            location: compute_location(
                r.online,
                r.city.as_deref(),
                r.addr_state.as_deref(),
                r.country_code.as_deref(),
            ),
        })
        .collect();

    Ok(Json(resp))
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tournaments", get(list_tournaments))
        .route("/events", axum::routing::put(put_events))
        .route("/stats", get(get_stats))
        .route("/stats/{player_id}", get(get_player_stats))
        .route(
            "/players/{player_id}/tournaments",
            get(get_ranking_player_tournaments),
        )
        .route("/head-to-head", get(get_head_to_head))
        .route("/head-to-head/{pid_a}/{pid_b}/sets", get(get_h2h_sets))
}
