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
}

#[derive(Serialize)]
pub struct TournamentResponse {
    pub id: Uuid,
    pub startgg_id: i64,
    pub name: String,
    pub slug: String,
    pub city: Option<String>,
    pub addr_state: Option<String>,
    pub country_code: Option<String>,
    pub venue_name: Option<String>,
    pub online: bool,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub events: Vec<ProjectEventResponse>,
}

#[derive(Serialize)]
pub struct SetRecord {
    pub opponent_id: Uuid,
    pub opponent_name: String,
    pub upset_factor: i64,
    pub winner_score: Option<i16>,
    pub loser_score: Option<i16>,
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

// ── Handlers ──────────────────────────────────────────────────────────────────

pub async fn list_tournaments(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    require_project(&state.db, project_id, user.id).await?;

    struct Row {
        tournament_id: Uuid,
        tournament_startgg_id: i64,
        tournament_name: String,
        tournament_slug: String,
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
    }

    let rows = sqlx::query_as!(
        Row,
        r#"
        SELECT
            t.id            AS tournament_id,
            t.startgg_id    AS tournament_startgg_id,
            t.name          AS tournament_name,
            t.slug          AS tournament_slug,
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
            pe.included
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
                slug: row.tournament_slug.clone(),
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
    struct EventRow {
        id: Uuid,
        startgg_id: i64,
        name: String,
        game_name: Option<String>,
        num_entrants: Option<i32>,
        start_at: Option<DateTime<Utc>>,
        included: bool,
    }

    let ev = sqlx::query_as!(
        EventRow,
        r#"
        SELECT e.id, e.startgg_id, e.name, e.game_name, e.num_entrants,
               e.start_at, pe.included
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
            s.loser_score
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        LEFT JOIN players wp ON wp.id = we.player_id AND wp.project_id = $1
        LEFT JOIN players lp ON lp.id = le.player_id AND lp.project_id = $1
        JOIN project_events pe ON pe.event_id = s.event_id AND pe.project_id = $1
        WHERE pe.included = true
          AND s.is_dq    = false
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
                });
            }
        }
        if let Some(lp_id) = row.loser_player_id {
            if let Some(entry) = stats.get_mut(&lp_id) {
                entry.2.push(SetRecord {
                    opponent_id: winner_opp_id,
                    opponent_name: row.winner_name.clone(),
                    upset_factor: uf,
                    winner_score: row.winner_score,
                    loser_score: row.loser_score,
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
        let a_uf: i64 = a.wins.iter().map(|s| s.upset_factor).sum();
        let b_uf: i64 = b.wins.iter().map(|s| s.upset_factor).sum();
        b_uf.cmp(&a_uf).then(b.wins.len().cmp(&a.wins.len()))
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

// ── Router ────────────────────────────────────────────────────────────────────

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, patch};
    axum::Router::new()
        .route("/tournaments", get(list_tournaments))
        .route("/events/{eid}", patch(patch_event))
        .route("/stats", get(get_stats))
        .route("/head-to-head", get(get_head_to_head))
}
