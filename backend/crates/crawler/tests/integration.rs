#![recursion_limit = "256"]

use serde_json::json;
use std::sync::atomic::AtomicBool;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[sqlx::test(migrations = "../../migrations")]
async fn test_crawl_single_tournament(pool: sqlx::PgPool) {
    let mock_server = MockServer::start().await;

    // Stub 1: tournaments page 1
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query Tournaments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "tournaments": {
                    "pageInfo": { "total": 1, "totalPages": 1 },
                    "nodes": [{
                        "id": "1001",
                        "name": "Test Major",
                        "slug": "tournament/test-major",
                        "startAt": 1700000000_i64,
                        "endAt": 1700086400_i64,
                        "countryCode": "US",
                        "city": "Seattle",
                        "addrState": "WA",
                        "numAttendees": 128,
                        "isOnline": false,
                        "lat": 47.6062,
                        "lng": -122.3321,
                        "timezone": "America/Los_Angeles",
                        "events": [{
                            "id": "2001",
                            "name": "Singles",
                            "slug": "tournament/test-major/event/singles",
                            "startAt": 1700000000_i64,
                            "state": 3,
                            "isOnline": false,
                            "numEntrants": 2,
                            "type": 1,
                            "competitionTier": null,
                            "videogame": { "id": "1", "name": "Super Smash Bros. Ultimate" }
                        }]
                    }]
                }
            }
        })))
        .mount(&mock_server)
        .await;

    // Stub 2: phase groups for event 2001
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query EventPhaseGroups"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "phases": [{
                        "id": "3001",
                        "phaseGroups": {
                            "pageInfo": { "totalPages": 1 },
                            "nodes": [{ "id": "4001" }]
                        }
                    }]
                }
            }
        })))
        .mount(&mock_server)
        .await;

    // Stub 3: sets for phase group 4001 (full query)
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query PhaseGroupSets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "phaseGroup": {
                    "id": "4001",
                    "sets": {
                        "pageInfo": { "total": 1, "totalPages": 1 },
                        "nodes": [{
                            "id": "5001",
                            "state": 3,
                            "winnerId": "6001",
                            "vodUrl": null,
                            "completedAt": 1700003600_i64,
                            "fullRoundText": "Winners Final",
                            "round": 1,
                            "lPlacement": null,
                            "wPlacement": null,
                            "displayScore": "3 - 1",
                            "phaseGroup": {
                                "id": "4001",
                                "displayIdentifier": "A",
                                "bracketType": "DOUBLE_ELIMINATION",
                                "phase": {
                                    "id": "3001",
                                    "name": "Bracket",
                                    "bracketType": "DOUBLE_ELIMINATION",
                                    "phaseOrder": 1,
                                    "isExhibition": false
                                }
                            },
                            "slots": [
                                {
                                    "slotIndex": 0,
                                    "standing": { "stats": { "score": { "value": 3.0 } } },
                                    "entrant": {
                                        "id": "6001",
                                        "initialSeedNum": 1,
                                        "isDisqualified": false,
                                        "participants": [{
                                            "player": { "id": "7001", "gamerTag": "PlayerA", "prefix": null },
                                            "user": {
                                                "id": "8001",
                                                "slug": "user/playera",
                                                "name": "Alice",
                                                "bio": null,
                                                "genderPronoun": null,
                                                "location": { "city": "Seattle", "state": "WA", "country": "US" },
                                                "images": [{ "url": "https://cdn.start.gg/img/a.jpg", "type": "profile" }]
                                            }
                                        }]
                                    }
                                },
                                {
                                    "slotIndex": 1,
                                    "standing": { "stats": { "score": { "value": 1.0 } } },
                                    "entrant": {
                                        "id": "6002",
                                        "initialSeedNum": 2,
                                        "isDisqualified": false,
                                        "participants": [{
                                            "player": { "id": "7002", "gamerTag": "PlayerB", "prefix": null },
                                            "user": {
                                                "id": "8002",
                                                "slug": "user/playerb",
                                                "name": "Bob",
                                                "bio": null,
                                                "genderPronoun": null,
                                                "location": null,
                                                "images": []
                                            }
                                        }]
                                    }
                                }
                            ],
                            "games": [{
                                "orderNum": 1,
                                "winnerId": "6001",
                                "stage": { "id": "101", "name": "Battlefield" },
                                "selections": [
                                    {
                                        "selectionType": "CHARACTER",
                                        "entrant": { "id": "6001" },
                                        "character": { "id": "1", "name": "Fox" }
                                    },
                                    {
                                        "selectionType": "CHARACTER",
                                        "entrant": { "id": "6002" },
                                        "character": { "id": "2", "name": "Marth" }
                                    }
                                ]
                            }]
                        }]
                    }
                }
            }
        })))
        .mount(&mock_server)
        .await;

    // Stub 4: standings for event 2001
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query EventStandings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "standings": {
                        "pageInfo": { "total": 2, "totalPages": 1 },
                        "nodes": [
                            { "id": "9001", "placement": 1, "isFinal": true, "entrant": { "id": "6001" } },
                            { "id": "9002", "placement": 2, "isFinal": true, "entrant": { "id": "6002" } }
                        ]
                    }
                }
            }
        })))
        .mount(&mock_server)
        .await;

    let config = crawler::cli::Config {
        database_url: "unused".into(),
        startgg_api_key: "test-key".into(),
        from_date: chrono::NaiveDate::from_ymd_opt(2023, 11, 14).unwrap(),
        to_date: chrono::NaiveDate::from_ymd_opt(2023, 11, 15).unwrap(),
        window_days: 1,
        delay_ms: 0,
        sets_per_page: 20,
        game_id: None,
        rust_log: "off".into(),
        startgg_base_url: Some(mock_server.uri()),
    };

    let shutdown = AtomicBool::new(false);
    crawler::scraper::run(&config, &pool, &shutdown)
        .await
        .unwrap();

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_tournaments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(1));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_players")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(2));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_sets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(1));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_set_games")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(1));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_game_selections")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(2));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_event_entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(2));
}

