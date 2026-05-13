// Integration tests for the RankingForge API.
// Requires a running PostgreSQL server:
//   DATABASE_URL=postgres://rankingforge:rankingforge@localhost:5432/rankingforge cargo test

use api::{StartggClient, routes, state::AppState};
use axum::{Router, body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_app(pool: PgPool, startgg_base_url: &str) -> Router {
    let startgg = if startgg_base_url.is_empty() {
        StartggClient::new("test-key".into())
    } else {
        StartggClient::new_with_base_url("test-key".into(), startgg_base_url.into())
    };
    let state = AppState {
        db: pool,
        startgg,
        session_secret: "test-secret-key-at-least-32-bytes!!".to_string(),
        cors_origin: "http://localhost".to_string(),
    };
    routes::router().with_state(state)
}

async fn read_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Register a user and return the session cookie string ("session_id=<uuid>").
async fn register(app: &Router, username: &str, password: &str) -> String {
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": username, "password": password})).unwrap(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "register should return 201"
    );

    resp.headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
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

async fn delete_req(app: &Router, uri: &str, cookie: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
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

// ── DB seeding helpers ────────────────────────────────────────────────────────

async fn seed_tournament_event(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    startgg_tournament_id: i64,
    startgg_event_id: i64,
) -> (Uuid, Uuid) {
    let tournament_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO tournaments (startgg_id, name, slug, online)
         VALUES ($1, 'Test Tournament', 'test-tournament', false)
         RETURNING id",
        startgg_tournament_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let event_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO events (tournament_id, startgg_id, name)
         VALUES ($1, $2, 'Singles')
         RETURNING id",
        tournament_id,
        startgg_event_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO project_events (project_id, event_id, included) VALUES ($1, $2, true)",
        project_id,
        event_id,
    )
    .execute(pool)
    .await
    .unwrap();

    (tournament_id, event_id)
}

async fn seed_entrant(
    pool: &sqlx::PgPool,
    event_id: Uuid,
    player_id: Option<Uuid>,
    startgg_entrant_id: i64,
    seed: Option<i32>,
) -> Uuid {
    sqlx::query_scalar!(
        "INSERT INTO entrants (event_id, player_id, startgg_entrant_id, display_name, seed)
         VALUES ($1, $2, $3, 'Player', $4)
         RETURNING id",
        event_id,
        player_id,
        startgg_entrant_id,
        seed,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_entrant_named(
    pool: &sqlx::PgPool,
    event_id: Uuid,
    player_id: Option<Uuid>,
    startgg_entrant_id: i64,
    display_name: &str,
    seed: Option<i32>,
) -> Uuid {
    sqlx::query_scalar!(
        "INSERT INTO entrants (event_id, player_id, startgg_entrant_id, display_name, seed)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id",
        event_id,
        player_id,
        startgg_entrant_id,
        display_name,
        seed,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_set(
    pool: &sqlx::PgPool,
    event_id: Uuid,
    winner_entrant_id: Uuid,
    loser_entrant_id: Uuid,
    startgg_set_id: i64,
) {
    sqlx::query!(
        "INSERT INTO sets (event_id, startgg_set_id, winner_entrant_id, loser_entrant_id, is_dq)
         VALUES ($1, $2, $3, $4, false)",
        event_id,
        startgg_set_id,
        winner_entrant_id,
        loser_entrant_id,
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn seed_set_with_scores(
    pool: &sqlx::PgPool,
    event_id: Uuid,
    winner_entrant_id: Uuid,
    loser_entrant_id: Uuid,
    startgg_set_id: i64,
    winner_score: i16,
    loser_score: i16,
) {
    sqlx::query!(
        "INSERT INTO sets
             (event_id, startgg_set_id, winner_entrant_id, loser_entrant_id,
              is_dq, winner_score, loser_score)
         VALUES ($1, $2, $3, $4, false, $5, $6)",
        event_id,
        startgg_set_id,
        winner_entrant_id,
        loser_entrant_id,
        winner_score,
        loser_score,
    )
    .execute(pool)
    .await
    .unwrap();
}

/// Create a project for the given user and return its UUID string.
async fn create_project(app: &Router, cookie: &str) -> String {
    let resp = post_json(app, "/projects", cookie, json!({"name": "Test Project"})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    read_json(resp).await["id"].as_str().unwrap().to_string()
}

/// Create a player in the given project and return its UUID string.
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

fn startgg_user_ok(id: i64, name: &str) -> Value {
    json!({ "data": { "user": { "id": id, "name": name } } })
}

fn startgg_user_null() -> Value {
    json!({ "data": { "user": null } })
}

fn startgg_games_ok() -> Value {
    json!({
        "data": {
            "videogames": {
                "nodes": [
                    {"id": 1, "name": "Super Smash Bros. Melee", "displayName": "SSBM"},
                    {"id": 2, "name": "Super Smash Bros. Ultimate", "displayName": null}
                ]
            }
        }
    })
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn auth_register_and_me(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;

    let resp = get_req(&app, "/auth/me", &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await["username"], "alice");
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_me_unauthenticated(pool: PgPool) {
    let app = make_app(pool, "");
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_logout_invalidates_session(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "bob", "password123").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = get_req(&app, "/auth/me", &cookie).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_login(pool: PgPool) {
    let app = make_app(pool, "");
    register(&app, "alice", "password123").await;

    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": "alice", "password": "password123"})).unwrap(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Login must set a session cookie
    assert!(
        resp.headers().contains_key("set-cookie"),
        "login must set session cookie"
    );

    let cookie = resp
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string();

    // Cookie from login must work for authenticated endpoints
    let resp = get_req(&app, "/auth/me", &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await["username"], "alice");
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_login_wrong_password(pool: PgPool) {
    let app = make_app(pool, "");
    register(&app, "alice", "password123").await;

    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": "alice", "password": "wrongpassword"})).unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_login_unknown_user(pool: PgPool) {
    let app = make_app(pool, "");

    let req = Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": "nobody", "password": "password123"})).unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_register_short_username(pool: PgPool) {
    let app = make_app(pool, "");
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": "ab", "password": "password123"})).unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_register_short_password(pool: PgPool) {
    let app = make_app(pool, "");
    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": "alice", "password": "short"})).unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn auth_register_duplicate_username(pool: PgPool) {
    let app = make_app(pool, "");
    register(&app, "alice", "password123").await;

    let req = Request::builder()
        .method("POST")
        .uri("/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_vec(&json!({"username": "alice", "password": "different!"})).unwrap(),
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ── Projects ──────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn projects_list_empty(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;

    let resp = get_req(&app, "/projects", &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await, json!([]));
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_create_and_get(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;

    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({"name": "PR 2024", "game_id": 1, "game_name": "Melee"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = read_json(resp).await;
    assert_eq!(body["name"], "PR 2024");
    assert_eq!(body["game_id"], 1);
    assert_eq!(body["game_name"], "Melee");
    // user_id must NOT be exposed
    assert!(body.get("user_id").is_none());

    let id = body["id"].as_str().unwrap();
    let resp = get_req(&app, &format!("/projects/{id}"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await["name"], "PR 2024");
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_list_shows_only_own(pool: PgPool) {
    let app = make_app(pool, "");
    let alice = register(&app, "alice", "password123").await;
    let bob = register(&app, "bob", "password456").await;

    create_project(&app, &alice).await;
    create_project(&app, &alice).await;
    create_project(&app, &bob).await;

    let resp = get_req(&app, "/projects", &alice).await;
    let projects = read_json(resp).await;
    assert_eq!(
        projects.as_array().unwrap().len(),
        2,
        "alice sees only her 2 projects"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_get_enforces_ownership(pool: PgPool) {
    let app = make_app(pool, "");
    let alice = register(&app, "alice", "password123").await;
    let bob = register(&app, "bob", "password456").await;

    let id = create_project(&app, &alice).await;

    let resp = get_req(&app, &format!("/projects/{id}"), &alice).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = get_req(&app, &format!("/projects/{id}"), &bob).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_create_requires_auth(pool: PgPool) {
    let app = make_app(pool, "");
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"name": "x"})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_create_empty_name(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;

    let resp = post_json(&app, "/projects", &cookie, json!({"name": "   "})).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_create_without_game(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;

    let resp = post_json(&app, "/projects", &cookie, json!({"name": "No Game Yet"})).await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = read_json(resp).await;
    assert_eq!(body["name"], "No Game Yet");
    assert!(body["game_id"].is_null());
    assert!(body["game_name"].is_null());
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_delete(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let id = create_project(&app, &cookie).await;

    let resp = delete_req(&app, &format!("/projects/{id}"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = get_req(&app, &format!("/projects/{id}"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn projects_delete_enforces_ownership(pool: PgPool) {
    let app = make_app(pool, "");
    let alice = register(&app, "alice", "password123").await;
    let bob = register(&app, "bob", "password456").await;

    let id = create_project(&app, &alice).await;

    let resp = delete_req(&app, &format!("/projects/{id}"), &bob).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // Project still exists for alice
    let resp = get_req(&app, &format!("/projects/{id}"), &alice).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Players ───────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn players_add_and_list(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = get_req(&app, &format!("/projects/{pid}/players"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await, json!([]), "empty list initially");

    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players"),
        &cookie,
        json!({"name": "Mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let player = read_json(resp).await;
    assert_eq!(player["name"], "Mango");
    assert_eq!(player["accounts"], json!([]));
    assert_eq!(player["project_id"].as_str().unwrap(), pid);

    let resp = get_req(&app, &format!("/projects/{pid}/players"), &cookie).await;
    let players = read_json(resp).await;
    assert_eq!(players.as_array().unwrap().len(), 1);
    assert_eq!(players[0]["name"], "Mango");
}

#[sqlx::test(migrations = "../../migrations")]
async fn players_list_requires_project_ownership(pool: PgPool) {
    let app = make_app(pool, "");
    let alice = register(&app, "alice", "password123").await;
    let bob = register(&app, "bob", "password456").await;

    let pid = create_project(&app, &alice).await;

    let resp = get_req(&app, &format!("/projects/{pid}/players"), &bob).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn players_add_empty_name(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players"),
        &cookie,
        json!({"name": "  "}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn players_delete(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;
    let player_id = create_player(&app, &cookie, &pid, "Mango").await;

    let resp = delete_req(
        &app,
        &format!("/projects/{pid}/players/{player_id}"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = get_req(&app, &format!("/projects/{pid}/players"), &cookie).await;
    assert_eq!(read_json(resp).await.as_array().unwrap().len(), 0);
}

#[sqlx::test(migrations = "../../migrations")]
async fn players_delete_nonexistent(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = delete_req(
        &app,
        &format!("/projects/{pid}/players/{}", Uuid::new_v4()),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Accounts ─────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn accounts_link_and_unlink(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(startgg_user_ok(12345, "Mango")))
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;
    let player_id = create_player(&app, &cookie, &pid, "Mango").await;

    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players/{player_id}/accounts"),
        &cookie,
        json!({"slug": "user/mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let account = read_json(resp).await;
    assert_eq!(account["startgg_user_id"], 12345);
    assert_eq!(account["slug"], "user/mango");
    assert_eq!(account["display_name"], "Mango");

    let account_id = account["id"].as_str().unwrap().to_string();

    // Account appears in player list
    let resp = get_req(&app, &format!("/projects/{pid}/players"), &cookie).await;
    let players = read_json(resp).await;
    assert_eq!(players[0]["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(players[0]["accounts"][0]["slug"], "user/mango");

    // Unlink
    let resp = delete_req(
        &app,
        &format!("/projects/{pid}/players/{player_id}/accounts/{account_id}"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    let resp = get_req(&app, &format!("/projects/{pid}/players"), &cookie).await;
    assert_eq!(
        read_json(resp).await[0]["accounts"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn accounts_link_user_not_found_on_startgg(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(startgg_user_null()))
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;
    let player_id = create_player(&app, &cookie, &pid, "Ghost").await;

    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players/{player_id}/accounts"),
        &cookie,
        json!({"slug": "user/doesnotexist"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        read_json(resp).await["message"],
        "user not found on start.gg"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn accounts_link_duplicate(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    // Two calls expected: one per link attempt (the slug → user ID lookup happens before the INSERT)
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(startgg_user_ok(12345, "Mango")))
        .expect(2)
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;
    let player_id = create_player(&app, &cookie, &pid, "Mango").await;

    let first = post_json(
        &app,
        &format!("/projects/{pid}/players/{player_id}/accounts"),
        &cookie,
        json!({"slug": "user/mango"}),
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = post_json(
        &app,
        &format!("/projects/{pid}/players/{player_id}/accounts"),
        &cookie,
        json!({"slug": "user/mango"}),
    )
    .await;
    assert_eq!(second.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        read_json(second).await["message"],
        "account already linked to this player"
    );
}

// ── Games ─────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn games_search(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(startgg_games_ok()))
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/games?q=smash")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let games = read_json(resp).await;
    let games = games.as_array().unwrap();
    assert_eq!(games.len(), 2);
    assert_eq!(games[0]["id"], 1);
    assert_eq!(games[0]["name"], "Super Smash Bros. Melee");
    assert_eq!(games[0]["display_name"], "SSBM");
    assert!(games[1]["display_name"].is_null());
}

#[sqlx::test(migrations = "../../migrations")]
async fn games_empty_query(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/games?q=")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test(migrations = "../../migrations")]
async fn games_requires_auth(pool: PgPool) {
    let app = make_app(pool, "");
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/games?q=melee")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ── Import ────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn import_enqueue_returns_202(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = post_json(&app, &format!("/projects/{pid}/import"), &cookie, json!({})).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let body = read_json(resp).await;
    assert_eq!(body["status"], "pending");
    assert!(body["id"].is_string());
    assert!(body["error"].is_null());
    assert!(body["created_at"].is_string());
}

#[sqlx::test(migrations = "../../migrations")]
async fn import_status_no_job_returns_404(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = get_req(&app, &format!("/projects/{pid}/import"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn import_status_after_enqueue(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    post_json(&app, &format!("/projects/{pid}/import"), &cookie, json!({})).await;

    let resp = get_req(&app, &format!("/projects/{pid}/import"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await["status"], "pending");
}

#[sqlx::test(migrations = "../../migrations")]
async fn import_requires_auth(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/projects/{pid}/import"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/projects/{pid}/import"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn import_enforces_ownership(pool: PgPool) {
    let app = make_app(pool, "");
    let alice = register(&app, "alice", "password123").await;
    let bob = register(&app, "bob", "password456").await;
    let pid = create_project(&app, &alice).await;

    let resp = post_json(&app, &format!("/projects/{pid}/import"), &bob, json!({})).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let resp = get_req(&app, &format!("/projects/{pid}/import"), &bob).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn import_status_returns_latest_job(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    post_json(&app, &format!("/projects/{pid}/import"), &cookie, json!({})).await;
    let second =
        read_json(post_json(&app, &format!("/projects/{pid}/import"), &cookie, json!({})).await)
            .await;

    let resp = get_req(&app, &format!("/projects/{pid}/import"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    // GET returns the most recently created job
    assert_eq!(read_json(resp).await["id"], second["id"]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn import_response_includes_date_params(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = post_json(
        &app,
        &format!("/projects/{pid}/import"),
        &cookie,
        json!({ "after_date": "2026-01-15", "before_date": "2026-03-31" }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = read_json(resp).await;
    assert_eq!(body["after_date"], "2026-01-15");
    assert_eq!(body["before_date"], "2026-03-31");
}

#[sqlx::test(migrations = "../../migrations")]
async fn import_response_date_params_null_when_unset(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = post_json(&app, &format!("/projects/{pid}/import"), &cookie, json!({})).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = read_json(resp).await;
    assert!(body["after_date"].is_null());
    assert!(body["before_date"].is_null());
}

// ── Tournaments ───────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn tournaments_list_empty(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = get_req(&app, &format!("/projects/{pid}/tournaments"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await, json!([]));
}

#[sqlx::test(migrations = "../../migrations")]
async fn tournaments_list_with_data(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    seed_tournament_event(&pool, pid, 1001, 2001).await;

    let resp = get_req(&app, &format!("/projects/{pid}/tournaments"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = read_json(resp).await;
    let tournaments = body.as_array().unwrap();
    assert_eq!(tournaments.len(), 1);
    assert_eq!(tournaments[0]["name"], "Test Tournament");
    let events = tournaments[0]["events"].as_array().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["name"], "Singles");
    assert_eq!(events[0]["included"], true);
}

#[sqlx::test(migrations = "../../migrations")]
async fn tournaments_list_requires_auth(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/projects/{pid}/tournaments"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn tournaments_list_enforces_ownership(pool: PgPool) {
    let app = make_app(pool, "");
    let alice = register(&app, "alice", "password123").await;
    let bob = register(&app, "bob", "password456").await;
    let pid = create_project(&app, &alice).await;

    let resp = get_req(&app, &format!("/projects/{pid}/tournaments"), &bob).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Events (PATCH) ────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn patch_event_toggle_included(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let (_, event_id) = seed_tournament_event(&pool, pid, 1002, 2002).await;

    let resp = patch_json(
        &app,
        &format!("/projects/{pid}/events/{event_id}"),
        &cookie,
        json!({"included": false}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body["included"], false);
    assert_eq!(body["name"], "Singles");

    let resp = patch_json(
        &app,
        &format!("/projects/{pid}/events/{event_id}"),
        &cookie,
        json!({"included": true}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await["included"], true);
}

#[sqlx::test(migrations = "../../migrations")]
async fn patch_event_unknown_returns_404(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = patch_json(
        &app,
        &format!("/projects/{pid}/events/{}", Uuid::new_v4()),
        &cookie,
        json!({"included": false}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn patch_event_enforces_ownership(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let alice = register(&app, "alice", "password123").await;
    let bob = register(&app, "bob", "password456").await;
    let alice_pid_str = create_project(&app, &alice).await;
    let alice_pid = Uuid::parse_str(&alice_pid_str).unwrap();

    let (_, event_id) = seed_tournament_event(&pool, alice_pid, 1003, 2003).await;

    let resp = patch_json(
        &app,
        &format!("/projects/{alice_pid}/events/{event_id}"),
        &bob,
        json!({"included": false}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn stats_empty_project(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = get_req(&app, &format!("/projects/{pid}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await, json!([]));
}

#[sqlx::test(migrations = "../../migrations")]
async fn stats_upset_factor_computed(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();
    let bob_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Bob").await).unwrap();

    let (_, event_id) = seed_tournament_event(&pool, pid, 2001, 3001).await;

    // Alice seed 2 (round 25), Bob seed 7 (round 22); Bob beats Alice
    // Bob's UF = round(2) - round(7) = 25 - 22 = 3
    let alice_e = seed_entrant(&pool, event_id, Some(alice_id), 101, Some(2)).await;
    let bob_e = seed_entrant(&pool, event_id, Some(bob_id), 102, Some(7)).await;
    seed_set(&pool, event_id, bob_e, alice_e, 501).await;

    let resp = get_req(&app, &format!("/projects/{pid}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let stats = read_json(resp).await;
    let entries = stats.as_array().unwrap();
    assert_eq!(entries.len(), 2);

    let bob = entries.iter().find(|s| s["name"] == "Bob").unwrap();
    assert_eq!(bob["wins"].as_array().unwrap().len(), 1);
    assert_eq!(bob["wins"][0]["opponent_name"], "Alice");
    assert_eq!(bob["wins"][0]["upset_factor"], 3);
    assert_eq!(bob["losses"], json!([]));

    let alice = entries.iter().find(|s| s["name"] == "Alice").unwrap();
    assert_eq!(alice["wins"], json!([]));
    assert_eq!(alice["losses"].as_array().unwrap().len(), 1);
    assert_eq!(alice["losses"][0]["opponent_name"], "Bob");
    assert_eq!(alice["losses"][0]["upset_factor"], 3);

    // Bob ranked first (higher UF)
    assert_eq!(entries[0]["name"], "Bob");
}

#[sqlx::test(migrations = "../../migrations")]
async fn stats_excluded_events_not_counted(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();
    let bob_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Bob").await).unwrap();

    let (_, event_id) = seed_tournament_event(&pool, pid, 2002, 3002).await;
    let alice_e = seed_entrant(&pool, event_id, Some(alice_id), 103, Some(2)).await;
    let bob_e = seed_entrant(&pool, event_id, Some(bob_id), 104, Some(7)).await;
    seed_set(&pool, event_id, bob_e, alice_e, 502).await;

    // Exclude the event
    sqlx::query!(
        "UPDATE project_events SET included = false WHERE project_id = $1 AND event_id = $2",
        pid,
        event_id,
    )
    .execute(&pool)
    .await
    .unwrap();

    let resp = get_req(&app, &format!("/projects/{pid}/stats"), &cookie).await;
    let entries = read_json(resp).await;
    for entry in entries.as_array().unwrap() {
        assert_eq!(entry["wins"], json!([]));
        assert_eq!(entry["losses"], json!([]));
    }
}

#[sqlx::test(migrations = "../../migrations")]
async fn stats_requires_auth(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/projects/{pid}/stats"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn stats_enforces_ownership(pool: PgPool) {
    let app = make_app(pool, "");
    let alice = register(&app, "alice", "password123").await;
    let bob = register(&app, "bob", "password456").await;
    let pid = create_project(&app, &alice).await;

    let resp = get_req(&app, &format!("/projects/{pid}/stats"), &bob).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn stats_includes_non_project_opponent(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id =
        Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();

    let (_, event_id) = seed_tournament_event(&pool, pid, 2010, 3010).await;

    // Alice (seed 2) and a non-project entrant "Outsider" (seed 7)
    let alice_e =
        seed_entrant_named(&pool, event_id, Some(alice_id), 110, "Alice", Some(2)).await;
    let outside_e =
        seed_entrant_named(&pool, event_id, None, 111, "Outsider", Some(7)).await;

    // Outsider beats Alice
    seed_set(&pool, event_id, outside_e, alice_e, 510).await;

    let resp = get_req(&app, &format!("/projects/{pid}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let stats = read_json(resp).await;
    let entries = stats.as_array().unwrap();
    assert_eq!(entries.len(), 1, "only project players in outer list");

    let alice = entries.iter().find(|s| s["name"] == "Alice").unwrap();
    assert_eq!(alice["wins"], json!([]));
    assert_eq!(alice["losses"].as_array().unwrap().len(), 1);
    assert_eq!(alice["losses"][0]["opponent_name"], "Outsider");
    // opponent_id for non-project entrants is the entrant UUID, not a player UUID
    assert_eq!(
        alice["losses"][0]["opponent_id"],
        outside_e.to_string()
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn stats_returns_game_scores(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id =
        Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();
    let bob_id =
        Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Bob").await).unwrap();

    let (_, event_id) = seed_tournament_event(&pool, pid, 2011, 3011).await;
    let alice_e = seed_entrant(&pool, event_id, Some(alice_id), 112, Some(1)).await;
    let bob_e = seed_entrant(&pool, event_id, Some(bob_id), 113, Some(2)).await;

    // Alice beats Bob 3-1
    seed_set_with_scores(&pool, event_id, alice_e, bob_e, 511, 3, 1).await;

    let resp = get_req(&app, &format!("/projects/{pid}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let stats = read_json(resp).await;
    let alice = stats
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Alice")
        .unwrap();
    assert_eq!(alice["wins"][0]["winner_score"], 3);
    assert_eq!(alice["wins"][0]["loser_score"], 1);

    let bob = stats
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Bob")
        .unwrap();
    assert_eq!(bob["losses"][0]["winner_score"], 3);
    assert_eq!(bob["losses"][0]["loser_score"], 1);
}

// ── Head-to-head ──────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn h2h_empty(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = get_req(&app, &format!("/projects/{pid}/head-to-head"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(read_json(resp).await, json!([]));
}

#[sqlx::test(migrations = "../../migrations")]
async fn h2h_with_sets(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();
    let bob_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Bob").await).unwrap();

    let (_, event_id) = seed_tournament_event(&pool, pid, 4001, 5001).await;
    let alice_e = seed_entrant(&pool, event_id, Some(alice_id), 201, Some(1)).await;
    let bob_e = seed_entrant(&pool, event_id, Some(bob_id), 202, Some(2)).await;
    // Alice beats Bob twice
    seed_set(&pool, event_id, alice_e, bob_e, 601).await;
    seed_set(&pool, event_id, alice_e, bob_e, 602).await;

    let resp = get_req(&app, &format!("/projects/{pid}/head-to-head"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let h2h = read_json(resp).await;
    let entries = h2h.as_array().unwrap();
    assert_eq!(entries.len(), 2, "one entry per direction");

    let alice_pid = alice_id.to_string();
    let bob_pid = bob_id.to_string();

    let alice_entry = entries
        .iter()
        .find(|e| e["player_id"] == alice_pid)
        .unwrap();
    assert_eq!(alice_entry["opponent_id"], bob_pid);
    assert_eq!(alice_entry["wins"], 2);
    assert_eq!(alice_entry["losses"], 0);

    let bob_entry = entries.iter().find(|e| e["player_id"] == bob_pid).unwrap();
    assert_eq!(bob_entry["opponent_id"], alice_pid);
    assert_eq!(bob_entry["wins"], 0);
    assert_eq!(bob_entry["losses"], 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn h2h_requires_auth(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/projects/{pid}/head-to-head"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test(migrations = "../../migrations")]
async fn h2h_enforces_ownership(pool: PgPool) {
    let app = make_app(pool, "");
    let alice = register(&app, "alice", "password123").await;
    let bob = register(&app, "bob", "password456").await;
    let pid = create_project(&app, &alice).await;

    let resp = get_req(&app, &format!("/projects/{pid}/head-to-head"), &bob).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
