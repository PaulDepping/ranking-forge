// End-to-end regression test: user registration → project setup → import → stats/H2H.
// Calls the real Axum router and the real import pipeline against a wiremock start.gg.
// Run with: DATABASE_URL=postgres://... cargo test -p e2e

use api::{StartggClient, routes, state::AppState};
use axum::{Router, body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;
use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_app(pool: PgPool, startgg_base_url: &str) -> Router {
    let startgg = StartggClient::new_with_base_url("test-key".into(), startgg_base_url.into());
    let state = AppState {
        db: pool,
        startgg,
        cors_origin: "http://localhost".to_string(),
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
            serde_json::to_vec(&json!({"email": format!("{username}@test.com"), "display_name": username, "password": password})).unwrap(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
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

fn mount_import_mocks(mock: &wiremock::MockServer) -> impl std::future::Future<Output = ()> + '_ {
    async move {
        Mock::given(method("POST"))
            .and(body_string_contains("\"mango\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "user": { "id": 12345_i64, "player": { "gamerTag": "Mango" } } }
            })))
            .mount(mock)
            .await;

        Mock::given(method("POST"))
            .and(body_string_contains("\"armada\""))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "user": { "id": 67890_i64, "player": { "gamerTag": "Armada" } } }
            })))
            .mount(mock)
            .await;

        Mock::given(method("POST"))
            .and(body_string_contains("userId"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "user": {
                        "tournaments": {
                            "pageInfo": { "total": 1, "totalPages": 1 },
                            "nodes": [{
                                "id": 1001_i64,
                                "name": "Test Tournament",
                                "slug": "tournament/test-2024",
                                "city": "San Jose",
                                "addrState": "CA",
                                "countryCode": "US",
                                "venueName": null,
                                "venueAddress": null,
                                "timezone": "America/Los_Angeles",
                                "isOnline": false,
                                "numAttendees": 8,
                                "startAt": 1700000000_i64,
                                "endAt":   1700086400_i64,
                                "events": [{
                                    "id": 2001_i64,
                                    "name": "Melee Singles",
                                    "numEntrants": 2,
                                    "startAt": 1700040000_i64
                                }]
                            }]
                        }
                    }
                }
            })))
            .mount(mock)
            .await;

        Mock::given(method("POST"))
            .and(body_string_contains("phaseGroups(query:"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "event": {
                        "phases": [{
                            "id": 5001_i64,
                            "name": "Bracket",
                            "bracketType": "DOUBLE_ELIMINATION",
                            "phaseOrder": 1,
                            "numSeeds": 2,
                            "groupCount": 1,
                            "state": "COMPLETED",
                            "isExhibition": false,
                            "phaseGroups": {
                                "pageInfo": { "total": 1, "totalPages": 1 },
                                "nodes": [{
                                    "id": 6001_i64,
                                    "displayIdentifier": "1",
                                    "bracketType": "DOUBLE_ELIMINATION",
                                    "bracketUrl": null,
                                    "numRounds": null,
                                    "startAt": null,
                                    "firstRoundTime": null,
                                    "state": 3
                                }]
                            }
                        }]
                    }
                }
            })))
            .mount(mock)
            .await;

        Mock::given(method("POST"))
            .and(body_string_contains("entrants(query:"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "event": {
                        "entrants": {
                            "pageInfo": { "total": 2, "totalPages": 1 },
                            "nodes": [
                                {
                                    "id": 3001_i64,
                                    "initialSeedNum": 2,
                                    "isDisqualified": false,
                                    "standing": { "placement": 2 },
                                    "participants": [{ "gamerTag": "Mango", "user": { "id": 12345_i64 } }]
                                },
                                {
                                    "id": 3002_i64,
                                    "initialSeedNum": 7,
                                    "isDisqualified": false,
                                    "standing": { "placement": 1 },
                                    "participants": [{ "gamerTag": "Armada", "user": { "id": 67890_i64 } }]
                                }
                            ]
                        }
                    }
                }
            })))
            .mount(mock)
            .await;

        Mock::given(method("POST"))
            .and(body_string_contains("sets(page:"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "event": {
                        "sets": {
                            "pageInfo": { "total": 1, "totalPages": 1 },
                            "nodes": [{
                                "id": 4001_i64,
                                "winnerId": 3002_i64,
                                "round": 1,
                                "fullRoundText": "Round 1",
                                "totalGames": 5,
                                "completedAt": 1700050000_i64,
                                "vodUrl": null,
                                "slots": [
                                    {
                                        "entrant": { "id": 3002_i64 },
                                        "standing": { "stats": { "score": { "value": 3.0 } } }
                                    },
                                    {
                                        "entrant": { "id": 3001_i64 },
                                        "standing": { "stats": { "score": { "value": 1.0 } } }
                                    }
                                ]
                            }]
                        }
                    }
                }
            })))
            .mount(mock)
            .await;
    }
}