#[sqlx::test(migrations = "../../migrations")]
async fn test_crawl_two_pass_fallback(pool: sqlx::PgPool) {
    let mock_server = MockServer::start().await;

    // Stub 1: tournaments page 1 — same fixture as single-tournament test
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query Tournaments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "tournaments": {
                    "pageInfo": { "total": 1, "totalPages": 1 },
                    "nodes": [{
                        "id": "1001",
                        "name": "Test Major",
                        "slug": "tournament/test-major",
                        "startAt": 1700000000_i64,
                        "endAt": 1700086400_i64,
                        "countryCode": "US",
                        "city": "Seattle",
                        "addrState": "WA",
                        "numAttendees": 128,
                        "isOnline": false,
                        "lat": 47.6062,
                        "lng": -122.3321,
                        "timezone": "America/Los_Angeles",
                        "events": [{
                            "id": "2001",
                            "name": "Singles",
                            "slug": "tournament/test-major/event/singles",
                            "startAt": 1700000000_i64,
                            "state": 3,
                            "isOnline": false,
                            "numEntrants": 2,
                            "type": 1,
                            "competitionTier": null,
                            "videogame": { "id": "1", "name": "Super Smash Bros. Ultimate" }
                        }]
                    }]
                }
            }
        })))
        .mount(&mock_server)
        .await;

    // Stub 2: phase groups for event 2001
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query EventPhaseGroups"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "phases": [{
                        "id": "3001",
                        "phaseGroups": {
                            "pageInfo": { "totalPages": 1 },
                            "nodes": [{ "id": "4001" }]
                        }
                    }]
                }
            }
        })))
        .mount(&mock_server)
        .await;

    // Stub 3: full sets query returns ComplexityError (registered first = highest priority in FIFO).
    // Matches the full PhaseGroupSets query specifically because that query contains "user {";
    // the slim query does not contain "user {" so this stub won't match the slim pass.
    // No up_to_n_times limit: with_complexity_retry halves perPage (20→10→5→2→1), making
    // multiple calls, all of which must return a complexity error to trigger the two-pass fallback.
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("user {"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "errors": [{ "message": "query complexity is too high (actual: 1234)" }],
            "data": null
        })))
        .mount(&mock_server)
        .await;

    // Stub 4: slim sets for phase group 4001 (registered after complexity stub = lower priority).
    // Matches any PhaseGroupSets call; fires for the slim pass after the full query is rejected.
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query PhaseGroupSets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "phaseGroup": {
                    "id": "4001",
                    "sets": {
                        "pageInfo": { "total": 1, "totalPages": 1 },
                        "nodes": [{
                            "id": "5001",
                            "state": 3,
                            "winnerId": "6001",
                            "vodUrl": null,
                            "completedAt": 1700003600_i64,
                            "fullRoundText": "Winners Final",
                            "round": 1,
                            "lPlacement": null,
                            "wPlacement": null,
                            "displayScore": "3 - 1",
                            "phaseGroup": {
                                "id": "4001",
                                "displayIdentifier": "A",
                                "bracketType": "DOUBLE_ELIMINATION",
                                "phase": {
                                    "id": "3001",
                                    "name": "Bracket",
                                    "bracketType": "DOUBLE_ELIMINATION",
                                    "phaseOrder": 1,
                                    "isExhibition": false
                                }
                            },
                            "slots": [
                                {
                                    "slotIndex": 0,
                                    "standing": null,
                                    "entrant": {
                                        "id": "6001",
                                        "initialSeedNum": 1,
                                        "isDisqualified": false,
                                        "participants": [{
                                            "player": { "id": "7001", "gamerTag": "PlayerA", "prefix": null }
                                        }]
                                    }
                                },
                                {
                                    "slotIndex": 1,
                                    "standing": null,
                                    "entrant": {
                                        "id": "6002",
                                        "initialSeedNum": 2,
                                        "isDisqualified": false,
                                        "participants": [{
                                            "player": { "id": "7002", "gamerTag": "PlayerB", "prefix": null }
                                        }]
                                    }
                                }
                            ]
                        }]
                    }
                }
            }
        })))
        .mount(&mock_server)
        .await;

    // Stub 5: games pass for phase group 4001
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query PhaseGroupGames"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "phaseGroup": {
                    "id": "4001",
                    "sets": {
                        "pageInfo": { "total": 1, "totalPages": 1 },
                        "nodes": [{
                            "id": "5001",
                            "games": [{
                                "orderNum": 1,
                                "winnerId": "6001",
                                "stage": { "id": "101", "name": "Battlefield" },
                                "selections": [
                                    {
                                        "selectionType": "CHARACTER",
                                        "entrant": { "id": "6001" },
                                        "character": { "id": "1", "name": "Fox" }
                                    },
                                    {
                                        "selectionType": "CHARACTER",
                                        "entrant": { "id": "6002" },
                                        "character": { "id": "2", "name": "Marth" }
                                    }
                                ]
                            }]
                        }]
                    }
                }
            }
        })))
        .mount(&mock_server)
        .await;

    // Stub 6: standings for event 2001
    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query EventStandings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "event": {
                    "standings": {
                        "pageInfo": { "total": 2, "totalPages": 1 },
                        "nodes": [
                            { "id": "9001", "placement": 1, "isFinal": true, "entrant": { "id": "6001" } },
                            { "id": "9002", "placement": 2, "isFinal": true, "entrant": { "id": "6002" } }
                        ]
                    }
                }
            }
        })))
        .mount(&mock_server)
        .await;

    let config = crawler::cli::Config {
        database_url: "unused".into(),
        startgg_api_key: "test-key".into(),
        from_date: chrono::NaiveDate::from_ymd_opt(2023, 11, 14).unwrap(),
        to_date: chrono::NaiveDate::from_ymd_opt(2023, 11, 15).unwrap(),
        window_days: 1,
        delay_ms: 0,
        sets_per_page: 20,
        game_id: None,
        rust_log: "off".into(),
        startgg_base_url: Some(mock_server.uri()),
    };

    let shutdown = AtomicBool::new(false);
    crawler::scraper::run(&config, &pool, &shutdown)
        .await
        .unwrap();

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_tournaments")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(1));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_players")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(2));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_sets")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(1));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_set_games")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(1));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_game_selections")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(2));

    let count = sqlx::query_scalar!("SELECT COUNT(*) FROM global_event_entries")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, Some(2));
}
