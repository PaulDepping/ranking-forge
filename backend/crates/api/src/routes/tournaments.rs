use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    routes::auth::AuthUser,
    routes::projects::require_project,
    state::AppState,
};
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
    pub opponent_id: Uuid,
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
pub struct ProjectEventPath {
    pub id: Uuid,
    pub eid: Uuid,
}

#[derive(Deserialize)]
pub struct H2HSetPath {
    pub id: Uuid,
    pub pid_a: Uuid,
    pub pid_b: Uuid,
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
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

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
            t.id            AS tournament_id,
            t.startgg_id    AS tournament_startgg_id,
            t.name          AS tournament_name,
            t.handle        AS tournament_handle,
            t.city,
            t.addr_state,
            t.country_code,
            t.venue_name,
            t.online,
            t.start_at      AS tournament_start_at,
            t.end_at,
            e.id            AS event_id,
            e.startgg_id    AS event_startgg_id,
            e.name          AS event_name,
            e.game_name,
            e.num_entrants,
            e.start_at      AS event_start_at,
            pe.included,
            e.event_type,
            ARRAY(
                SELECT p.bracket_type
                FROM phases p
                WHERE p.event_id = e.id
                  AND p.bracket_type IS NOT NULL
                ORDER BY p.phase_order ASC NULLS LAST
            )               AS "bracket_types!: Vec<String>"
        FROM project_events pe
        JOIN events      e ON e.id = pe.event_id
        JOIN tournaments t ON t.id = e.tournament_id
        WHERE pe.project_id = $1
        ORDER BY t.start_at DESC NULLS LAST, t.name ASC, e.name ASC
        "#,
        project_id,
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
pub struct PatchEventBody {
    pub included: bool,
}

pub async fn patch_event(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<ProjectEventPath>,
    Json(body): Json<PatchEventBody>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, path.id, user.id).await?;

    // Verify the event belongs to this project.
    sqlx::query!(
        "SELECT project_id FROM project_events WHERE project_id = $1 AND event_id = $2",
        path.id,
        path.eid,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    sqlx::query!(
        r#"
        INSERT INTO project_events (project_id, event_id, included)
        VALUES ($1, $2, $3)
        ON CONFLICT (project_id, event_id) DO UPDATE SET included = EXCLUDED.included
        "#,
        path.id,
        path.eid,
        body.included,
    )
    .execute(&state.db)
    .await?;

    // Return the full event response.
    #[derive(Debug)]
    struct EventRow {
        id: Uuid,
        startgg_id: i64,
        name: String,
        game_name: Option<String>,
        num_entrants: Option<i32>,
        start_at: Option<DateTime<Utc>>,
        included: bool,
        event_type: Option<i32>,
        bracket_types: Vec<String>,
    }

    let ev = sqlx::query_as!(
        EventRow,
        r#"
        SELECT e.id, e.startgg_id, e.name, e.game_name, e.num_entrants,
               e.start_at, pe.included, e.event_type,
               ARRAY(
                   SELECT p.bracket_type
                   FROM phases p
                   WHERE p.event_id = e.id
                     AND p.bracket_type IS NOT NULL
                   ORDER BY p.phase_order ASC NULLS LAST
               ) AS "bracket_types!: Vec<String>"
        FROM events e
        JOIN project_events pe ON pe.event_id = e.id AND pe.project_id = $1
        WHERE e.id = $2
        "#,
        path.id,
        path.eid,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ProjectEventResponse {
        id: ev.id,
        startgg_id: ev.startgg_id,
        name: ev.name,
        game_name: ev.game_name,
        num_entrants: ev.num_entrants,
        start_at: ev.start_at,
        included: ev.included,
        event_type: ev.event_type,
        bracket_types: ev.bracket_types,
    }))
}

