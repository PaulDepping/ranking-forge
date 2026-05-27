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

// ── Event filtering constants (from discover_hannover_stats Task 2 Step 3) ────
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
        &format!("/projects/{project_id}/tournaments"),
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
        &format!("/projects/{project_id}/tournaments"),
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
    for tournament in tournaments.as_array().unwrap() {
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
                let resp = patch_json(
                    &app,
                    &format!("/projects/{project_id}/events/{event_uuid}"),
                    &cookie,
                    json!({"included": false}),
                )
                .await;
                assert!(
                    resp.status().is_success(),
                    "PATCH event/{event_uuid} returned {}",
                    resp.status()
                );
            }
        }
    }
}

/// Temporary discovery test — run once with --nocapture to read off golden
/// event IDs and stats values, then delete this function after Task 4.
///
/// Run with:
///   DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
///   STARTGG_API_KEY=<your-key> \
///   SQLX_OFFLINE=true \
///   cargo test -p e2e --features live-tests -- discover_hannover_stats --nocapture
#[sqlx::test(migrations = "../../migrations")]
async fn discover_hannover_stats(pool: PgPool) {
    let api_key = live_api_key();
    let startgg_client = StartggClient::new(api_key.clone());
    let app = make_app(pool.clone(), "https://api.start.gg/gql/alpha");

    let cookie = register(&app, "discoveruser", "pass1234").await;
    set_startgg_api_key(&pool, &cookie, &api_key).await;

    // Create a Melee project
    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({
            "name": "Discover H2H Stats",
            "game_id": 1,
            "game_name": "Super Smash Bros. Melee"
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project = read_json(resp).await;
    let project_id = project["id"].as_str().unwrap().to_string();

    // Add Player 1 (King)
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "King"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let king_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{king_id}/accounts"),
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

    // Run import — same date window as import_hannover_weekly_88_and_84
    let project_uuid = Uuid::parse_str(&project_id).unwrap();
    worker::import::run(
        &pool,
        &startgg_client,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams {
            after_date: Some(1761582600),  // 2025-10-27
            before_date: Some(1765384200), // 2025-12-10
        },
    )
    .await
    .unwrap();

    // ── Print all events (to identify KEEP_EVENT_STARTGG_ID_* values) ─────────
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tournaments = read_json(resp).await;
    eprintln!("\n=== TOURNAMENTS & EVENTS ===");
    for t in tournaments.as_array().unwrap() {
        eprintln!(
            "Tournament: {}  (handle: {})",
            t["name"].as_str().unwrap_or("?"),
            t["handle"].as_str().unwrap_or("?")
        );
        for e in t["events"].as_array().unwrap() {
            eprintln!(
                "  event  startgg_id={}  name={:?}  included={}",
                e["startgg_id"],
                e["name"].as_str().unwrap_or("?"),
                e["included"]
            );
        }
    }

    // ── Print full stats ───────────────────────────────────────────────────────
    let resp = get_req(&app, &format!("/projects/{project_id}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    eprintln!(
        "\n=== STATS ===\n{}",
        serde_json::to_string_pretty(&stats).unwrap()
    );

    // ── Print H2H summary ──────────────────────────────────────────────────────
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/head-to-head"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h2h = read_json(resp).await;
    eprintln!(
        "\n=== H2H SUMMARY ===\n{}",
        serde_json::to_string_pretty(&h2h).unwrap()
    );

    // ── Print H2H sets drilldown ───────────────────────────────────────────────
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/head-to-head/{king_id}/{player2_id}/sets"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sets = read_json(resp).await;
    eprintln!(
        "\n=== H2H SETS ({king_id} vs {player2_id}) ===\n{}",
        serde_json::to_string_pretty(&sets).unwrap()
    );

    panic!(
        "discovery complete — review output above, fill in golden constants, \
         extend import_hannover_weekly_88_and_84, then delete this function"
    );
}
