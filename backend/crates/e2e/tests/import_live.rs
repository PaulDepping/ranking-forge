#![cfg(feature = "live-tests")]

use common::startgg::StartggClient;

// ── Discovery helper ──────────────────────────────────────────────────────────
// Run once with a known Hannover player slug to identify golden tournament data.
// Command:
//   DATABASE_URL=postgres://postgres:postgres@localhost:15432/postgres \
//   STARTGG_API_KEY=<your-key> \
//   SQLX_OFFLINE=true \
//   cargo test -p e2e --features live-tests -- discover_hannover_weeklies --nocapture
//
// After collecting output, replace this file with the golden tests in Task 4.

#[tokio::test]
async fn discover_hannover_weeklies() {
    let key = std::env::var("STARTGG_API_KEY")
        .expect("STARTGG_API_KEY must be set to run live tests");

    let player_slug = "user/06b4042d";
    let melee_game_id: i64 = 1;

    let client = StartggClient::new(key);

    let user = client
        .user_by_slug(player_slug)
        .await
        .expect("API call failed")
        .expect("player slug not found — check it is correct");
    eprintln!("User ID: {}  gamerTag: {:?}", user.id, user.gamer_tag());

    // Fetch page 1 of Melee tournaments (50 per page — enough for a local player)
    let page = client
        .tournaments_by_user(user.id, melee_game_id, 1, 50)
        .await
        .expect("tournaments_by_user failed");
    eprintln!(
        "\nTotal pages: {:?}  Tournaments on page 1: {}",
        page.page_info.as_ref().map(|p| p.total_pages),
        page.nodes.len()
    );

    for t in &page.nodes {
        eprintln!("\n=== {} ===", t.name);
        eprintln!("  slug:    {}", t.slug);
        eprintln!("  state:   {:?}", t.state);
        eprintln!("  startAt: {:?}", t.start_at);
        if let Some(events) = &t.events {
            for e in events {
                eprintln!("  event: {} (id: {})", e.name, e.id);
                eprintln!("    numEntrants: {:?}  state: {:?}", e.num_entrants, e.state);
            }
        }
    }
}
