#![cfg(feature = "topology-tests")]

use reqwest::Client;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::sleep;

// ── Golden dataset ─────────────────────────────────────────────────────────────
// These are completed past Smash Hannover Weeklies — data is immutable.

const PLAYER1_SLUG: &str = "user/06b4042d";
const PLAYER2_SLUG: &str = "user/54b7bbf3";
const WEEKLY_100_NAME: &str = "Smash Hannover Weekly #100";
const WEEKLY_88_NAME: &str = "Smash Hannover Weekly #88";

// ── Helpers ───────────────────────────────────────────────────────────────────

fn api_url() -> String {
    std::env::var("API_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}

fn db_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set to run topology tests")
}

async fn wait_for_api(client: &Client) {
    let url = format!("{}/health", api_url());
    for _ in 0..60 {
        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return;
            }
        }
        sleep(Duration::from_millis(500)).await;
    }
    panic!("API at {} did not become ready within 30s", api_url());
}

async fn register(client: &Client, email: &str, password: &str) -> String {
    let resp = client
        .post(format!("{}/auth/register", api_url()))
        .json(&json!({
            "email": email,
            "display_name": "topology-test-user",
            "password": password
        }))
        .send()
        .await
        .expect("register request failed");
    assert_eq!(resp.status().as_u16(), 201, "register should return 201");
    let body: Value = resp.json().await.unwrap();
    body["session_id"].as_str().unwrap().to_string()
}