pub async fn get_stats(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

    struct PlayerRow {
        id: Uuid,
        name: String,
    }

    let players = sqlx::query_as!(
        PlayerRow,
        "SELECT id, name FROM players WHERE project_id = $1 ORDER BY created_at ASC",
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    struct SetRow {
        winner_player_id: Option<Uuid>,
        winner_name: String,
        winner_seed: Option<i32>,
        winner_entrant_id: Uuid,
        loser_player_id: Option<Uuid>,
        loser_name: String,
        loser_seed: Option<i32>,
        loser_entrant_id: Uuid,
        winner_score: Option<i16>,
        loser_score: Option<i16>,
        event_name: String,
        tournament_name: String,
        tournament_handle: String,
        round_name: Option<String>,
        completed_at: Option<DateTime<Utc>>,
        is_dq: bool,
        vod_url: Option<String>,
        startgg_set_id: i64,
        phase_name: Option<String>,
        pool_identifier: Option<String>,
        winner_placement: Option<i32>,
        loser_placement: Option<i32>,
        num_entrants: Option<i32>,
        online: bool,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        event_handle: Option<String>,
    }

    let sets = sqlx::query_as!(
        SetRow,
        r#"
        SELECT
            we.player_id                       AS "winner_player_id?: Uuid",
            COALESCE(wp.name, we.display_name) AS "winner_name!",
            we.seed                            AS winner_seed,
            we.id                              AS winner_entrant_id,
            le.player_id                       AS "loser_player_id?: Uuid",
            COALESCE(lp.name, le.display_name) AS "loser_name!",
            le.seed                            AS loser_seed,
            le.id                              AS loser_entrant_id,
            s.winner_score,
            s.loser_score,
            e.name                             AS event_name,
            t.name                             AS tournament_name,
            t.handle                           AS tournament_handle,
            s.round_name,
            s.completed_at,
            s.is_dq,
            s.vod_url,
            s.startgg_set_id,
            ph.name                            AS "phase_name?: String",
            pg.display_identifier              AS "pool_identifier?: String",
            we.final_placement                 AS winner_placement,
            le.final_placement                 AS loser_placement,
            e.num_entrants,
            t.online,
            t.city,
            t.addr_state,
            t.country_code,
            e.handle                           AS "event_handle?: String"
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        LEFT JOIN players wp ON wp.id = we.player_id AND wp.project_id = $1
        LEFT JOIN players lp ON lp.id = le.player_id AND lp.project_id = $1
        JOIN project_events pe ON pe.event_id = s.event_id AND pe.project_id = $1
        JOIN events e ON e.id = s.event_id
        JOIN tournaments t ON t.id = e.tournament_id
        LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
        LEFT JOIN phases ph ON ph.id = pg.phase_id
        WHERE pe.included = true
          AND s.is_dq    = false
          AND s.has_placeholder = false
          AND (wp.id IS NOT NULL OR lp.id IS NOT NULL)
        "#,
        project_id,
    )
    .fetch_all(&state.db)
    .await?;

    // Initialise per-player accumulators for every player (including those with no sets).
    let mut stats: HashMap<Uuid, (String, Vec<SetRecord>, Vec<SetRecord>)> = players
        .into_iter()
        .map(|p| (p.id, (p.name, Vec::new(), Vec::new())))
        .collect();

    for row in sets {
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
        let loser_opp_id = row.loser_player_id.unwrap_or(row.loser_entrant_id);
        let winner_opp_id = row.winner_player_id.unwrap_or(row.winner_entrant_id);

        if let Some(wp_id) = row.winner_player_id {
            if let Some(entry) = stats.get_mut(&wp_id) {
                entry.1.push(SetRecord {
                    opponent_id: loser_opp_id,
                    opponent_name: row.loser_name.clone(),
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
                });
            }
        }
        if let Some(lp_id) = row.loser_player_id {
            if let Some(entry) = stats.get_mut(&lp_id) {
                entry.2.push(SetRecord {
                    opponent_id: winner_opp_id,
                    opponent_name: row.winner_name,
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
                });
            }
        }
    }

    for entry in stats.values_mut() {
        entry.1.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));
        entry.2.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));
    }

    let mut resp: Vec<PlayerStatsResponse> = stats
        .into_iter()
        .map(|(id, (name, wins, losses))| PlayerStatsResponse {
            player_id: id,
            name,
            wins,
            losses,
        })
        .collect();

    resp.sort_by(|a, b| {
        let a_total = a.wins.len() + a.losses.len();
        let b_total = b.wins.len() + b.losses.len();
        let a_rate = if a_total == 0 {
            -1.0_f64
        } else {
            a.wins.len() as f64 / a_total as f64
        };
        let b_rate = if b_total == 0 {
            -1.0_f64
        } else {
            b.wins.len() as f64 / b_total as f64
        };
        b_rate
            .partial_cmp(&a_rate)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.wins.len().cmp(&a.wins.len()))
    });

    Ok(Json(resp))
}

