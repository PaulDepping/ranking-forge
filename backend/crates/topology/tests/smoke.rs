#![cfg(feature = "topology-tests")]

use reqwest::Client;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::sleep;

// ── Golden dataset (mirrors import_live.rs) ───────────────────────────────────
// These are completed past Smash Hannover Weeklies — data is immutable.

const PLAYER1_SLUG: &str = "user/06b4042d"; // gamerTag: "King"
const PLAYER2_SLUG: &str = "user/54b7bbf3";
const WEEKLY_100_NAME: &str = "Smash Hannover Weekly #100";
const WEEKLY_88_NAME: &str = "Smash Hannover Weekly #88";

// ── Helpers ───────────────────────────────────────────────────────────────────

fn api_url() -> String {
    std::env::var("API_URL").unwrap_or_else(|_| "http://localhost:3000".to_string())
}

fn startgg_api_key() -> String {
    std::env::var("STARTGG_API_KEY").expect("STARTGG_API_KEY must be set to run topology tests")
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

// ── Test ─────────────────────────────────────────────────────────────────────

/// Exercises the full job-queue path: api inserts job + sends NOTIFY → worker
/// wakes via PgListener → claims job with SELECT ... FOR UPDATE SKIP LOCKED →
/// import runs → job marked done → data visible via API.
///
/// Requires: running api (default http://localhost:3000), running worker,
/// and STARTGG_API_KEY in the environment.
#[tokio::test]
async fn smoke_import_roundtrip() {
    let key = startgg_api_key();
    let client = Client::new();

    // 1. Wait for API to be up
    wait_for_api(&client).await;

    // 2. Register a user (unique email so repeated runs against the same DB succeed)
    let unique_email = format!(
        "topology-{}@test.com",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    let session_id = register(&client, &unique_email, "password1234").await;

    // 3. Set the start.gg API key (endpoint validates key against start.gg)
    let resp = client
        .put(format!("{}/account/startgg-key", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .json(&json!({ "api_key": key }))
        .send()
        .await
        .expect("PUT /account/startgg-key failed");
    assert_eq!(
        resp.status().as_u16(),
        204,
        "PUT /account/startgg-key should return 204"
    );

    // 4. Create a Melee project
    let project = post_json(
        &client,
        "/projects",
        &session_id,
        json!({
            "name": "Topology Smoke Test",
            "game_id": 1,
            "game_name": "Super Smash Bros. Melee"
        }),
    )
    .await;
    let project_id = project["id"]
        .as_str()
        .expect("project.id missing")
        .to_string();

    // 5. Add two Hannover players with their start.gg accounts
    let mut player_ids: Vec<String> = Vec::new();
    for (name, slug) in [("Player1", PLAYER1_SLUG), ("Player2", PLAYER2_SLUG)] {
        let player = post_json(
            &client,
            &format!("/projects/{project_id}/players"),
            &session_id,
            json!({ "name": name }),
        )
        .await;
        let player_id = player["id"]
            .as_str()
            .expect("player.id missing")
            .to_string();
        post_json(
            &client,
            &format!("/projects/{project_id}/players/{player_id}/accounts"),
            &session_id,
            json!({ "handle": slug }),
        )
        .await;
        player_ids.push(player_id);
    }

    // 5b. Create a ranking and add both players to it
    let ranking = post_json(
        &client,
        &format!("/projects/{project_id}/rankings"),
        &session_id,
        json!({ "name": "Topology Smoke Ranking" }),
    )
    .await;
    let ranking_id = ranking["id"]
        .as_str()
        .expect("ranking.id missing")
        .to_string();
    for player_id in &player_ids {
        post_no_body(
            &client,
            &format!("/projects/{project_id}/rankings/{ranking_id}/players"),
            &session_id,
            json!({ "player_id": player_id }),
        )
        .await;
    }

    // 6. Trigger import — api inserts job row and sends NOTIFY jobs.
    // Scope to the window around Hannover Weekly #84 and #88 to avoid fetching
    // the full tournament history (which would take far longer than the 120 s
    // timeout).  This mirrors the date range used in import_live.rs.
    let resp = client
        .post(format!("{}/projects/{project_id}/import", api_url()))
        .header("cookie", format!("session_id={session_id}"))
        .json(&json!({
            "after_date": "2025-10-27",
            "before_date": "2025-12-10"
        }))
        .send()
        .await
        .expect("POST /projects/{project_id}/import failed");
    assert!(
        resp.status().is_success(),
        "POST /import returned {}",
        resp.status()
    );

    // 7. Poll for job completion — up to 600s (300 × 2s)
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
                "import job failed: {}",
                import["error"].as_str().unwrap_or("(no error message)")
            ),
            _ => {} // "pending" or "running" — keep polling
        }
    }
    assert_eq!(
        last_status, "done",
        "import did not complete within 600s (last observed status: {last_status})"
    );

    // 8. Assert at least one known Hannover Weekly is in the tournament list
    let tournaments = get_json(
        &client,
        &format!("/projects/{project_id}/rankings/{ranking_id}/tournaments"),
        &session_id,
    )
    .await;
    let names: Vec<&str> = tournaments
        .as_array()
        .expect("GET /tournaments should return an array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(
        names
            .iter()
            .any(|n| *n == WEEKLY_100_NAME || *n == WEEKLY_88_NAME),
        "expected '{}' or '{}' in tournaments; got: {:?}",
        WEEKLY_100_NAME,
        WEEKLY_88_NAME,
        names
    );

    // 9. Assert at least one set is recorded in stats
    let stats = get_json(
        &client,
        &format!("/projects/{project_id}/rankings/{ranking_id}/stats"),
        &session_id,
    )
    .await;
    let total_sets: usize = stats
        .as_array()
        .expect("GET /stats should return an array")
        .iter()
        .map(|p| {
            p["wins"].as_array().map(|a| a.len()).unwrap_or(0)
                + p["losses"].as_array().map(|a| a.len()).unwrap_or(0)
        })
        .sum();
    assert!(
        total_sets > 0,
        "expected at least one set in stats after import, got 0"
    );
}
