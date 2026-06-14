#![cfg(feature = "live-tests")]

// Golden-dataset live integration tests for the Smash Hannover Weekly series.
//
// These tests call the real start.gg API. They are gated behind the `live-tests`
// feature flag and require STARTGG_API_KEY to be set in the environment.
//
// Run with:
//   DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
//   STARTGG_API_KEY=<your-key> \
//   SQLX_OFFLINE=true \
//   cargo test -p e2e --features live-tests

use api::{routes, state::AppState};
use axum::{Router, body::Body, http::Request, http::StatusCode};
use common::startgg::StartggClient;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// ── Golden dataset ────────────────────────────────────────────────────────────

const PLAYER1_SLUG: &str = "user/06b4042d"; // gamerTag: "King"
const PLAYER2_SLUG: &str = "user/54b7bbf3";

const WEEKLY_100_NAME: &str = "Smash Hannover Weekly #100";
const WEEKLY_100_HANDLE: &str = "smash-hannover-weekly-100";

const WEEKLY_88_NAME: &str = "Smash Hannover Weekly #88";
const WEEKLY_88_HANDLE: &str = "smash-hannover-weekly-88";

const WEEKLY_84_NAME: &str = "Smash Hannover Weekly #84";
const WEEKLY_84_HANDLE: &str = "smash-hannover-weekly-84";

// ── Event filtering constants (Melee Singles event IDs from golden dataset) ─────
const KEEP_EVENT_STARTGG_ID_88: i64 = 1514034; // Melee Singles at Smash Hannover Weekly #88
const KEEP_EVENT_STARTGG_ID_84: i64 = 1495126; // Melee Singles at Smash Hannover Weekly #84

// ── Helpers (mirrors full_flow.rs) ───────────────────────────────────────────

fn make_app(pool: PgPool, startgg_base_url: &str) -> Router {
    let state = AppState {
        db: pool,
        cors_origin: "http://localhost".to_string(),
        startgg_base_url: startgg_base_url.to_string(),
    };
    routes::router().with_state(state)
}

async fn read_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn register(app: &Router, username: &str, password: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"email": format!("{username}@test.com"), "display_name": username, "password": password}))
                .unwrap(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    format!("session_id={}", body["session_id"].as_str().unwrap())
}

