// Integration tests for the RankingForge API.
// Requires a running PostgreSQL server:
//   DATABASE_URL=postgres://rankingforge:rankingforge@localhost:5432/rankingforge cargo test

use api::{routes, state::AppState, StartggClient};
use axum::{body::Body, http::Request, http::StatusCode, Router};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_app(pool: PgPool, startgg_base_url: &str) -> Router {
    let startgg = if startgg_base_url.is_empty() {
        StartggClient::new("test-key")
    } else {
        StartggClient::new_with_base_url("test-key", startgg_base_url)
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
    assert_eq!(resp.status(), StatusCode::CREATED, "register should return 201");

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
        .oneshot(Request::builder().uri("/auth/me").body(Body::empty()).unwrap())
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
    assert!(resp.headers().contains_key("set-cookie"), "login must set session cookie");

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
            serde_json::to_vec(&json!({"username": "alice", "password": "wrongpassword"}))
                .unwrap(),
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
    assert_eq!(projects.as_array().unwrap().len(), 2, "alice sees only her 2 projects");
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
                .body(Body::from(serde_json::to_vec(&json!({"name": "x"})).unwrap()))
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

    let resp = delete_req(&app, &format!("/projects/{pid}/players/{player_id}"), &cookie).await;
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
    assert_eq!(read_json(resp).await[0]["accounts"].as_array().unwrap().len(), 0);
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
    assert_eq!(read_json(resp).await["message"], "user not found on start.gg");
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
