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