async fn post_json(app: &Router, uri: &str, cookie: &str, body: Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_req(app: &Router, uri: &str, cookie: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn put_json(app: &Router, uri: &str, cookie: &str, body: Value) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(uri)
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn patch_json(
    app: &Router,
    uri: &str,
    cookie: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(uri)
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn set_startgg_api_key(pool: &PgPool, cookie: &str, api_key: &str) {
    let session_id: uuid::Uuid = cookie
        .split('=')
        .nth(1)
        .unwrap()
        .parse()
        .expect("invalid session UUID in cookie");
    sqlx::query!(
        "UPDATE users SET startgg_api_key = $1
         WHERE id = (SELECT user_id FROM sessions WHERE id = $2)",
        api_key,
        session_id,
    )
    .execute(pool)
    .await
    .expect("failed to set startgg_api_key");
}

async fn create_ranking(app: &Router, cookie: &str, project_id: &str, name: &str) -> String {
    let resp = post_json(
        app,
        &format!("/projects/{project_id}/rankings"),
        cookie,
        json!({"name": name}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    read_json(resp).await["id"].as_str().unwrap().to_string()
}

async fn add_player_to_ranking(
    app: &Router,
    cookie: &str,
    project_id: &str,
    ranking_id: &str,
    player_id: &str,
) {
    let resp = post_json(
        app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/players"),
        cookie,
        json!({"player_id": player_id}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
}

fn live_api_key() -> String {
    std::env::var("STARTGG_API_KEY").expect("STARTGG_API_KEY must be set to run live tests")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Golden-dataset test: import Smash Hannover Weekly #100.
///
/// Registers a user, creates a Melee project (game_id = 1), adds Player 1
/// (slug user/06b4042d / gamerTag "King"), links their start.gg account, then
/// runs the import worker against the real API.
///
/// Asserts:
/// - "Smash Hannover Weekly #100" appears in the project's tournament list
/// - at least one event with num_entrants > 0
#[sqlx::test(migrations = "../../migrations")]
async fn import_hannover_weekly_100(pool: PgPool) {
    let api_key = live_api_key();

    let startgg_client = StartggClient::new(api_key.clone());
    let app = make_app(pool.clone(), "https://api.start.gg/gql/alpha");

    let cookie = register(&app, "liveuser1", "pass1234").await;
    set_startgg_api_key(&pool, &cookie, &api_key).await;

    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({
            "name": "Hannover Melee PR",
            "game_id": 1,
            "game_name": "Super Smash Bros. Melee"
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project = read_json(resp).await;
    let project_id = project["id"].as_str().unwrap().to_string();

    // Add Player 1 and link their start.gg account
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "King"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let player1_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{player1_id}/accounts"),
        &cookie,
        json!({"handle": PLAYER1_SLUG}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create ranking and add player before import (so ranking_events rows are created)
    let ranking_id = create_ranking(&app, &cookie, &project_id, "Hannover PR").await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &player1_id).await;

    // Scope to ±8 days around the known startAt of Weekly #100 (2026-03-10).
    let project_uuid = Uuid::parse_str(&project_id).unwrap();
    worker::import::run(
        &pool,
        &startgg_client,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams {
            after_date: Some(1772469000),  // 2026-03-02
            before_date: Some(1773851400), // 2026-03-18
        },
    )
    .await
    .unwrap();

    // Assert: Weekly #100 appears in the tournament list
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    let tournaments = body.as_array().unwrap();

    let weekly_100 = tournaments
        .iter()
        .find(|t| t["name"] == WEEKLY_100_NAME)
        .unwrap_or_else(|| {
            let names: Vec<&str> = tournaments
                .iter()
                .filter_map(|t| t["name"].as_str())
                .collect();
            panic!(
                "Expected '{}' in tournament list, got: {:?}",
                WEEKLY_100_NAME, names
            );
        });

    // Verify the handle matches the golden dataset
    assert_eq!(
        weekly_100["handle"].as_str().unwrap_or(""),
        WEEKLY_100_HANDLE,
        "tournament handle did not match golden dataset"
    );

    // Assert: at least one event with num_entrants > 0
    let events = weekly_100["events"].as_array().unwrap();
    assert!(
        !events.is_empty(),
        "Weekly #100 should have at least one event"
    );
    let has_entrants = events
        .iter()
        .any(|e| e["num_entrants"].as_i64().unwrap_or(0) > 0);
    assert!(
        has_entrants,
        "At least one event in Weekly #100 should have num_entrants > 0, events: {:?}",
        events
    );
}

/// Golden-dataset test: import Smash Hannover Weekly #88 and #84.
///
/// Registers a user, creates a Melee project, adds both Player 1 (slug
/// user/06b4042d) and Player 2 (slug user/54b7bbf3), links their start.gg
/// accounts, then runs the import worker against the real API.
///
/// Asserts:
/// - "Smash Hannover Weekly #88" appears in the tournament list
/// - "Smash Hannover Weekly #84" appears in the tournament list
/// - Handles match golden constants
/// - All events except Melee Singles at #88 (startgg_id 1514034) and #84
///   (startgg_id 1495126) are excluded
#[sqlx::test(migrations = "../../migrations")]
async fn import_hannover_weekly_88_and_84(pool: PgPool) {
    let api_key = live_api_key();

    let startgg_client = StartggClient::new(api_key.clone());
    let app = make_app(pool.clone(), "https://api.start.gg/gql/alpha");

    let cookie = register(&app, "liveuser2", "pass1234").await;
    set_startgg_api_key(&pool, &cookie, &api_key).await;

    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({
            "name": "Hannover Melee PR 2-Player",
            "game_id": 1,
            "game_name": "Super Smash Bros. Melee"
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project = read_json(resp).await;
    let project_id = project["id"].as_str().unwrap().to_string();

    // Add Player 1
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "King"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let player1_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{player1_id}/accounts"),
        &cookie,
        json!({"handle": PLAYER1_SLUG}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Add Player 2
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Player2"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let player2_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{player2_id}/accounts"),
        &cookie,
        json!({"handle": PLAYER2_SLUG}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create ranking and add both players before import
    let ranking_id = create_ranking(&app, &cookie, &project_id, "Hannover PR 2P").await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &player1_id).await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &player2_id).await;

    // Scope to cover both #84 (2025-11-04) and #88 (2025-12-02) ±8 days each.
    let project_uuid = Uuid::parse_str(&project_id).unwrap();
    worker::import::run(
        &pool,
        &startgg_client,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams {
            after_date: Some(1761582600),  // 2025-10-27 (~8 days before #84)
            before_date: Some(1765384200), // 2025-12-10 (~8 days after #88)
        },
    )
    .await
    .unwrap();

    // Assert: Weekly #88 appears in the tournament list
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    let tournaments = body.as_array().unwrap();

    let tournament_names: Vec<&str> = tournaments
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(
        tournaments.iter().any(|t| t["name"] == WEEKLY_88_NAME),
        "Expected '{}' in tournament list, got: {:?}",
        WEEKLY_88_NAME,
        tournament_names
    );

    // Assert: Weekly #84 appears in the tournament list
    assert!(
        tournaments.iter().any(|t| t["name"] == WEEKLY_84_NAME),
        "Expected '{}' in tournament list, got: {:?}",
        WEEKLY_84_NAME,
        tournament_names
    );

    // Verify handles match the golden dataset
    let weekly_88 = tournaments
        .iter()
        .find(|t| t["name"] == WEEKLY_88_NAME)
        .unwrap();
    assert_eq!(
        weekly_88["handle"].as_str().unwrap_or(""),
        WEEKLY_88_HANDLE,
        "Weekly #88 handle did not match golden dataset"
    );

    let weekly_84 = tournaments
        .iter()
        .find(|t| t["name"] == WEEKLY_84_NAME)
        .unwrap();
    assert_eq!(
        weekly_84["handle"].as_str().unwrap_or(""),
        WEEKLY_84_HANDLE,
        "Weekly #84 handle did not match golden dataset"
    );

    // ── Filter: exclude all events except the two target Melee Singles events ──
    let keep_ids = [KEEP_EVENT_STARTGG_ID_88, KEEP_EVENT_STARTGG_ID_84];
    let mut exclusions: Vec<serde_json::Value> = Vec::new();
    for tournament in tournaments {
        let tournament_name = tournament["name"].as_str().unwrap_or("?");
        for event in tournament["events"].as_array().unwrap_or_else(|| {
            panic!(
                "tournament '{}' events field should be an array",
                tournament_name
            )
        }) {
            let startgg_id = event["startgg_id"].as_i64().unwrap_or(0);
            if !keep_ids.contains(&startgg_id) {
                let event_uuid = event["id"]
                    .as_str()
                    .expect("event id should be a string UUID");
                exclusions.push(json!({"event_id": event_uuid, "included": false}));
            }
        }
    }
    let ranking_uuid = Uuid::parse_str(&ranking_id).unwrap();
    let resp = put_json(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/events"),
        &cookie,
        json!(exclusions),
    )
    .await;
    assert!(
        resp.status().is_success(),
        "PUT /events returned {}",
        resp.status()
    );
    worker::compute::run(&pool, ranking_uuid).await.unwrap();

    // ── Stats ──────────────────────────────────────────────────────────────────
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/stats"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    let stats_arr = stats.as_array().unwrap();

    let king_stats = stats_arr
        .iter()
        .find(|s| s["player_id"] == json!(player1_id))
        .expect("King not found in stats");
    let player2_stats = stats_arr
        .iter()
        .find(|s| s["player_id"] == json!(player2_id))
        .expect("Player2 not found in stats");

    let king_wins = king_stats["wins"].as_array().unwrap();
    let king_losses = king_stats["losses"].as_array().unwrap();
    let p2_wins = player2_stats["wins"].as_array().unwrap();
    let p2_losses = player2_stats["losses"].as_array().unwrap();

    assert_eq!(king_wins.len(), 12, "king wins count");
    assert_eq!(king_losses.len(), 0, "king losses count");
    assert_eq!(p2_wins.len(), 7, "player2 wins count");
    assert_eq!(p2_losses.len(), 1, "player2 losses count");

    fn find_set<'a>(arr: &'a [Value], set_id: i64) -> &'a Value {
        arr.iter()
            .find(|s| s["startgg_set_id"] == json!(set_id))
            .unwrap_or_else(|| panic!("set {set_id} not found"))
    }

    // King wins at #88
    // All King wins at #88 are in Melee Singles, none are DQs
    for w in king_wins
        .iter()
        .filter(|w| w["tournament_name"] == json!(WEEKLY_88_NAME))
    {
        assert_eq!(
            w["event_name"],
            json!("Melee Singles"),
            "king #88 win event_name"
        );
        assert_eq!(w["is_dq"], json!(false), "king #88 win is_dq");
    }
    {
        let s = find_set(king_wins, 96986292);
        assert_eq!(s["opponent_name"], json!("Klinx"), "96986292 opponent_name");
        assert_eq!(s["upset_factor"], json!(-3_i64), "96986292 upset_factor");
        assert_eq!(s["round_name"], json!("Round 1"), "96986292 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "96986292 winner_seed");
        assert_eq!(s["loser_seed"], json!(5_i64), "96986292 loser_seed");
    }
    {
        let s = find_set(king_wins, 96986545);
        assert_eq!(s["opponent_name"], json!("Pompf"), "96986545 opponent_name");
        assert_eq!(s["upset_factor"], json!(-2_i64), "96986545 upset_factor");
        assert_eq!(s["round_name"], json!("Round 2"), "96986545 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "96986545 winner_seed");
        assert_eq!(s["loser_seed"], json!(4_i64), "96986545 loser_seed");
    }
    {
        let s = find_set(king_wins, 96986713);
        assert_eq!(
            s["opponent_name"],
            json!("Efisch"),
            "96986713 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-1_i64), "96986713 upset_factor");
        assert_eq!(s["round_name"], json!("Round 3"), "96986713 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "96986713 winner_seed");
        assert_eq!(s["loser_seed"], json!(2_i64), "96986713 loser_seed");
    }
    {
        let s = find_set(king_wins, 96987400);
        assert_eq!(
            s["opponent_name"],
            json!("Kometeisball"),
            "96987400 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-4_i64), "96987400 upset_factor");
        assert_eq!(s["round_name"], json!("Round 4"), "96987400 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "96987400 winner_seed");
        assert_eq!(s["loser_seed"], json!(7_i64), "96987400 loser_seed");
    }

    // King wins at #84
    // All King wins at #84 are in Melee Singles, none are DQs
    for w in king_wins
        .iter()
        .filter(|w| w["tournament_name"] == json!(WEEKLY_84_NAME))
    {
        assert_eq!(
            w["event_name"],
            json!("Melee Singles"),
            "king #84 win event_name"
        );
        assert_eq!(w["is_dq"], json!(false), "king #84 win is_dq");
    }
    {
        let s = find_set(king_wins, 95869986);
        assert_eq!(
            s["opponent_name"],
            json!("zyklop007"),
            "95869986 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-5_i64), "95869986 upset_factor");
        assert_eq!(s["round_name"], json!("Round 2"), "95869986 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "95869986 winner_seed");
        assert_eq!(s["loser_seed"], json!(9_i64), "95869986 loser_seed");
    }
    {
        let s = find_set(king_wins, 95869990);
        assert_eq!(
            s["opponent_name"],
            json!("Hektor"),
            "95869990 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-4_i64), "95869990 upset_factor");
        assert_eq!(s["round_name"], json!("Round 3"), "95869990 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "95869990 winner_seed");
        assert_eq!(s["loser_seed"], json!(8_i64), "95869990 loser_seed");
    }
    {
        let s = find_set(king_wins, 95869994);
        assert_eq!(
            s["opponent_name"],
            json!("Kometeisball"),
            "95869994 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-4_i64), "95869994 upset_factor");
        assert_eq!(s["round_name"], json!("Round 4"), "95869994 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "95869994 winner_seed");
        assert_eq!(s["loser_seed"], json!(7_i64), "95869994 loser_seed");
    }
    {
        let s = find_set(king_wins, 95869998);
        assert_eq!(
            s["opponent_name"],
            json!("Thought"),
            "95869998 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-3_i64), "95869998 upset_factor");
        assert_eq!(s["round_name"], json!("Round 5"), "95869998 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "95869998 winner_seed");
        assert_eq!(s["loser_seed"], json!(6_i64), "95869998 loser_seed");
    }
    {
        let s = find_set(king_wins, 95870002);
        assert_eq!(s["opponent_name"], json!("Marc"), "95870002 opponent_name");
        assert_eq!(s["upset_factor"], json!(-3_i64), "95870002 upset_factor");
        assert_eq!(s["round_name"], json!("Round 6"), "95870002 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "95870002 winner_seed");
        assert_eq!(s["loser_seed"], json!(5_i64), "95870002 loser_seed");
    }
    {
        let s = find_set(king_wins, 95870006);
        assert_eq!(
            s["opponent_name"],
            json!("Efisch"),
            "95870006 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-2_i64), "95870006 upset_factor");
        assert_eq!(s["round_name"], json!("Round 7"), "95870006 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "95870006 winner_seed");
        assert_eq!(s["loser_seed"], json!(4_i64), "95870006 loser_seed");
    }
    {
        let s = find_set(king_wins, 95870010);
        assert_eq!(
            s["opponent_name"],
            json!("Sabaca"),
            "95870010 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-2_i64), "95870010 upset_factor");
        assert_eq!(s["round_name"], json!("Round 8"), "95870010 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "95870010 winner_seed");
        assert_eq!(s["loser_seed"], json!(3_i64), "95870010 loser_seed");
    }
    {
        let s = find_set(king_wins, 95870014);
        assert_eq!(
            s["opponent_name"],
            json!("Player2"),
            "95870014 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-1_i64), "95870014 upset_factor");
        assert_eq!(s["round_name"], json!("Round 9"), "95870014 round_name");
        assert_eq!(s["winner_seed"], json!(1_i64), "95870014 winner_seed");
        assert_eq!(s["loser_seed"], json!(2_i64), "95870014 loser_seed");
    }

    // Player2 wins at #84
    // All Player2 wins at #84 are in Melee Singles, none are DQs
    for w in p2_wins.iter() {
        assert_eq!(w["event_name"], json!("Melee Singles"), "p2 win event_name");
        assert_eq!(
            w["tournament_name"],
            json!("Smash Hannover Weekly #84"),
            "p2 win tournament_name"
        );
        assert_eq!(w["is_dq"], json!(false), "p2 win is_dq");
    }
    // Player2 loss is at #84
    for l in p2_losses.iter() {
        assert_eq!(
            l["event_name"],
            json!("Melee Singles"),
            "p2 loss event_name"
        );
        assert_eq!(
            l["tournament_name"],
            json!("Smash Hannover Weekly #84"),
            "p2 loss tournament_name"
        );
        assert_eq!(l["is_dq"], json!(false), "p2 loss is_dq");
    }
    {
        let s = find_set(p2_wins, 95869982);
        assert_eq!(
            s["opponent_name"],
            json!("zyklop007"),
            "95869982 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-4_i64), "95869982 upset_factor");
        assert_eq!(s["round_name"], json!("Round 1"), "95869982 round_name");
        assert_eq!(s["winner_seed"], json!(2_i64), "95869982 winner_seed");
        assert_eq!(s["loser_seed"], json!(9_i64), "95869982 loser_seed");
    }
    {
        let s = find_set(p2_wins, 95869987);
        assert_eq!(
            s["opponent_name"],
            json!("Kometeisball"),
            "95869987 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-3_i64), "95869987 upset_factor");
        assert_eq!(s["round_name"], json!("Round 2"), "95869987 round_name");
        assert_eq!(s["winner_seed"], json!(2_i64), "95869987 winner_seed");
        assert_eq!(s["loser_seed"], json!(7_i64), "95869987 loser_seed");
    }
    {
        let s = find_set(p2_wins, 95869992);
        assert_eq!(s["opponent_name"], json!("Marc"), "95869992 opponent_name");
        assert_eq!(s["upset_factor"], json!(-2_i64), "95869992 upset_factor");
        assert_eq!(s["round_name"], json!("Round 3"), "95869992 round_name");
        assert_eq!(s["winner_seed"], json!(2_i64), "95869992 winner_seed");
        assert_eq!(s["loser_seed"], json!(5_i64), "95869992 loser_seed");
    }
    {
        let s = find_set(p2_wins, 95869997);
        assert_eq!(
            s["opponent_name"],
            json!("Sabaca"),
            "95869997 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-1_i64), "95869997 upset_factor");
        assert_eq!(s["round_name"], json!("Round 4"), "95869997 round_name");
        assert_eq!(s["winner_seed"], json!(2_i64), "95869997 winner_seed");
        assert_eq!(s["loser_seed"], json!(3_i64), "95869997 loser_seed");
    }
    {
        let s = find_set(p2_wins, 95870005);
        assert_eq!(
            s["opponent_name"],
            json!("Hektor"),
            "95870005 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-3_i64), "95870005 upset_factor");
        assert_eq!(s["round_name"], json!("Round 6"), "95870005 round_name");
        assert_eq!(s["winner_seed"], json!(2_i64), "95870005 winner_seed");
        assert_eq!(s["loser_seed"], json!(8_i64), "95870005 loser_seed");
    }
    {
        let s = find_set(p2_wins, 95870008);
        assert_eq!(
            s["opponent_name"],
            json!("Thought"),
            "95870008 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-2_i64), "95870008 upset_factor");
        assert_eq!(s["round_name"], json!("Round 7"), "95870008 round_name");
        assert_eq!(s["winner_seed"], json!(2_i64), "95870008 winner_seed");
        assert_eq!(s["loser_seed"], json!(6_i64), "95870008 loser_seed");
    }
    {
        let s = find_set(p2_wins, 95870011);
        assert_eq!(
            s["opponent_name"],
            json!("Efisch"),
            "95870011 opponent_name"
        );
        assert_eq!(s["upset_factor"], json!(-1_i64), "95870011 upset_factor");
        assert_eq!(s["round_name"], json!("Round 8"), "95870011 round_name");
        assert_eq!(s["winner_seed"], json!(2_i64), "95870011 winner_seed");
        assert_eq!(s["loser_seed"], json!(4_i64), "95870011 loser_seed");
    }

    // Player2 loss at #84
    {
        let s = find_set(p2_losses, 95870014);
        assert_eq!(
            s["opponent_name"],
            json!("King"),
            "p2 loss 95870014 opponent_name"
        );
        assert_eq!(
            s["upset_factor"],
            json!(-1_i64),
            "p2 loss 95870014 upset_factor"
        );
        assert_eq!(
            s["round_name"],
            json!("Round 9"),
            "p2 loss 95870014 round_name"
        );
        assert_eq!(
            s["winner_seed"],
            json!(1_i64),
            "p2 loss 95870014 winner_seed"
        );
        assert_eq!(s["loser_seed"], json!(2_i64), "p2 loss 95870014 loser_seed");
    }

    // ── H2H summary ───────────────────────────────────────────────────────────
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/head-to-head"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h2h = read_json(resp).await;
    let h2h_arr = h2h.as_array().unwrap();

    let king_vs_p2 = h2h_arr
        .iter()
        .find(|e| e["player_id"] == json!(player1_id) && e["opponent_id"] == json!(player2_id))
        .expect("king→player2 entry missing from H2H summary");
    let p2_vs_king = h2h_arr
        .iter()
        .find(|e| e["player_id"] == json!(player2_id) && e["opponent_id"] == json!(player1_id))
        .expect("player2→king entry missing from H2H summary");

    assert_eq!(king_vs_p2["wins"], json!(1_i64), "king→p2 wins");
    assert_eq!(king_vs_p2["losses"], json!(0_i64), "king→p2 losses");
    assert_eq!(p2_vs_king["wins"], json!(0_i64), "p2→king wins");
    assert_eq!(p2_vs_king["losses"], json!(1_i64), "p2→king losses");

    // ── H2H sets drilldown ────────────────────────────────────────────────────
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/head-to-head/{player1_id}/{player2_id}/sets"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sets_body = read_json(resp).await;
    let sets_arr = sets_body.as_array().unwrap();

    assert_eq!(sets_arr.len(), 1, "H2H sets count");

    // sets_arr.len() == 1 asserted above; direct index is safe
    assert_eq!(sets_arr[0]["is_win"], json!(true), "sets[0] is_win");
    assert_eq!(
        sets_arr[0]["tournament_name"],
        json!("Smash Hannover Weekly #84"),
        "sets[0] tournament_name"
    );
    assert_eq!(
        sets_arr[0]["event_name"],
        json!("Melee Singles"),
        "sets[0] event_name"
    );
    assert_eq!(
        sets_arr[0]["round_name"],
        json!("Round 9"),
        "sets[0] round_name"
    );
    assert_eq!(
        sets_arr[0]["opponent_name"],
        json!("Player2"),
        "sets[0] opponent_name"
    );
}
