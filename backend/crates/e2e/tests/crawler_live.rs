#![cfg(feature = "live-tests")]

// Live integration test: real start.gg API → crawler → global_* tables →
// worker import/compute → API stats/H2H assertions.
//
// Requires STARTGG_API_KEY in environment. Skips gracefully when absent.
//
// Run:
//   DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
//   STARTGG_API_KEY=<your-key> \
//   SQLX_OFFLINE=true \
//   cargo test -p e2e --features live-tests -- --nocapture

use std::sync::atomic::AtomicBool;

use api::{routes, state::AppState};
use axum::{Router, body::Body, http::Request, http::StatusCode};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// ── Golden dataset ────────────────────────────────────────────────────────────

const PLAYER1_SLUG: &str = "user/06b4042d"; // King
const PLAYER2_SLUG: &str = "user/54b7bbf3";

const MELEE_GAME_ID: u64 = 1;

// Smash Hannover Weekly #84 — 2025-11-04
// Smash Hannover Weekly #88 — 2025-12-02
const EVENT_DATES: [(i32, u32, u32); 2] = [(2025, 11, 4), (2025, 12, 2)];

// ── App factory ───────────────────────────────────────────────────────────────

fn make_app(pool: PgPool) -> Router {
    let state = AppState {
        db: pool,
        cors_origin: "http://localhost".to_string(),
    };
    routes::router().with_state(state)
}

// ── HTTP helpers ──────────────────────────────────────────────────────────────

async fn read_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn register(app: &Router, username: &str, password: &str) -> String {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({
                        "email": format!("{username}@test.com"),
                        "display_name": username,
                        "password": password
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body: Value = read_json(resp).await;
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

// ── Domain helpers ────────────────────────────────────────────────────────────

async fn create_project(app: &Router, cookie: &str, name: &str) -> String {
    let resp = post_json(app, "/projects", cookie, json!({"name": name})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    read_json(resp).await["id"].as_str().unwrap().to_string()
}

async fn create_player(app: &Router, cookie: &str, project_id: &str, name: &str) -> String {
    let resp = post_json(
        app,
        &format!("/projects/{project_id}/players"),
        cookie,
        json!({"name": name}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    read_json(resp).await["id"].as_str().unwrap().to_string()
}

async fn link_account(app: &Router, cookie: &str, project_id: &str, player_id: &str, handle: &str) {
    let resp = post_json(
        app,
        &format!("/projects/{project_id}/players/{player_id}/accounts"),
        cookie,
        json!({"handle": handle}),
    )
    .await;
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "link_account failed for {handle}"
    );
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

// ── Test ──────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn crawl_and_import_hannover_weekly(pool: PgPool) {
    let api_key = match std::env::var("STARTGG_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("STARTGG_API_KEY not set — skipping live test");
            return;
        }
    };

    // ── Step 1: Crawl real start.gg for two specific event days ──────────────

    let shutdown = AtomicBool::new(false);

    for (year, month, day) in EVENT_DATES {
        let date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
        let config = crawler::cli::Config {
            database_url: "unused".into(),
            startgg_api_key: api_key.clone(),
            from_date: date,
            to_date: date,
            window_days: 1,
            delay_ms: 250,
            sets_per_page: 20,
            game_id: Some(MELEE_GAME_ID),
            rust_log: "off".into(),
            startgg_base_url: None,
        };
        crawler::scraper::run(&config, &pool, &shutdown)
            .await
            .unwrap_or_else(|e| panic!("crawler failed for {year}-{month:02}-{day:02}: {e}"));
    }

    // ── Step 2: Project setup ─────────────────────────────────────────────────

    let app = make_app(pool.clone());
    let cookie = register(&app, "user1", "pass1234").await;
    let project_id = create_project(&app, &cookie, "Hannover Weeklies").await;

    let king_pid = create_player(&app, &cookie, &project_id, "King").await;
    let player2_pid = create_player(&app, &cookie, &project_id, "Player2").await;

    link_account(&app, &cookie, &project_id, &king_pid, PLAYER1_SLUG).await;
    link_account(&app, &cookie, &project_id, &player2_pid, PLAYER2_SLUG).await;

    let ranking_id = create_ranking(&app, &cookie, &project_id, "Main").await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &king_pid).await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &player2_pid).await;

    // ── Step 3: Import + compute ──────────────────────────────────────────────

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/import"),
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let job_id: Uuid = read_json(resp).await["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let project_id_uuid: Uuid = project_id.parse().unwrap();
    let ranking_id_uuid: Uuid = ranking_id.parse().unwrap();

    worker::import::run(&pool, project_id_uuid, job_id, Default::default())
        .await
        .unwrap();
    worker::compute::run(&pool, ranking_id_uuid).await.unwrap();

    // ── Step 4: Assertions ────────────────────────────────────────────────────

    // At least two tournaments appear in the ranking (one per crawled event day).
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tournaments = read_json(resp).await;
    let tournament_list = tournaments.as_array().unwrap();
    assert!(
        tournament_list.len() >= 2,
        "expected ≥2 tournaments, got {}: {tournaments}",
        tournament_list.len()
    );

    // King has at least one set result (wins or losses).
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/stats"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    let king_stats = stats
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["player_id"] == king_pid)
        .unwrap_or_else(|| panic!("King not found in stats: {stats}"));
    let total_sets = king_stats["wins"].as_array().unwrap().len()
        + king_stats["losses"].as_array().unwrap().len();
    assert!(
        total_sets > 0,
        "King should have at least one set result, got stats: {king_stats}"
    );

    // H2H grid is non-empty.
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/head-to-head"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h2h = read_json(resp).await;
    assert!(
        !h2h.as_array().unwrap().is_empty(),
        "expected non-empty H2H grid: {h2h}"
    );
}
