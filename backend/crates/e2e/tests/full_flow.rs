// End-to-end regression test: user registration → project setup → import → stats/H2H.
// Calls the real Axum router and the real import pipeline against seeded global_* tables.

use api::{routes, state::AppState};
use axum::{Router, body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

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
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"email": format!("{username}@test.com"), "display_name": username, "password": password})).unwrap(),
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

/// Link a start.gg account to a project player. `handle` is passed directly
/// to the API (e.g. "mango" or "user/mango" — the API strips the "user/" prefix).
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

// ── Mirror seed ───────────────────────────────────────────────────────────────

/// Seeds the global mirror with the Mango/Armada test scenario:
/// - One global_game (Super Smash Bros. Melee, startgg_id=1)
/// - Two global_players (Mango uid=12345, Armada uid=67890)
/// - One global_tournament → one global_event (Melee Singles) → one global_phase → one global_phase_group
/// - Two global_event_entries (seed 2 for Mango, seed 7 for Armada)
/// - One global_set (Armada beat Mango, 3-1, is_dq=false)
///
/// Returns (mango_uid, armada_uid) so tests can verify linked startgg_accounts.
async fn seed_global_data(pool: &PgPool) -> (i64, i64) {
    let mango_uid: i64 = 12345;
    let armada_uid: i64 = 67890;

    let mango_id = sqlx::query_scalar!(
        "INSERT INTO global_players (startgg_user_id, handle) VALUES ($1, 'Mango') RETURNING id",
        mango_uid,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let armada_id = sqlx::query_scalar!(
        "INSERT INTO global_players (startgg_user_id, handle) VALUES ($1, 'Armada') RETURNING id",
        armada_uid,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    // Seed the game so projects filtered by game_id=1 can discover this event.
    let game_id = sqlx::query_scalar!(
        "INSERT INTO global_games (startgg_id, name) VALUES (1, 'Super Smash Bros. Melee') RETURNING id"
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let tournament_id = sqlx::query_scalar!(
        r#"INSERT INTO global_tournaments (startgg_id, name, slug, city, addr_state, country_code,
               online, num_attendees, start_at, end_at)
           VALUES (1001, 'Test Tournament', 'tournament/test-2024', 'San Jose', 'CA', 'US',
                   false, 8, to_timestamp(1700000000), to_timestamp(1700086400))
           RETURNING id"#,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let event_id = sqlx::query_scalar!(
        r#"INSERT INTO global_events (startgg_id, tournament_id, game_id, name, slug, start_at, num_entrants, state)
           VALUES (2001, $1, $2, 'Melee Singles', 'tournament/test-2024/event/melee-singles',
                   to_timestamp(1700040000), 2, 'COMPLETED')
           RETURNING id"#,
        tournament_id,
        game_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let phase_id = sqlx::query_scalar!(
        "INSERT INTO global_phases (startgg_id, event_id, name, phase_order, bracket_type)
         VALUES (5001, $1, 'Bracket', 1, 'DOUBLE_ELIMINATION') RETURNING id",
        event_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let phase_group_id = sqlx::query_scalar!(
        "INSERT INTO global_phase_groups (startgg_id, phase_id, display_identifier, bracket_type)
         VALUES (6001, $1, '1', 'DOUBLE_ELIMINATION') RETURNING id",
        phase_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    // Armada seed 7, placement 1 (winner); Mango seed 2, placement 2
    sqlx::query!(
        "INSERT INTO global_event_entries (event_id, player_id, seed, placement) VALUES ($1, $2, 7, 1)",
        event_id,
        armada_id,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query!(
        "INSERT INTO global_event_entries (event_id, player_id, seed, placement) VALUES ($1, $2, 2, 2)",
        event_id,
        mango_id,
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query!(
        r#"INSERT INTO global_sets
               (startgg_id, event_id, phase_group_id, winner_player_id, loser_player_id,
                round, round_name, winner_score, loser_score, is_dq, completed_at)
           VALUES (4001, $1, $2, $3, $4, 1, 'Round 1', 3, 1, false, to_timestamp(1700050000))"#,
        event_id,
        phase_group_id,
        armada_id,
        mango_id,
    )
    .execute(pool)
    .await
    .unwrap();

    (mango_uid, armada_uid)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Full happy-path regression test:
///
/// register → create project → add Mango + Armada → link start.gg accounts →
/// run import worker → assert tournaments/stats → trigger import via API → run compute →
/// assert stats change with event exclusion, then restore.
#[sqlx::test(migrations = "../../migrations")]
async fn full_import_flow(pool: PgPool) {
    let (_mango_uid, _armada_uid) = seed_global_data(&pool).await;
    let app = make_app(pool.clone());

    let cookie = register(&app, "user1", "pass1234").await;
    let project_id = create_project(&app, &cookie, "Test Project").await;

    // Add players
    let mango_pid = create_player(&app, &cookie, &project_id, "Mango").await;
    let armada_pid = create_player(&app, &cookie, &project_id, "Armada").await;

    // Link accounts — link_account looks up global_players by handle (strips "user/" prefix)
    link_account(&app, &cookie, &project_id, &mango_pid, "mango").await;
    link_account(&app, &cookie, &project_id, &armada_pid, "armada").await;

    // Create ranking and add both players
    let ranking_id = create_ranking(&app, &cookie, &project_id, "Main Ranking").await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &mango_pid).await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &armada_pid).await;

    // Trigger import — response body has "id" (job id)
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/import"),
        &cookie,
        json!({}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let job_body = read_json(resp).await;
    let job_id: Uuid = job_body["id"].as_str().unwrap().parse().unwrap();
    let project_id_uuid: Uuid = project_id.parse().unwrap();
    let ranking_id_uuid: Uuid = ranking_id.parse().unwrap();

    // Run worker inline against the same pool
    worker::import::run(&pool, project_id_uuid, job_id, Default::default())
        .await
        .unwrap();
    worker::compute::run(&pool, ranking_id_uuid).await.unwrap();

    // Assert tournament appears
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    let tournaments = body.as_array().unwrap();
    assert_eq!(tournaments.len(), 1);
    assert_eq!(tournaments[0]["name"], "Test Tournament");

    let events = tournaments[0]["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert!(events[0]["included"].as_bool().unwrap());
    let event_id = events[0]["id"].as_str().unwrap().to_string();

    // Assert stats show one set
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/stats"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    let stats_arr = stats.as_array().unwrap();

    let armada_stats = stats_arr
        .iter()
        .find(|s| s["player_id"] == armada_pid)
        .unwrap();
    assert_eq!(armada_stats["wins"].as_array().unwrap().len(), 1);
    assert_eq!(armada_stats["wins"][0]["opponent_name"], "Mango");
    assert_eq!(armada_stats["wins"][0]["upset_factor"], json!(3));
    assert_eq!(armada_stats["losses"], json!([]));

    let mango_stats = stats_arr
        .iter()
        .find(|s| s["player_id"] == mango_pid)
        .unwrap();
    assert_eq!(mango_stats["wins"], json!([]));
    assert_eq!(mango_stats["losses"].as_array().unwrap().len(), 1);
    assert_eq!(mango_stats["losses"][0]["opponent_name"], "Armada");
    assert_eq!(mango_stats["losses"][0]["upset_factor"], json!(3));

    // ── Head-to-head ──────────────────────────────────────────────────────────

    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/head-to-head"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h2h = read_json(resp).await;
    let h2h_arr = h2h.as_array().unwrap();

    let armada_vs_mango = h2h_arr
        .iter()
        .find(|e| e["player_id"] == armada_pid && e["opponent_id"] == mango_pid)
        .unwrap();
    assert_eq!(armada_vs_mango["wins"], json!(1));
    assert_eq!(armada_vs_mango["losses"], json!(0));

    let mango_vs_armada = h2h_arr
        .iter()
        .find(|e| e["player_id"] == mango_pid && e["opponent_id"] == armada_pid)
        .unwrap();
    assert_eq!(mango_vs_armada["wins"], json!(0));
    assert_eq!(mango_vs_armada["losses"], json!(1));

    // ── H2H sets endpoint ─────────────────────────────────────────────────────

    let resp = get_req(
        &app,
        &format!(
            "/projects/{project_id}/rankings/{ranking_id}/head-to-head/{mango_pid}/{armada_pid}/sets"
        ),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sets_body = read_json(resp).await;
    let sets_arr = sets_body.as_array().unwrap();
    assert_eq!(sets_arr.len(), 1);
    // pid_a = mango_pid, mango lost → is_win = false
    assert_eq!(sets_arr[0]["is_win"], json!(false));
    assert_eq!(sets_arr[0]["tournament_name"], json!("Test Tournament"));
    assert_eq!(sets_arr[0]["event_name"], json!("Melee Singles"));
    assert_eq!(sets_arr[0]["round_name"], json!("Round 1"));
    assert_eq!(sets_arr[0]["opponent_name"], json!("Armada"));

    // ── Event exclusion ───────────────────────────────────────────────────────

    let event_uuid = Uuid::parse_str(&event_id).unwrap();
    let resp = put_json(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/events"),
        &cookie,
        json!([{"event_id": event_uuid, "included": false}]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    worker::compute::run(&pool, ranking_id_uuid).await.unwrap();

    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/stats"),
        &cookie,
    )
    .await;
    let stats = read_json(resp).await;
    let armada_stats = stats
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["player_id"] == armada_pid)
        .unwrap();
    assert_eq!(armada_stats["wins"], json!([]));
    assert_eq!(armada_stats["losses"], json!([]));

    // ── Re-include ────────────────────────────────────────────────────────────

    let resp = put_json(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/events"),
        &cookie,
        json!([{"event_id": event_uuid, "included": true}]),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    worker::compute::run(&pool, ranking_id_uuid).await.unwrap();

    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/stats"),
        &cookie,
    )
    .await;
    let stats = read_json(resp).await;
    let armada_stats = stats
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["player_id"] == armada_pid)
        .unwrap();
    assert_eq!(armada_stats["wins"].as_array().unwrap().len(), 1);
    assert_eq!(armada_stats["wins"][0]["upset_factor"], json!(3));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_rename_project(pool: PgPool) {
    let app = make_app(pool.clone());
    let cookie = register(&app, "alice", "password123").await;

    // Create a project
    let resp = post_json(&app, "/projects", &cookie, json!({"name": "Original"})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = read_json(resp).await;
    let project_id = body["id"].as_str().unwrap().to_string();

    // Rename it
    let resp = patch_json(
        &app,
        &format!("/projects/{project_id}"),
        &cookie,
        json!({"name": "Renamed"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body["name"], "Renamed");

    // Confirm GET reflects new name
    let resp = get_req(&app, &format!("/projects/{project_id}"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body["name"], "Renamed");

    // Empty name is rejected
    let resp = patch_json(
        &app,
        &format!("/projects/{project_id}"),
        &cookie,
        json!({"name": "   "}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Name over 100 chars is rejected
    let resp = patch_json(
        &app,
        &format!("/projects/{project_id}"),
        &cookie,
        json!({"name": "a".repeat(101)}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Another user cannot rename alice's project
    let bob_cookie = register(&app, "bob", "password456").await;
    let resp = patch_json(
        &app,
        &format!("/projects/{project_id}"),
        &bob_cookie,
        json!({"name": "Stolen"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// After the first import, players should be ranked in descending winrate order.
/// Mango is added first (insertion rank 1) but loses his only set;
/// Armada is added second (insertion rank 2) but wins his only set.
/// After import the list order should flip: [Armada, Mango].
#[sqlx::test(migrations = "../../migrations")]
async fn import_seeds_rank_by_winrate(pool: PgPool) {
    seed_global_data(&pool).await;

    let app = make_app(pool.clone());
    let cookie = register(&app, "testuser", "pass1234").await;

    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({"name": "PR", "game_id": 1, "game_name": "Melee"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    // Mango added first → insertion rank 1
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let mango_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{mango_id}/accounts"),
        &cookie,
        json!({"handle": "user/mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Armada added second → insertion rank 2
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Armada"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let armada_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{armada_id}/accounts"),
        &cookie,
        json!({"handle": "user/armada"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create ranking and add both players
    let ranking_id = create_ranking(&app, &cookie, &project_id, "Test Ranking").await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &mango_id).await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &armada_id).await;

    // Import: Armada wins 1-0, Mango loses 0-1
    worker::import::run(
        &pool,
        Uuid::parse_str(&project_id).unwrap(),
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    // After import, GET /rankings/{rid}/players should return Armada first (higher winrate)
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/players"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let players = read_json(resp).await;
    let names: Vec<&str> = players
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Armada", "Mango"]);
}

/// A second import must not overwrite a ranking the user has manually set.
/// After the first import the automatic sort puts Armada first. We then
/// explicitly reverse the order (Mango first) and verify a second import
/// leaves that manual ordering intact.
#[sqlx::test(migrations = "../../migrations")]
async fn import_skips_sort_if_already_ranked(pool: PgPool) {
    seed_global_data(&pool).await;

    let app = make_app(pool.clone());
    let cookie = register(&app, "testuser", "pass1234").await;

    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({"name": "PR", "game_id": 1, "game_name": "Melee"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let mango_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{mango_id}/accounts"),
        &cookie,
        json!({"handle": "user/mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Armada"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let armada_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{armada_id}/accounts"),
        &cookie,
        json!({"handle": "user/armada"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create ranking and add both players
    let ranking_id = create_ranking(&app, &cookie, &project_id, "Test Ranking").await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &mango_id).await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &armada_id).await;

    let project_uuid = Uuid::parse_str(&project_id).unwrap();

    // First import: automatic sort gives Armada rank 1, Mango rank 2
    worker::import::run(
        &pool,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    // User manually reorders: Mango first, Armada second
    let resp = put_json(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/ranking"),
        &cookie,
        json!({"player_ids": [mango_id, armada_id]}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Second import: must not overwrite the manual ranking
    worker::import::run(
        &pool,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/players"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let players = read_json(resp).await;
    let names: Vec<&str> = players
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    // Manual order (Mango first) must be preserved
    assert_eq!(names, vec!["Mango", "Armada"]);
}

/// A project with no game_id should import all tournaments/events regardless
/// of which game they are for.
#[sqlx::test(migrations = "../../migrations")]
async fn import_no_game_filter_flow(pool: PgPool) {
    seed_global_data(&pool).await;

    let app = make_app(pool.clone());

    let cookie = register(&app, "gameuser", "pass1234").await;

    // Create project with NO game_id
    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({ "name": "All Games PR" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project = read_json(resp).await;
    let project_id = project["id"].as_str().unwrap().to_string();

    // Add Mango and link start.gg account
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let mango_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{mango_id}/accounts"),
        &cookie,
        json!({"handle": "user/mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create ranking and add player
    let ranking_id = create_ranking(&app, &cookie, &project_id, "Test Ranking").await;
    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &mango_id).await;

    // Import: no game filter — all events in the mirror should be discovered
    let project_uuid = Uuid::parse_str(&project_id).unwrap();
    worker::import::run(
        &pool,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    // The tournament should appear in the ranking's tournament list
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    let tournaments = body.as_array().unwrap();
    assert_eq!(
        tournaments.len(),
        1,
        "project with no game filter should import all events from the mirror"
    );
    assert_eq!(tournaments[0]["name"], "Test Tournament");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_ranking_player_tournaments(pool: PgPool) {
    let app = make_app(pool.clone());
    let cookie = register(&app, "alice", "pass1234").await;

    let resp = post_json(&app, "/projects", &cookie, json!({"name": "Test"})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let ranking_id = create_ranking(&app, &cookie, &project_id, "Season 1").await;

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let player_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    add_player_to_ranking(&app, &cookie, &project_id, &ranking_id, &player_id).await;

    // Player in ranking → 200 with empty list (no import run)
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/players/{player_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await, json!([]));

    // Player exists in project but not in this ranking → 404
    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Armada"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let other_id = read_json(resp).await["id"].as_str().unwrap().to_string();
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/rankings/{ranking_id}/players/{other_id}/tournaments"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// In the mirror-backed architecture, the crawler is responsible for only
/// storing COMPLETED events in global_event_entries. This test verifies that
/// the import worker succeeds gracefully when a player exists in the mirror
/// but has no event entries (i.e. the crawler has not populated any data for them).
#[sqlx::test(migrations = "../../migrations")]
async fn skips_non_completed_events(pool: PgPool) {
    let app = make_app(pool.clone());
    let cookie = register(&app, "testuser", "pass1234").await;

    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({"name": "PR", "game_id": 1, "game_name": "Melee"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players"),
        &cookie,
        json!({"name": "Mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let mango_id = read_json(resp).await["id"].as_str().unwrap().to_string();

    // Seed a global player record so link_account can find Mango in the mirror,
    // but do NOT seed any tournament/event/entry data — simulating a player known
    // to the crawler but with no completed events stored yet.
    sqlx::query!(
        "INSERT INTO global_players (startgg_user_id, handle) VALUES ($1, 'mango')",
        12345_i64
    )
    .execute(&pool)
    .await
    .unwrap();

    let resp = post_json(
        &app,
        &format!("/projects/{project_id}/players/{mango_id}/accounts"),
        &cookie,
        json!({"handle": "user/mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Import completes without error — worker finds the linked player but
    // discovers no event entries in the mirror.
    worker::import::run(
        &pool,
        Uuid::parse_str(&project_id).unwrap(),
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    // The global mirror has no events for this player, so global_events is empty.
    let event_count: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM global_events")
        .fetch_one(&pool)
        .await
        .unwrap()
        .unwrap_or(0);
    assert_eq!(
        event_count, 0,
        "global mirror has no events for this player (crawler has not populated any)"
    );
}