// ── Test ──────────────────────────────────────────────────────────────────────

/// Full happy-path regression test:
///
/// register → create project → add Mango + Armada → link start.gg accounts →
/// run import worker → assert tournaments/stats/H2H → toggle event exclusion →
/// assert stats change, then restore.
#[sqlx::test(migrations = "../../migrations")]
async fn full_import_flow(pool: PgPool) {
    let mock = MockServer::start().await;

    // user_by_slug("mango") — handle is normalized before the start.gg call
    Mock::given(method("POST"))
        .and(body_string_contains("\"mango\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "user": { "id": 12345_i64, "player": { "gamerTag": "Mango" } } }
        })))
        .mount(&mock)
        .await;

    // user_by_slug("armada") — handle is normalized before the start.gg call
    Mock::given(method("POST"))
        .and(body_string_contains("\"armada\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "user": { "id": 67890_i64, "player": { "gamerTag": "Armada" } } }
        })))
        .mount(&mock)
        .await;

    // tournaments_by_user — called once per linked player; both share the same tournament
    Mock::given(method("POST"))
        .and(body_string_contains("userId"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "user": {
                    "tournaments": {
                        "pageInfo": { "total": 1, "totalPages": 1 },
                        "nodes": [{
                            "id": 1001_i64,
                            "name": "Test Tournament",
                            "slug": "tournament/test-2024",
                            "city": "San Jose",
                            "addrState": "CA",
                            "countryCode": "US",
                            "venueName": null,
                            "venueAddress": null,
                            "timezone": "America/Los_Angeles",
                            "isOnline": false,
                            "numAttendees": 8,
                            "startAt": 1700000000_i64,
                            "endAt":   1700086400_i64,
                            "events": [{
                                "id": 2001_i64,
                                "name": "Melee Singles",
                                "numEntrants": 2,
                                "startAt": 1700040000_i64
                            }]
                        }]
                    }
                }
            }
        })))
        .expect(2)
        .mount(&mock)
        .await;

    // event_phases: one phase with one phase group
    Mock::given(method("POST"))
        .and(body_string_contains("phaseGroups(query:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "phases": [{
                        "id": 5001_i64,
                        "name": "Bracket",
                        "bracketType": "DOUBLE_ELIMINATION",
                        "phaseOrder": 1,
                        "numSeeds": 2,
                        "groupCount": 1,
                        "state": "COMPLETED",
                        "isExhibition": false,
                        "phaseGroups": {
                            "pageInfo": { "total": 1, "totalPages": 1 },
                            "nodes": [{
                                "id": 6001_i64,
                                "displayIdentifier": "1",
                                "bracketType": "DOUBLE_ELIMINATION",
                                "bracketUrl": null,
                                "numRounds": null,
                                "startAt": null,
                                "firstRoundTime": null,
                                "state": 3
                            }]
                        }
                    }]
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    // event_entrants: Mango seeded 2nd, Armada seeded 7th
    Mock::given(method("POST"))
        .and(body_string_contains("entrants(query:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "entrants": {
                        "pageInfo": { "total": 2, "totalPages": 1 },
                        "nodes": [
                            {
                                "id": 3001_i64,
                                "initialSeedNum": 2,
                                "isDisqualified": false,
                                "standing": { "placement": 2 },
                                "participants": [{ "gamerTag": "Mango", "user": { "id": 12345_i64 } }]
                            },
                            {
                                "id": 3002_i64,
                                "initialSeedNum": 7,
                                "isDisqualified": false,
                                "standing": { "placement": 1 },
                                "participants": [{ "gamerTag": "Armada", "user": { "id": 67890_i64 } }]
                            }
                        ]
                    }
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    // event_sets: Armada (entrant 3002, seed 7) beats Mango (entrant 3001, seed 2)
    // upset_factor = seed_to_projected_round(2) - seed_to_projected_round(7) = 25 - 22 = 3
    Mock::given(method("POST"))
        .and(body_string_contains("sets(page:"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "sets": {
                        "pageInfo": { "total": 1, "totalPages": 1 },
                        "nodes": [{
                            "id": 4001_i64,
                            "winnerId": 3002_i64,
                            "round": 1,
                            "fullRoundText": "Round 1",
                            "totalGames": 5,
                            "completedAt": 1700050000_i64,
                            "vodUrl": null,
                            "slots": [
                                {
                                    "entrant": { "id": 3002_i64 },
                                    "standing": { "stats": { "score": { "value": 3.0 } } }
                                },
                                {
                                    "entrant": { "id": 3001_i64 },
                                    "standing": { "stats": { "score": { "value": 1.0 } } }
                                }
                            ]
                        }]
                    }
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let base_url = mock.uri();
    let app = make_app(pool.clone(), &base_url);

    // ── Setup ─────────────────────────────────────────────────────────────────

    let cookie = register(&app, "testuser", "pass1234").await;

    let resp = post_json(
        &app,
        "/projects",
        &cookie,
        json!({
            "name": "Melee Power Rankings",
            "game_id": 1,
            "game_name": "Super Smash Bros. Melee"
        }),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let project = read_json(resp).await;
    let project_id = project["id"].as_str().unwrap().to_string();

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

    // ── Import ────────────────────────────────────────────────────────────────

    let project_uuid = Uuid::parse_str(&project_id).unwrap();
    let startgg_worker = StartggClient::new_with_base_url("test-key".into(), base_url.into());
    worker::import::run(
        &pool,
        &startgg_worker,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    // ── Tournaments ───────────────────────────────────────────────────────────

    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/tournaments"),
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

    // ── Stats ─────────────────────────────────────────────────────────────────

    let resp = get_req(&app, &format!("/projects/{project_id}/stats"), &cookie).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let stats = read_json(resp).await;
    let stats_arr = stats.as_array().unwrap();

    let armada_stats = stats_arr
        .iter()
        .find(|s| s["player_id"] == armada_id)
        .unwrap();
    assert_eq!(armada_stats["wins"].as_array().unwrap().len(), 1);
    assert_eq!(armada_stats["wins"][0]["opponent_name"], "Mango");
    assert_eq!(armada_stats["wins"][0]["upset_factor"], json!(3));
    assert_eq!(armada_stats["losses"], json!([]));

    // Enriched fields
    assert_eq!(
        armada_stats["wins"][0]["tournament_name"],
        json!("Test Tournament")
    );
    assert_eq!(
        armada_stats["wins"][0]["tournament_handle"],
        json!("test-2024")
    );
    assert_eq!(
        armada_stats["wins"][0]["event_name"],
        json!("Melee Singles")
    );
    assert_eq!(armada_stats["wins"][0]["round_name"], json!("Round 1"));
    assert_eq!(armada_stats["wins"][0]["winner_seed"], json!(7));
    assert_eq!(armada_stats["wins"][0]["loser_seed"], json!(2));
    assert_eq!(armada_stats["wins"][0]["is_dq"], json!(false));
    assert_eq!(armada_stats["wins"][0]["startgg_set_id"], json!(4001_i64));

    let mango_stats = stats_arr
        .iter()
        .find(|s| s["player_id"] == mango_id)
        .unwrap();
    assert_eq!(mango_stats["wins"], json!([]));
    assert_eq!(mango_stats["losses"].as_array().unwrap().len(), 1);
    assert_eq!(mango_stats["losses"][0]["opponent_name"], "Armada");
    assert_eq!(mango_stats["losses"][0]["upset_factor"], json!(3));

    // Loser-side construction path covered
    assert_eq!(
        mango_stats["losses"][0]["tournament_name"],
        json!("Test Tournament")
    );

    // ── Head-to-head ──────────────────────────────────────────────────────────

    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/head-to-head"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let h2h = read_json(resp).await;
    let h2h_arr = h2h.as_array().unwrap();

    let armada_vs_mango = h2h_arr
        .iter()
        .find(|e| e["player_id"] == armada_id && e["opponent_id"] == mango_id)
        .unwrap();
    assert_eq!(armada_vs_mango["wins"], json!(1));
    assert_eq!(armada_vs_mango["losses"], json!(0));

    let mango_vs_armada = h2h_arr
        .iter()
        .find(|e| e["player_id"] == mango_id && e["opponent_id"] == armada_id)
        .unwrap();
    assert_eq!(mango_vs_armada["wins"], json!(0));
    assert_eq!(mango_vs_armada["losses"], json!(1));

    // ── H2H sets endpoint ─────────────────────────────────────────────────────
    let resp = get_req(
        &app,
        &format!("/projects/{project_id}/head-to-head/{mango_id}/{armada_id}/sets"),
        &cookie,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let sets_body = read_json(resp).await;
    let sets_arr = sets_body.as_array().unwrap();
    assert_eq!(sets_arr.len(), 1);
    // pid_a = mango_id, mango lost → is_win = false
    assert_eq!(sets_arr[0]["is_win"], json!(false));
    assert_eq!(sets_arr[0]["tournament_name"], json!("Test Tournament"));
    assert_eq!(sets_arr[0]["event_name"], json!("Melee Singles"));
    assert_eq!(sets_arr[0]["round_name"], json!("Round 1"));
    assert_eq!(sets_arr[0]["opponent_name"], json!("Armada"));

    // ── Event exclusion ───────────────────────────────────────────────────────

    let resp = patch_json(
        &app,
        &format!("/projects/{project_id}/events/{event_id}"),
        &cookie,
        json!({"included": false}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = get_req(&app, &format!("/projects/{project_id}/stats"), &cookie).await;
    let stats = read_json(resp).await;
    let armada_stats = stats
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["player_id"] == armada_id)
        .unwrap();
    assert_eq!(armada_stats["wins"], json!([]));
    assert_eq!(armada_stats["losses"], json!([]));

    // ── Re-include ────────────────────────────────────────────────────────────

    let resp = patch_json(
        &app,
        &format!("/projects/{project_id}/events/{event_id}"),
        &cookie,
        json!({"included": true}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = get_req(&app, &format!("/projects/{project_id}/stats"), &cookie).await;
    let stats = read_json(resp).await;
    let armada_stats = stats
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["player_id"] == armada_id)
        .unwrap();
    assert_eq!(armada_stats["wins"].as_array().unwrap().len(), 1);
    assert_eq!(armada_stats["wins"][0]["upset_factor"], json!(3));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_rename_project(pool: PgPool) {
    let app = make_app(pool, "http://unused");
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
    let mock = MockServer::start().await;
    mount_import_mocks(&mock).await;

    let base_url = mock.uri();
    let app = make_app(pool.clone(), &base_url);
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

    // Import: Armada wins 1-0, Mango loses 0-1
    let startgg = StartggClient::new_with_base_url("test-key".into(), base_url.into());
    worker::import::run(
        &pool,
        &startgg,
        Uuid::parse_str(&project_id).unwrap(),
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    // After import, GET /players should return Armada first (higher winrate)
    let resp = get_req(&app, &format!("/projects/{project_id}/players"), &cookie).await;
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
    let mock = MockServer::start().await;
    mount_import_mocks(&mock).await;

    let base_url = mock.uri();
    let app = make_app(pool.clone(), &base_url);
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

    let project_uuid = Uuid::parse_str(&project_id).unwrap();
    let startgg = StartggClient::new_with_base_url("test-key".into(), base_url.clone().into());

    // First import: automatic sort gives Armada rank 1, Mango rank 2
    worker::import::run(
        &pool,
        &startgg,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    // User manually reorders: Mango first, Armada second
    let resp = put_json(
        &app,
        &format!("/projects/{project_id}/ranking"),
        &cookie,
        json!({"player_ids": [mango_id, armada_id]}),
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Second import: must not overwrite the manual ranking
    let startgg2 = StartggClient::new_with_base_url("test-key".into(), base_url.into());
    worker::import::run(
        &pool,
        &startgg2,
        project_uuid,
        Uuid::nil(),
        common::jobs::ImportParams::default(),
    )
    .await
    .unwrap();

    let resp = get_req(&app, &format!("/projects/{project_id}/players"), &cookie).await;
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