pub async fn get_head_to_head(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

    struct H2HRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        count: i64,
    }

    let rows = sqlx::query_as!(
        H2HRow,
        r#"
        SELECT
            we.player_id AS "winner_player_id!: Uuid",
            le.player_id AS "loser_player_id!: Uuid",
            COUNT(*)     AS "count!: i64"
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN players  wp ON wp.id = we.player_id AND wp.project_id = $1
        JOIN players  lp ON lp.id = le.player_id AND lp.project_id = $1
        JOIN project_events pe ON pe.event_id = s.event_id AND pe.project_id = $1
        WHERE pe.included = true
          AND s.is_dq    = false
          AND s.has_placeholder = false
        GROUP BY we.player_id, le.player_id
        "#,
        project_id,
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
    AuthUser(user): AuthUser,
    Path(path): Path<H2HSetPath>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, path.id, user.id).await?;

    struct H2HSetRow {
        winner_player_id: Uuid,
        winner_name: String,
        winner_seed: Option<i32>,
        loser_player_id: Uuid,
        loser_name: String,
        loser_seed: Option<i32>,
        winner_score: Option<i16>,
        loser_score: Option<i16>,
        event_name: String,
        tournament_name: String,
        tournament_handle: String,
        round_name: Option<String>,
        completed_at: Option<DateTime<Utc>>,
        is_dq: bool,
        vod_url: Option<String>,
        startgg_set_id: i64,
        phase_name: Option<String>,
        pool_identifier: Option<String>,
        winner_placement: Option<i32>,
        loser_placement: Option<i32>,
        num_entrants: Option<i32>,
        online: bool,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        event_handle: Option<String>,
    }

    let rows = sqlx::query_as!(
        H2HSetRow,
        r#"
        SELECT
            we.player_id                       AS "winner_player_id!: Uuid",
            COALESCE(wp.name, we.display_name) AS "winner_name!",
            we.seed                            AS winner_seed,
            le.player_id                       AS "loser_player_id!: Uuid",
            COALESCE(lp.name, le.display_name) AS "loser_name!",
            le.seed                            AS loser_seed,
            s.winner_score,
            s.loser_score,
            e.name                             AS event_name,
            t.name                             AS tournament_name,
            t.handle                           AS tournament_handle,
            s.round_name,
            s.completed_at,
            s.is_dq,
            s.vod_url,
            s.startgg_set_id,
            ph.name                            AS "phase_name?: String",
            pg.display_identifier              AS "pool_identifier?: String",
            we.final_placement                 AS winner_placement,
            le.final_placement                 AS loser_placement,
            e.num_entrants,
            t.online,
            t.city,
            t.addr_state,
            t.country_code,
            e.handle                           AS "event_handle?: String"
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN players  wp ON wp.id = we.player_id AND wp.project_id = $1
        JOIN players  lp ON lp.id = le.player_id AND lp.project_id = $1
        JOIN events   e  ON e.id  = s.event_id
        JOIN tournaments t ON t.id = e.tournament_id
        JOIN project_events pe ON pe.event_id = s.event_id AND pe.project_id = $1
        LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
        LEFT JOIN phases ph ON ph.id = pg.phase_id
        WHERE pe.included = true
          AND s.is_dq = false
          AND s.has_placeholder = false
          AND (
              (we.player_id = $2 AND le.player_id = $3)
           OR (we.player_id = $3 AND le.player_id = $2)
          )
        ORDER BY s.completed_at DESC NULLS LAST
        "#,
        path.id,
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
                (row.loser_player_id, row.loser_name)
            } else {
                (row.winner_player_id, row.winner_name)
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

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, patch};
    axum::Router::new()
        .route("/tournaments", get(list_tournaments))
        .route("/events/{eid}", patch(patch_event))
        .route("/stats", get(get_stats))
        .route("/head-to-head", get(get_head_to_head))
        .route("/head-to-head/{pid_a}/{pid_b}/sets", get(get_h2h_sets))
}
