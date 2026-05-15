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
        "INSERT INTO tournaments (startgg_id, name, handle, online)
         VALUES ($1, 'Test Tournament', 'test-tournament', false)
         RETURNING id",
        startgg_tournament_id,
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let event_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO events (tournament_id, startgg_id, name, handle)
         VALUES ($1, $2, 'Singles', 'singles')
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
    json!({ "data": { "user": { "id": id, "player": { "gamerTag": name } } } })
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
        json!({"handle": "user/mango"}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);

    let account = read_json(resp).await;
    assert_eq!(account["startgg_user_id"], 12345);
    assert_eq!(account["handle"], "mango");
    assert_eq!(account["display_name"], "Mango");

    let account_id = account["id"].as_str().unwrap().to_string();

    // Account appears in player list
    let resp = get_req(&app, &format!("/projects/{pid}/players"), &cookie).await;
    let players = read_json(resp).await;
    assert_eq!(players[0]["accounts"].as_array().unwrap().len(), 1);
    assert_eq!(players[0]["accounts"][0]["handle"], "mango");

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
        json!({"handle": "user/doesnotexist"}),
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
        json!({"handle": "user/mango"}),
    )
    .await;
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = post_json(
        &app,
        &format!("/projects/{pid}/players/{player_id}/accounts"),
        &cookie,
        json!({"handle": "user/mango"}),
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

#[sqlx::test(migrations = "../../migrations")]
async fn import_enqueue_no_body_returns_202(pool: PgPool) {
    let app = make_app(pool, "");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&format!("/projects/{pid}/import"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let body = read_json(resp).await;
    assert_eq!(body["status"], "pending");
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

    let alice_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();

    let (_, event_id) = seed_tournament_event(&pool, pid, 2010, 3010).await;

    // Alice (seed 2) and a non-project entrant "Outsider" (seed 7)
    let alice_e = seed_entrant_named(&pool, event_id, Some(alice_id), 110, "Alice", Some(2)).await;
    let outside_e = seed_entrant_named(&pool, event_id, None, 111, "Outsider", Some(7)).await;

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
    assert_eq!(alice["losses"][0]["opponent_id"], outside_e.to_string());
}

#[sqlx::test(migrations = "../../migrations")]
async fn stats_returns_game_scores(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();
    let bob_id = Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Bob").await).unwrap();

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

// ── Tournament entrants ───────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn test_list_tournament_entrants(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;

    // Mock the tournament events query
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "tournament": {
                    "events": [{ "id": 999 }]
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Mock the entrant list query
    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": [
                            {
                                "participants": [{
                                    "gamerTag": "Mang0",
                                    "user": { "id": 1001, "slug": "user/mang0" }
                                }]
                            }
                        ]
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;

    // Create project with a game_id so tournament_entrants has a game to filter by
    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({"name": "Melee PR", "game_id": 1, "game_name": "Melee"}),
    )
    .await;
    let pid = read_json(resp).await["id"].as_str().unwrap().to_string();

    let resp = get_req(
        &app,
        &format!("/projects/{pid}/tournament-entrants?tournament=some-weekly"),
        &cookie,
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    let arr = body.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["handle"], "mang0");
    assert_eq!(arr[0]["name"], "Mang0");
    assert!(arr[0]["startgg_user_id"].is_number());
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_list_tournament_entrants_normalizes_url(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let mock = MockServer::start().await;

    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "tournament": { "events": [{ "id": 999 }] } }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    Mock::given(wiremock::matchers::method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "totalPages": 1 },
                        "nodes": []
                    }
                }
            }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;

    // Create project with a game_id
    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({"name": "Melee PR", "game_id": 1, "game_name": "Melee"}),
    )
    .await;
    let pid = read_json(resp).await["id"].as_str().unwrap().to_string();

    // Passing a full URL — should be normalized to "some-weekly"
    let resp = get_req(
        &app,
        &format!(
            "/projects/{pid}/tournament-entrants?tournament=https%3A%2F%2Fwww.start.gg%2Ftournament%2Fsome-weekly%2Fevent%2Fmelee-singles"
        ),
        &cookie,
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body.as_array().unwrap().len(), 0); // empty entrant list, but request succeeded
}