async fn post_json(client: &Client, uri: &str, session_id: &str, body: Value) -> Value {
    client
        .post(format!("{}{uri}", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .json(&body)
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST {uri} failed: {e}"))
        .json()
        .await
        .unwrap_or_else(|e| panic!("POST {uri} response body was not JSON: {e}"))
}

async fn post_no_body(client: &Client, uri: &str, session_id: &str, body: Value) {
    let resp = client
        .post(format!("{}{uri}", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .json(&body)
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST {uri} failed: {e}"));
    assert!(
        resp.status().is_success(),
        "POST {uri} returned {}",
        resp.status()
    );
}

async fn get_json(client: &Client, uri: &str, session_id: &str) -> Value {
    client
        .get(format!("{}{uri}", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .send()
        .await
        .unwrap_or_else(|e| panic!("GET {uri} failed: {e}"))
        .json()
        .await
        .unwrap_or_else(|e| panic!("GET {uri} response body was not JSON: {e}"))
}

// ── Seed ──────────────────────────────────────────────────────────────────────

/// Seeds global mirror rows for the two Hannover Weekly players and their shared events.
/// This replaces the old start.gg API key requirement — the global mirror is seeded
/// directly into the DB so the import job can find data without hitting start.gg.
async fn seed_topology_data(pool: &PgPool) {
    // Insert two players using their known start.gg user IDs
    // (These are the real IDs for the Hannover Weekly test players)
    let p1_id: i64 = 1823808; // user/06b4042d
    let p2_id: i64 = 3619891; // user/54b7bbf3

    sqlx::query!(
        "INSERT INTO global_players (startgg_user_id, handle) VALUES ($1, '06b4042d'), ($2, '54b7bbf3')
         ON CONFLICT (startgg_user_id) DO NOTHING",
        p1_id,
        p2_id,
    )
    .execute(pool)
    .await
    .expect("failed to seed global_players");

    // Insert the tournament + event + entries + a set for at least one of the Hannover Weeklies
    let tournament_id = sqlx::query_scalar!(
        r#"INSERT INTO global_tournaments (startgg_id, name, slug, online, start_at)
           VALUES (612663, 'Smash Hannover Weekly #100', 'tournament/smash-hannover-weekly-100', false, '2025-11-10')
           ON CONFLICT (startgg_id) DO UPDATE SET name = EXCLUDED.name
           RETURNING id"#,
    )
    .fetch_one(pool)
    .await
    .expect("failed to seed global_tournament");

    let event_id = sqlx::query_scalar!(
        r#"INSERT INTO global_events (startgg_id, tournament_id, name, state)
           VALUES (1534512, $1, 'Melee Singles', 'COMPLETED')
           ON CONFLICT (startgg_id) DO UPDATE SET name = EXCLUDED.name
           RETURNING id"#,
        tournament_id,
    )
    .fetch_one(pool)
    .await
    .expect("failed to seed global_event");

    let p1_gp = sqlx::query_scalar!(
        "SELECT id FROM global_players WHERE startgg_user_id = $1",
        p1_id
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let p2_gp = sqlx::query_scalar!(
        "SELECT id FROM global_players WHERE startgg_user_id = $1",
        p2_id
    )
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query!(
        "INSERT INTO global_event_entries (event_id, player_id, seed, placement) VALUES ($1, $2, 1, 2), ($1, $3, 2, 1)
         ON CONFLICT DO NOTHING",
        event_id,
        p1_gp,
        p2_gp,
    )
    .execute(pool)
    .await
    .expect("failed to seed entries");

    sqlx::query!(
        r#"INSERT INTO global_sets (startgg_id, event_id, winner_player_id, loser_player_id, is_dq, completed_at)
           VALUES (9999901, $1, $2, $3, false, NOW())
           ON CONFLICT DO NOTHING"#,
        event_id,
        p2_gp,
        p1_gp,
    )
    .execute(pool)
    .await
    .expect("failed to seed set");
}

// ── Test ─────────────────────────────────────────────────────────────────────

/// Exercises the full job-queue path: api inserts job + sends NOTIFY → worker
/// wakes via PgListener → claims job with SELECT ... FOR UPDATE SKIP LOCKED →
/// import runs → job marked done → data visible via API.
///
/// Requires: running api (default http://localhost:3000), running worker,
/// DATABASE_URL in the environment, and the global mirror tables pre-seeded
/// (this test seeds them itself via seed_topology_data).
#[tokio::test]
async fn smoke_import_roundtrip() {
    let client = Client::new();
    wait_for_api(&client).await;

    // Seed global mirror data so the import job can find events
    let pool = PgPool::connect(&db_url())
        .await
        .expect("failed to connect to DB");
    seed_topology_data(&pool).await;

    let unique_email = format!(
        "topology-{}@test.com",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let session_id = register(&client, &unique_email, "password1234").await;

    // No API key setup needed — project creation is now ungated
    let project = post_json(
        &client,
        "/projects",
        &session_id,
        json!({ "name": "Topology Smoke Test", "game_id": 1, "game_name": "Super Smash Bros. Melee" }),
    )
    .await;
    let project_id = project["id"].as_str().unwrap().to_string();

    // Add players and link their seeded global accounts
    let mut player_ids: Vec<String> = Vec::new();
    for (name, slug) in [("Player1", PLAYER1_SLUG), ("Player2", PLAYER2_SLUG)] {
        let player = post_json(
            &client,
            &format!("/projects/{project_id}/players"),
            &session_id,
            json!({ "name": name }),
        )
        .await;
        let player_id = player["id"].as_str().unwrap().to_string();
        post_json(
            &client,
            &format!("/projects/{project_id}/players/{player_id}/accounts"),
            &session_id,
            json!({ "handle": slug.trim_start_matches("user/") }),
        )
        .await;
        player_ids.push(player_id);
    }

    // Create ranking and add players
    let ranking = post_json(
        &client,
        &format!("/projects/{project_id}/rankings"),
        &session_id,
        json!({ "name": "Topology Smoke Ranking" }),
    )
    .await;
    let ranking_id = ranking["id"].as_str().unwrap().to_string();
    for player_id in &player_ids {
        post_no_body(
            &client,
            &format!("/projects/{project_id}/rankings/{ranking_id}/players"),
            &session_id,
            json!({ "player_id": player_id }),
        )
        .await;
    }

    // Trigger import
    let resp = client
        .post(format!("{}/projects/{project_id}/import", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .json(&json!({}))
        .send()
        .await
        .expect("POST import failed");
    assert!(
        resp.status().is_success(),
        "POST /import returned {}",
        resp.status()
    );

    // Poll for completion
    let mut last_status = String::from("unknown");
    for _ in 0..300 {
        sleep(Duration::from_secs(2)).await;
        let import = get_json(
            &client,
            &format!("/projects/{project_id}/import"),
            &session_id,
        )
        .await;
        last_status = import["status"].as_str().unwrap_or("unknown").to_string();
        match last_status.as_str() {
            "done" => break,
            "failed" => panic!(
                "import failed: {}",
                import["error"].as_str().unwrap_or("(no error)")
            ),
            _ => {}
        }
    }
    assert_eq!(last_status, "done", "import did not complete within 600s");

    // Assert tournament appears
    let tournaments = get_json(
        &client,
        &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"),
        &session_id,
    )
    .await;
    let names: Vec<&str> = tournaments
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(
        names
            .iter()
            .any(|n| *n == WEEKLY_100_NAME || *n == WEEKLY_88_NAME),
        "expected a Hannover Weekly in tournaments; got: {:?}",
        names
    );

    // Assert at least one set in stats
    let stats = get_json(
        &client,
        &format!("/projects/{project_id}/rankings/{ranking_id}/stats"),
        &session_id,
    )
    .await;
    let total_sets: usize = stats
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            p["wins"].as_array().map(|a| a.len()).unwrap_or(0)
                + p["losses"].as_array().map(|a| a.len()).unwrap_or(0)
        })
        .sum();
    assert!(total_sets > 0, "expected at least one set in stats");
}