// ── Rename player ─────────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn test_rename_player(pool: PgPool) {
    let app = make_app(pool, "http://unused");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;
    let player_id = create_player(&app, &cookie, &pid, "OldName").await;

    let resp = patch_json(
        &app,
        &format!("/projects/{pid}/players/{player_id}"),
        &cookie,
        json!({"name": "NewName"}),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;
    assert_eq!(body["name"], "NewName");
    assert_eq!(body["id"], player_id);
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_rename_player_empty_name_returns_422(pool: PgPool) {
    let app = make_app(pool, "http://unused");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;
    let player_id = create_player(&app, &cookie, &pid, "OldName").await;

    let resp = patch_json(
        &app,
        &format!("/projects/{pid}/players/{player_id}"),
        &cookie,
        json!({"name": ""}),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ── Bulk add players ──────────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn test_bulk_add_players(pool: PgPool) {
    let app = make_app(pool, "http://unused");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players/bulk"),
        &cookie,
        json!({
            "players": [
                { "name": "Mang0", "startgg_user_id": 1001, "handle": "mang0" },
                { "name": "Armada", "startgg_user_id": 1002, "handle": "armada" }
            ]
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let results: Vec<serde_json::Value> = read_json(resp).await.as_array().unwrap().clone();
    assert_eq!(results.len(), 2);
    // Both should be created
    assert!(results.iter().all(|r| r["status"] == "created"));
    let handles: Vec<&str> = results.iter().map(|r| r["handle"].as_str().unwrap()).collect();
    assert!(handles.contains(&"mang0"));
    assert!(handles.contains(&"armada"));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_bulk_add_players_skips_duplicates(pool: PgPool) {
    let app = make_app(pool, "http://unused");
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    // Add first batch
    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players/bulk"),
        &cookie,
        json!({
            "players": [
                { "name": "Mang0", "startgg_user_id": 1001, "handle": "mang0" }
            ]
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Add again — same startgg_user_id should be skipped
    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players/bulk"),
        &cookie,
        json!({
            "players": [
                { "name": "Mang0", "startgg_user_id": 1001, "handle": "mang0" },
                { "name": "Armada", "startgg_user_id": 1002, "handle": "armada" }
            ]
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let results: Vec<serde_json::Value> = read_json(resp).await.as_array().unwrap().clone();
    assert_eq!(results.len(), 2);
    let mang0 = results.iter().find(|r| r["handle"] == "mang0").unwrap();
    let armada = results.iter().find(|r| r["handle"] == "armada").unwrap();
    assert_eq!(mang0["status"], "skipped");
    assert_eq!(armada["status"], "created");
}

// ── Add players by handles ────────────────────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn test_add_players_by_handles(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

    let mock = MockServer::start().await;

    // Mock: user_by_slug for "mang0" — returns user
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(startgg_user_ok(1001, "Mang0")))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    // Mock: user_by_slug for "notauser" — returns no user
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "user": null }
        })))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players/by-handles"),
        &cookie,
        json!({ "handles": ["mang0", "notauser"] }),
    )
    .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let results: Vec<serde_json::Value> = read_json(resp).await.as_array().unwrap().clone();
    assert_eq!(results.len(), 2);

    let mang0 = results.iter().find(|r| r["handle"] == "mang0").unwrap();
    let notauser = results.iter().find(|r| r["handle"] == "notauser").unwrap();
    assert_eq!(mang0["status"], "created");
    assert_eq!(mang0["name"], "Mang0");
    assert_eq!(notauser["status"], "not_found");
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_add_players_by_handles_skips_existing(pool: PgPool) {
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

    let mock = MockServer::start().await;

    // First call: used to create the player initially
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(startgg_user_ok(1001, "Mang0")))
        .mount(&mock)
        .await;

    let app = make_app(pool, &mock.uri());
    let cookie = register(&app, "alice", "password123").await;
    let pid = create_project(&app, &cookie).await;

    // First add
    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players/by-handles"),
        &cookie,
        json!({ "handles": ["mang0"] }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let r: Vec<serde_json::Value> = read_json(resp).await.as_array().unwrap().clone();
    assert_eq!(r[0]["status"], "created");

    // Second add — same handle, same startgg_user_id => skipped
    let resp = post_json(
        &app,
        &format!("/projects/{pid}/players/by-handles"),
        &cookie,
        json!({ "handles": ["mang0"] }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let r: Vec<serde_json::Value> = read_json(resp).await.as_array().unwrap().clone();
    assert_eq!(r[0]["status"], "skipped");
}

// ── Tournament event_type / bracket_types ─────────────────────────────────────

#[sqlx::test(migrations = "../../migrations")]
async fn list_tournaments_includes_event_type_and_bracket_types(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "usr1", "pass1234").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    // Insert a tournament with an event that has event_type=1 and two phases
    let t_id: uuid::Uuid = sqlx::query_scalar!(
        "INSERT INTO tournaments (startgg_id, name, handle, online)
         VALUES (9991, 'Test Cup', 'tournament/test-cup', false)
         RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let e_id: uuid::Uuid = sqlx::query_scalar!(
        "INSERT INTO events (tournament_id, startgg_id, name, event_type, handle)
         VALUES ($1, 9991, 'Melee Singles', 1, 'melee-singles')
         RETURNING id",
        t_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO phases (startgg_id, event_id, bracket_type, phase_order)
         VALUES (9991, $1, 'ROUND_ROBIN', 1), (9992, $1, 'DOUBLE_ELIMINATION', 2)",
        e_id
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO project_events (project_id, event_id, included)
         VALUES ($1, $2, true)",
        pid,
        e_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let resp = get_req(&app, &format!("/projects/{pid}/tournaments"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_json(resp).await;

    let tournaments = body.as_array().unwrap();
    assert_eq!(tournaments.len(), 1);
    let event = &tournaments[0]["events"][0];
    assert_eq!(event["event_type"], json!(1));
    assert_eq!(
        event["bracket_types"],
        json!(["ROUND_ROBIN", "DOUBLE_ELIMINATION"])
    );
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

#[sqlx::test(migrations = "../../migrations")]
async fn stats_returns_enriched_set_fields(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id =
        Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();
    let bob_id =
        Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Bob").await).unwrap();

    // Tournament with location
    let t_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO tournaments (startgg_id, name, handle, online, city, addr_state)
         VALUES (9001, 'LACS', 'tournament/lacs', false, 'Los Angeles', 'CA')
         RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Event with num_entrants
    let e_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO events (tournament_id, startgg_id, name, num_entrants, handle)
         VALUES ($1, 8001, 'Melee Singles', 128, 'melee-singles')
         RETURNING id",
        t_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO project_events (project_id, event_id, included) VALUES ($1, $2, true)",
        pid,
        e_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Phase and phase_group
    let phase_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO phases (startgg_id, event_id, name, phase_order)
         VALUES (7001, $1, 'Top 8', 2)
         RETURNING id",
        e_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let pg_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO phase_groups (startgg_id, phase_id, display_identifier)
         VALUES (6001, $1, 'Pool A')
         RETURNING id",
        phase_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Entrants with final_placement: Bob placed 1st, Alice placed 2nd
    let bob_e: Uuid = sqlx::query_scalar!(
        "INSERT INTO entrants (event_id, player_id, startgg_entrant_id, display_name, seed, final_placement)
         VALUES ($1, $2, 501, 'Bob', 1, 1)
         RETURNING id",
        e_id,
        bob_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let alice_e: Uuid = sqlx::query_scalar!(
        "INSERT INTO entrants (event_id, player_id, startgg_entrant_id, display_name, seed, final_placement)
         VALUES ($1, $2, 502, 'Alice', 2, 2)
         RETURNING id",
        e_id,
        alice_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Set: Bob beats Alice, in the phase_group
    sqlx::query!(
        "INSERT INTO sets (event_id, phase_group_id, startgg_set_id, winner_entrant_id, loser_entrant_id, is_dq)
         VALUES ($1, $2, 901, $3, $4, false)",
        e_id,
        pg_id,
        bob_e,
        alice_e
    )
    .execute(&pool)
    .await
    .unwrap();

    let resp = get_req(&app, &format!("/projects/{pid}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let stats = read_json(resp).await;
    let bob = stats
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "Bob")
        .unwrap();
    let win = &bob["wins"][0];

    assert_eq!(win["location"], "Los Angeles, CA");
    assert_eq!(win["num_entrants"], 128);
    assert_eq!(win["phase_name"], "Top 8");
    assert_eq!(win["pool_identifier"], "Pool A");
    assert_eq!(win["winner_placement"], 1);
    assert_eq!(win["loser_placement"], 2);
}

#[sqlx::test(migrations = "../../migrations")]
async fn h2h_sets_returns_enriched_fields(pool: PgPool) {
    let app = make_app(pool.clone(), "");
    let cookie = register(&app, "alice", "password123").await;
    let pid_str = create_project(&app, &cookie).await;
    let pid = Uuid::parse_str(&pid_str).unwrap();

    let alice_id =
        Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Alice").await).unwrap();
    let bob_id =
        Uuid::parse_str(&create_player(&app, &cookie, &pid_str, "Bob").await).unwrap();

    // Online tournament (location should be "Online")
    let t_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO tournaments (startgg_id, name, handle, online, city, addr_state)
         VALUES (9002, 'Online Major', 'tournament/online-major', true, 'Austin', 'TX')
         RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let e_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO events (tournament_id, startgg_id, name, num_entrants, handle)
         VALUES ($1, 8002, 'Melee Singles', 64, 'melee-singles')
         RETURNING id",
        t_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO project_events (project_id, event_id, included) VALUES ($1, $2, true)",
        pid,
        e_id
    )
    .execute(&pool)
    .await
    .unwrap();

    let phase_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO phases (startgg_id, event_id, name, phase_order)
         VALUES (7002, $1, 'Bracket', 1)
         RETURNING id",
        e_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let pg_id: Uuid = sqlx::query_scalar!(
        "INSERT INTO phase_groups (startgg_id, phase_id, display_identifier)
         VALUES (6002, $1, NULL)
         RETURNING id",
        phase_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let alice_e: Uuid = sqlx::query_scalar!(
        "INSERT INTO entrants (event_id, player_id, startgg_entrant_id, display_name, seed, final_placement)
         VALUES ($1, $2, 503, 'Alice', 1, 1)
         RETURNING id",
        e_id,
        alice_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let bob_e: Uuid = sqlx::query_scalar!(
        "INSERT INTO entrants (event_id, player_id, startgg_entrant_id, display_name, seed, final_placement)
         VALUES ($1, $2, 504, 'Bob', 2, 2)
         RETURNING id",
        e_id,
        bob_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO sets (event_id, phase_group_id, startgg_set_id, winner_entrant_id, loser_entrant_id, is_dq)
         VALUES ($1, $2, 902, $3, $4, false)",
        e_id,
        pg_id,
        alice_e,
        bob_e
    )
    .execute(&pool)
    .await
    .unwrap();

    let resp = get_req(
        &app,
        &format!("/projects/{pid}/head-to-head/{alice_id}/{bob_id}/sets"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let sets = read_json(resp).await;
    let set = &sets[0];

    // online=true overrides city/state
    assert_eq!(set["location"], "Online");
    assert_eq!(set["num_entrants"], 64);
    assert_eq!(set["phase_name"], "Bracket");
    assert_eq!(set["pool_identifier"], json!(null));
    assert_eq!(set["winner_placement"], 1);
    assert_eq!(set["loser_placement"], 2);
}
