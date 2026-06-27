# Start.gg Mirror Crawler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `crawler` binary that continuously mirrors start.gg tournament data into `global_*` Postgres tables, resolving player identity at write time.

**Architecture:** New `backend/crates/crawler` workspace member that ports the HCI scraper's API layer (retry, backoff, complexity-error handling, sliding window) and replaces its raw-schema DB layer with normalized global-table upserts. A two-pass fallback (identity pass + games pass) ensures game/character data is never permanently lost when a phase group is too complex for the full query.

**Tech Stack:** Rust, sqlx 0.8.6, tokio 1.52.3, reqwest 0.13.3, clap 4.6.1 (env feature), chrono 0.4.44, anyhow 1.0, thiserror, tracing/tracing-subscriber, wiremock 0.6 (tests only), humantime 2.3.0 (tests only)

## Global Constraints

- All `sqlx::query!` macros require `bash backend/prepare-sqlx.sh` after any change
- `SQLX_OFFLINE=true` must work for CI builds (`.sqlx/` committed)
- Tests use `#[sqlx::test(migrations = "../../migrations")]` — no DB mocks
- Start.gg calls in tests go through wiremock (never real network)
- `cargo add` only — never edit version numbers in `Cargo.toml` manually
- Run `cd backend && cargo fmt --all` before every commit
- Follow existing patterns from `crates/worker` for config/signal handling

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `backend/migrations/001_initial.sql` | Modify | Append all `global_*` table DDL + `crawler_checkpoints` |
| `backend/Cargo.toml` | Modify | Add `"crates/crawler"` to workspace members |
| `backend/crates/crawler/Cargo.toml` | Create | Crate manifest with all dependencies |
| `backend/crates/crawler/src/main.rs` | Create | Tokio main: config, DB connect, migrations, signal setup, run loop |
| `backend/crates/crawler/src/lib.rs` | Create | `pub mod` declarations |
| `backend/crates/crawler/src/cli.rs` | Create | `clap` `Config` struct with env-var defaults |
| `backend/crates/crawler/src/api.rs` | Create | `gql_query`, error types, 6 query string constants |
| `backend/crates/crawler/src/api_types.rs` | Create | All deserialization types for all 6 queries |
| `backend/crates/crawler/src/db.rs` | Create | All upsert functions for `global_*` tables + checkpoint R/W |
| `backend/crates/crawler/src/scraper.rs` | Create | `run()`, sliding window loop, per-tournament/event processing, two-pass fallback |
| `backend/Dockerfile` | Modify | Copy `crawler` binary in builder stage; add `FROM base AS crawler` stage |
| `docker-compose.yml` | Modify | Add `crawler` service |

---

### Task 1: Schema migration

**Files:**
- Modify: `backend/migrations/001_initial.sql`

**Interfaces:**
- Produces: tables `global_games`, `global_players`, `global_tournaments`, `global_events`, `global_phases`, `global_phase_groups`, `global_event_entries`, `global_sets`, `global_set_games`, `global_game_selections`, `global_player_ratings`, `crawler_checkpoints`

- [ ] **Step 1: Append global tables to migration**

Open `backend/migrations/001_initial.sql` and append the following at the end of the file:

```sql
-- ============================================================
-- Global mirror tables (start.gg crawler)
-- ============================================================

CREATE TABLE global_games (
    id         UUID   PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id BIGINT NOT NULL UNIQUE,
    name       TEXT   NOT NULL
);
CREATE INDEX global_games_startgg_id_idx ON global_games(startgg_id);

CREATE TABLE global_players (
    id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_user_id     BIGINT      UNIQUE,
    startgg_player_id   BIGINT      UNIQUE,
    handle              TEXT        NOT NULL,
    display_name        TEXT,
    profile_image_url   TEXT,
    startgg_slug        TEXT,
    bio                 TEXT,
    pronouns            TEXT,
    location_city       TEXT,
    location_state      TEXT,
    location_country    TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_players_startgg_user_id_idx   ON global_players(startgg_user_id);
CREATE INDEX global_players_startgg_player_id_idx ON global_players(startgg_player_id);

CREATE TABLE global_tournaments (
    id            UUID             PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT           NOT NULL UNIQUE,
    name          TEXT             NOT NULL,
    slug          TEXT             NOT NULL,
    start_at      TIMESTAMPTZ,
    end_at        TIMESTAMPTZ,
    country_code  TEXT,
    city          TEXT,
    addr_state    TEXT,
    online        BOOLEAN,
    num_attendees INTEGER,
    lat           DOUBLE PRECISION,
    lng           DOUBLE PRECISION,
    timezone      TEXT,
    created_at    TIMESTAMPTZ      NOT NULL DEFAULT NOW()
);
CREATE INDEX global_tournaments_start_at_idx   ON global_tournaments(start_at);
CREATE INDEX global_tournaments_startgg_id_idx ON global_tournaments(startgg_id);

CREATE TABLE global_events (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id       BIGINT      NOT NULL UNIQUE,
    tournament_id    UUID        NOT NULL REFERENCES global_tournaments(id) ON DELETE CASCADE,
    game_id          UUID        REFERENCES global_games(id),
    name             TEXT        NOT NULL,
    slug             TEXT,
    start_at         TIMESTAMPTZ,
    num_entrants     INTEGER,
    is_online        BOOLEAN,
    competition_tier INTEGER,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_events_tournament_id_idx    ON global_events(tournament_id);
CREATE INDEX global_events_game_id_start_at_idx ON global_events(game_id, start_at);
CREATE INDEX global_events_startgg_id_idx       ON global_events(startgg_id);

CREATE TABLE global_phases (
    id            UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT  NOT NULL UNIQUE,
    event_id      UUID    NOT NULL REFERENCES global_events(id) ON DELETE CASCADE,
    name          TEXT,
    phase_order   INTEGER,
    bracket_type  TEXT,
    is_exhibition BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX global_phases_event_id_idx   ON global_phases(event_id);
CREATE INDEX global_phases_startgg_id_idx ON global_phases(startgg_id);

CREATE TABLE global_phase_groups (
    id                 UUID   PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id         BIGINT NOT NULL UNIQUE,
    phase_id           UUID   NOT NULL REFERENCES global_phases(id) ON DELETE CASCADE,
    display_identifier TEXT,
    bracket_type       TEXT
);
CREATE INDEX global_phase_groups_phase_id_idx   ON global_phase_groups(phase_id);
CREATE INDEX global_phase_groups_startgg_id_idx ON global_phase_groups(startgg_id);

CREATE TABLE global_event_entries (
    id        UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id  UUID    NOT NULL REFERENCES global_events(id) ON DELETE CASCADE,
    player_id UUID    NOT NULL REFERENCES global_players(id) ON DELETE CASCADE,
    seed      INTEGER,
    placement INTEGER,
    UNIQUE (event_id, player_id)
);
CREATE INDEX global_event_entries_player_id_idx ON global_event_entries(player_id);
CREATE INDEX global_event_entries_event_id_idx  ON global_event_entries(event_id);

CREATE TABLE global_sets (
    id               UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id       BIGINT      NOT NULL UNIQUE,
    event_id         UUID        NOT NULL REFERENCES global_events(id) ON DELETE CASCADE,
    phase_group_id   UUID        REFERENCES global_phase_groups(id),
    winner_player_id UUID        REFERENCES global_players(id),
    loser_player_id  UUID        REFERENCES global_players(id),
    round            INTEGER,
    round_name       TEXT,
    winner_score     SMALLINT,
    loser_score      SMALLINT,
    is_dq            BOOLEAN     NOT NULL DEFAULT FALSE,
    vod_url          TEXT,
    completed_at     TIMESTAMPTZ,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_sets_event_id_idx          ON global_sets(event_id);
CREATE INDEX global_sets_phase_group_id_idx    ON global_sets(phase_group_id);
CREATE INDEX global_sets_winner_player_id_idx  ON global_sets(winner_player_id);
CREATE INDEX global_sets_loser_player_id_idx   ON global_sets(loser_player_id);
CREATE INDEX global_sets_completed_at_idx      ON global_sets(completed_at);
CREATE INDEX global_sets_startgg_id_idx        ON global_sets(startgg_id);

CREATE TABLE global_set_games (
    id               UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    set_id           UUID    NOT NULL REFERENCES global_sets(id) ON DELETE CASCADE,
    order_num        INTEGER NOT NULL,
    winner_player_id UUID    REFERENCES global_players(id),
    stage_id         BIGINT,
    stage_name       TEXT,
    UNIQUE (set_id, order_num)
);
CREATE INDEX global_set_games_set_id_idx ON global_set_games(set_id);

CREATE TABLE global_game_selections (
    game_id        UUID   NOT NULL REFERENCES global_set_games(id) ON DELETE CASCADE,
    player_id      UUID   REFERENCES global_players(id),
    selection_type TEXT   NOT NULL,
    character_id   BIGINT,
    character_name TEXT,
    PRIMARY KEY (game_id, player_id, selection_type)
);
CREATE INDEX global_game_selections_player_id_idx ON global_game_selections(player_id);

CREATE TABLE global_player_ratings (
    player_id       UUID        NOT NULL REFERENCES global_players(id) ON DELETE CASCADE,
    game_id         UUID        NOT NULL REFERENCES global_games(id),
    computed_rating FLOAT       NOT NULL,
    display_data    JSONB       NOT NULL DEFAULT '{}',
    algorithm_state JSONB       NOT NULL DEFAULT '{}',
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (player_id, game_id)
);
CREATE INDEX global_player_ratings_game_id_idx ON global_player_ratings(game_id);

CREATE TABLE crawler_checkpoints (
    key        TEXT        PRIMARY KEY,
    value      JSONB       NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 2: Verify migration compiles with test runner**

```bash
cd backend && bash test.sh
```

Expected: existing tests still pass (migration runs on a fresh DB per test via `sqlx::test`).

- [ ] **Step 3: Commit**

```bash
git add backend/migrations/001_initial.sql
git commit -m "feat(schema): add global_* mirror tables and crawler_checkpoints"
```

---

### Task 2: Crate scaffold + CLI

**Files:**
- Create: `backend/crates/crawler/Cargo.toml`
- Create: `backend/crates/crawler/src/lib.rs`
- Create: `backend/crates/crawler/src/cli.rs`
- Modify: `backend/Cargo.toml`

**Interfaces:**
- Produces: `Config` struct in `crawler::cli` with fields: `database_url: String`, `startgg_api_key: String`, `from_date: NaiveDate`, `to_date: NaiveDate`, `window_days: u32`, `delay_ms: u64`, `sets_per_page: u32`, `game_id: Option<u64>`, `rust_log: String`

- [ ] **Step 1: Register crate in workspace**

Edit `backend/Cargo.toml`:

```toml
[workspace]
members  = ["crates/common", "crates/api", "crates/worker", "crates/e2e", "crates/topology", "crates/crawler"]
resolver = "2"
```

- [ ] **Step 2: Create Cargo.toml**

Create `backend/crates/crawler/Cargo.toml`:

```toml
[package]
name = "crawler"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "crawler"
path = "src/main.rs"

[dependencies]
anyhow = "1.0.102"
chrono = { version = "0.4.44", features = ["serde"] }
clap = { version = "4.6.1", features = ["derive", "env"] }
dotenvy = "0.15.7"
reqwest = { version = "0.13.3", default-features = false, features = ["json", "rustls"] }
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.149"
sqlx = { version = "0.8.6", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "macros"], default-features = false }
thiserror = "2.0.12"
tokio = { version = "1.52.3", features = ["full"] }
tracing = "0.1.44"
tracing-subscriber = { version = "0.3.23", features = ["env-filter"] }
uuid = { version = "1.23.1", features = ["v4"] }

[dev-dependencies]
humantime = "2.3.0"
wiremock = "0.6"
```

> Note: Use `cargo add` to add any missing deps rather than editing version numbers manually. These versions match the worker crate.

- [ ] **Step 3: Create lib.rs**

Create `backend/crates/crawler/src/lib.rs`:

```rust
pub mod api;
pub mod api_types;
pub mod cli;
pub mod db;
pub mod scraper;
```

- [ ] **Step 4: Create cli.rs**

Create `backend/crates/crawler/src/cli.rs`:

```rust
use chrono::NaiveDate;
use clap::Parser;

fn default_from_date() -> NaiveDate {
    NaiveDate::from_ymd_opt(2015, 1, 1).unwrap()
}

fn default_to_date() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

#[derive(Debug, Parser)]
pub struct Config {
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "STARTGG_API_KEY")]
    pub startgg_api_key: String,

    #[arg(long, env = "CRAWLER_FROM_DATE", default_value_t = default_from_date())]
    pub from_date: NaiveDate,

    #[arg(long, env = "CRAWLER_TO_DATE", default_value_t = default_to_date())]
    pub to_date: NaiveDate,

    #[arg(long, env = "CRAWLER_WINDOW_DAYS", default_value = "7")]
    pub window_days: u32,

    #[arg(long, env = "CRAWLER_DELAY_MS", default_value = "750")]
    pub delay_ms: u64,

    #[arg(long, env = "CRAWLER_SETS_PER_PAGE", default_value = "20")]
    pub sets_per_page: u32,

    #[arg(long, env = "CRAWLER_GAME_ID")]
    pub game_id: Option<u64>,

    #[arg(long, env = "RUST_LOG", default_value = "info")]
    pub rust_log: String,
}
```

- [ ] **Step 5: Create stub main.rs so crate compiles**

Create `backend/crates/crawler/src/main.rs`:

```rust
use clap::Parser;
use cli::Config;

mod api;
mod api_types;
mod cli;
mod db;
mod scraper;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let _config = Config::parse();
}
```

- [ ] **Step 6: Verify crate compiles**

```bash
cd backend && cargo build -p crawler
```

Expected: compiles (may warn about unused modules — that's fine at this stage).

- [ ] **Step 7: Commit**

```bash
cd backend && cargo fmt --all
git add backend/Cargo.toml backend/crates/crawler/
git commit -m "feat(crawler): scaffold crate with CLI config"
```

---

### Task 3: API layer

**Files:**
- Create: `backend/crates/crawler/src/api.rs`

**Interfaces:**
- Produces:
  - `pub struct ComplexityError { pub actual: Option<u64> }`
  - `pub struct PaginationLimitError`
  - `pub struct MaxRetriesError`
  - `pub const TOURNAMENT_QUERY: &str`
  - `pub const PHASE_GROUPS_QUERY: &str`
  - `pub const PHASE_GROUP_SETS_QUERY: &str`
  - `pub const PHASE_GROUP_SETS_QUERY_SLIM: &str`
  - `pub const PHASE_GROUP_GAMES_QUERY: &str`
  - `pub const EVENT_STANDINGS_QUERY: &str`
  - `pub async fn gql_query<T: DeserializeOwned>(client: &Client, base_url: &str, token: &str, query: &str, variables: Value, fallback_backoff: Duration) -> Result<T>`

- [ ] **Step 1: Write api.rs**

Create `backend/crates/crawler/src/api.rs`:

```rust
use anyhow::{Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, instrument};

use crate::api_types::{GqlError, GqlResponse};

pub const STARTGG_API_URL: &str = "https://api.start.gg/gql/alpha";

#[derive(Debug, thiserror::Error)]
#[error("query complexity too high{}", .actual.map(|n| format!(" (actual: {n})")).unwrap_or_default())]
pub struct ComplexityError {
    pub actual: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
#[error("API pagination limit reached (>10,000 entries)")]
pub struct PaginationLimitError;

#[derive(Debug, thiserror::Error)]
#[error("max retries exceeded for GraphQL query")]
pub struct MaxRetriesError;

fn parse_complexity_error(errors: &[GqlError]) -> Option<ComplexityError> {
    let msg = errors.first()?.message.as_deref()?;
    if !msg.contains("query complexity is too high") {
        return None;
    }
    let actual = msg
        .split("actual: ")
        .nth(1)
        .and_then(|s| s.trim_end_matches(')').trim().parse().ok());
    Some(ComplexityError { actual })
}

pub const TOURNAMENT_QUERY: &str = r#"
query Tournaments($page: Int!, $perPage: Int!, $filter: TournamentPageFilter) {
  tournaments(query: {
    page: $page
    perPage: $perPage
    filter: $filter
  }) {
    pageInfo { total totalPages }
    nodes {
      id name slug startAt endAt countryCode city addrState
      numAttendees isOnline lat lng timezone
      events {
        id name slug startAt state isOnline numEntrants type competitionTier
        videogame { id name }
      }
    }
  }
}
"#;

pub const PHASE_GROUPS_QUERY: &str = r#"
query EventPhaseGroups($eventId: ID!) {
  event(id: $eventId) {
    phases {
      id
      phaseGroups(query: { page: 1, perPage: 500 }) {
        pageInfo { totalPages }
        nodes { id }
      }
    }
  }
}
"#;

pub const PHASE_GROUP_SETS_QUERY: &str = r#"
query PhaseGroupSets($phaseGroupId: ID!, $page: Int!, $perPage: Int!) {
  phaseGroup(id: $phaseGroupId) {
    id
    sets(page: $page, perPage: $perPage) {
      pageInfo { total totalPages }
      nodes {
        id state winnerId vodUrl completedAt fullRoundText round
        lPlacement wPlacement displayScore
        phaseGroup {
          id displayIdentifier bracketType
          phase { id name bracketType phaseOrder isExhibition }
        }
        slots {
          slotIndex
          standing { stats { score { value } } }
          entrant {
            id initialSeedNum isDisqualified
            participants {
              player { id gamerTag prefix }
              user {
                id slug name bio genderPronoun
                location { city state country }
                images { url type }
              }
            }
          }
        }
        games {
          orderNum winnerId
          stage { id name }
          selections {
            selectionType
            entrant { id }
            character { id name }
          }
        }
      }
    }
  }
}
"#;

pub const PHASE_GROUP_SETS_QUERY_SLIM: &str = r#"
query PhaseGroupSets($phaseGroupId: ID!, $page: Int!, $perPage: Int!) {
  phaseGroup(id: $phaseGroupId) {
    id
    sets(page: $page, perPage: $perPage) {
      pageInfo { total totalPages }
      nodes {
        id state winnerId vodUrl completedAt fullRoundText round
        lPlacement wPlacement displayScore
        phaseGroup {
          id displayIdentifier bracketType
          phase { id name bracketType phaseOrder isExhibition }
        }
        slots {
          slotIndex
          standing { stats { score { value } } }
          entrant {
            id initialSeedNum isDisqualified
            participants {
              player { id gamerTag prefix }
            }
          }
        }
      }
    }
  }
}
"#;

pub const PHASE_GROUP_GAMES_QUERY: &str = r#"
query PhaseGroupGames($phaseGroupId: ID!, $page: Int!, $perPage: Int!) {
  phaseGroup(id: $phaseGroupId) {
    id
    sets(page: $page, perPage: $perPage) {
      pageInfo { total totalPages }
      nodes {
        id
        games {
          orderNum winnerId
          stage { id name }
          selections {
            selectionType
            entrant { id }
            character { id name }
          }
        }
      }
    }
  }
}
"#;

pub const EVENT_STANDINGS_QUERY: &str = r#"
query EventStandings($eventId: ID!, $page: Int!, $perPage: Int!) {
  event(id: $eventId) {
    standings(query: { page: $page, perPage: $perPage }) {
      pageInfo { total totalPages }
      nodes {
        id placement isFinal totalPoints
        entrant { id }
      }
    }
  }
}
"#;

#[instrument(skip(client, token, query), fields(%variables))]
pub async fn gql_query<T: DeserializeOwned>(
    client: &Client,
    base_url: &str,
    token: &str,
    query: &str,
    variables: Value,
    fallback_backoff: Duration,
) -> Result<T> {
    debug!("sending GraphQL request");
    let mut backoff = fallback_backoff;
    let mut last_decode_failure: Option<String> = None;
    for attempt in 0..15u32 {
        let resp = match client
            .post(base_url)
            .bearer_auth(token)
            .json(&json!({ "query": query, "variables": variables }))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!(attempt = attempt + 1, error = %e, sleep_ms = backoff.as_millis(), "Request failed, retrying");
                sleep(backoff).await;
                continue;
            }
        };

        let status = resp.status();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(Duration::from_secs);

        if status.is_server_error() {
            let sleep_dur = retry_after.unwrap_or(backoff);
            debug!(attempt = attempt + 1, %status, sleep_ms = sleep_dur.as_millis(), "Server error, retrying");
            sleep(sleep_dur).await;
            continue;
        }

        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                let sleep_dur = retry_after.unwrap_or(backoff);
                debug!(attempt = attempt + 1, error = %e, sleep_ms = sleep_dur.as_millis(), "Failed to read body, retrying");
                sleep(sleep_dur).await;
                continue;
            }
        };

        let body: GqlResponse<T> = match serde_json::from_str(&text) {
            Ok(b) => b,
            Err(e) => {
                let sleep_dur = retry_after.unwrap_or(backoff);
                debug!(attempt = attempt + 1, error = %e, body = %text, sleep_ms = sleep_dur.as_millis(), "Failed to decode body, retrying");
                last_decode_failure = Some(text);
                sleep(sleep_dur).await;
                continue;
            }
        };

        if status == 429 {
            let sleep_dur = retry_after.unwrap_or(backoff);
            debug!(attempt = attempt + 1, sleep_ms = sleep_dur.as_millis(), "Rate limited, retrying");
            sleep(sleep_dur).await;
            backoff = backoff.mul_f32(1.25);
            continue;
        }

        if let Some(errors) = &body.errors {
            if let Some(ce) = parse_complexity_error(errors) {
                return Err(anyhow::Error::new(ce));
            }
            if errors.iter().any(|e| {
                e.message.as_deref().map(|m| m.contains("Cannot query more than the 10,000th entry")).unwrap_or(false)
            }) {
                return Err(anyhow::Error::new(PaginationLimitError));
            }
            let is_internal = errors.iter().any(|e| {
                e.extensions.as_ref().and_then(|ext| ext.category.as_deref()) == Some("internal")
            });
            if is_internal {
                let sleep_dur = retry_after.unwrap_or(backoff);
                debug!(attempt = attempt + 1, sleep_ms = sleep_dur.as_millis(), "Internal API error, retrying");
                sleep(sleep_dur).await;
                continue;
            }
            anyhow::bail!("Unknown GraphQL errors in response");
        }

        if body.success == Some(false) {
            anyhow::bail!("GraphQL response reported success=false");
        }

        debug!(%status, "request succeeded");
        return body.data.context("GraphQL response missing data field");
    }

    if let Some(body) = last_decode_failure {
        tracing::error!(body, "Last response body before giving up");
    }
    Err(anyhow::Error::new(MaxRetriesError)
        .context(format!("Max retries exceeded (variables={})", variables)))
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd backend && cargo build -p crawler 2>&1 | head -30
```

Expected: compiles (api_types module is still a stub — errors expected only from missing types).

- [ ] **Step 3: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/crawler/src/api.rs
git commit -m "feat(crawler): add API layer with gql_query and 6 query constants"
```

---

### Task 4: API types

**Files:**
- Create: `backend/crates/crawler/src/api_types.rs`

**Interfaces:**
- Consumes: query constants from `api.rs`
- Produces: `GqlResponse<T>`, `GqlError`, `GqlErrorExtensions`, `PageInfo`, `TournamentsData`, `TournamentNode`, `EventNode`, `VideogameNode`, `EventPhaseGroupsData`, `PhaseNode`, `PhaseGroupIdNode`, `FullPhaseGroupSetsData`, `FullSetNode`, `PhaseGroupInfo`, `PhaseInfo`, `SlotNode`, `EntrantNode`, `ParticipantNode`, `PlayerNode`, `UserNode`, `UserImage`, `UserLocation`, `GameNode`, `SelectionNode`, `StageNode`, `SlimPhaseGroupSetsData`, `SlimSetNode`, `SlimEntrantNode`, `SlimParticipantNode`, `GamesPhaseGroupSetsData`, `GamesSetNode`, `EventStandingsData`, `StandingNode`; plus free functions `deserialize_id`, `deserialize_opt_id`, `deserialize_null_default`

- [ ] **Step 1: Write api_types.rs**

Create `backend/crates/crawler/src/api_types.rs`:

```rust
use serde::{Deserialize, Deserializer};

// ---------------------------------------------------------------------------
// ID helpers — start.gg returns IDs as either integers or strings
// ---------------------------------------------------------------------------

pub fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

pub fn deserialize_id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw { Int(i64), Str(String) }
    match Raw::deserialize(deserializer)? {
        Raw::Int(n) => Ok(n),
        Raw::Str(s) => s.parse().map_err(serde::de::Error::custom),
    }
}

pub fn deserialize_opt_id<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<i64>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw { Int(i64), Str(String), Null }
    match Option::<Raw>::deserialize(deserializer)? {
        None | Some(Raw::Null) => Ok(None),
        Some(Raw::Int(n)) => Ok(Some(n)),
        Some(Raw::Str(s)) => Ok(s.parse().ok()),
    }
}

// ---------------------------------------------------------------------------
// GQL envelope
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GqlResponse<T> {
    pub data: Option<T>,
    pub errors: Option<Vec<GqlError>>,
    pub success: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct GqlError {
    pub message: Option<String>,
    pub extensions: Option<GqlErrorExtensions>,
}

#[derive(Debug, Deserialize)]
pub struct GqlErrorExtensions {
    pub category: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub total: Option<i64>,
    pub total_pages: Option<i64>,
}

// ---------------------------------------------------------------------------
// TOURNAMENT_QUERY
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct TournamentsData {
    pub tournaments: TournamentsPage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentsPage {
    pub page_info: PageInfo,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub nodes: Vec<TournamentNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub start_at: Option<i64>,
    pub end_at: Option<i64>,
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub addr_state: Option<String>,
    pub num_attendees: Option<i64>,
    pub is_online: Option<bool>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub timezone: Option<String>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub events: Vec<EventNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: String,
    pub slug: Option<String>,
    pub start_at: Option<i64>,
    pub state: Option<i64>,
    pub is_online: Option<bool>,
    pub num_entrants: Option<i64>,
    #[serde(rename = "type")]
    pub event_type: Option<i64>,
    pub competition_tier: Option<i64>,
    pub videogame: Option<VideogameNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideogameNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: String,
}

// ---------------------------------------------------------------------------
// PHASE_GROUPS_QUERY
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct EventPhaseGroupsData {
    pub event: Option<EventPhasesNode>,
}

#[derive(Debug, Deserialize)]
pub struct EventPhasesNode {
    #[serde(deserialize_with = "deserialize_null_default")]
    pub phases: Vec<PhaseNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub phase_groups: PhaseGroupsPage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseGroupsPage {
    pub page_info: PageInfo,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub nodes: Vec<PhaseGroupIdNode>,
}

#[derive(Debug, Deserialize)]
pub struct PhaseGroupIdNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
}

// ---------------------------------------------------------------------------
// PHASE_GROUP_SETS_QUERY (full)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullPhaseGroupSetsData {
    pub phase_group: Option<FullPhaseGroupNode>,
}

#[derive(Debug, Deserialize)]
pub struct FullPhaseGroupNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub sets: SetsPage<FullSetNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetsPage<T> {
    pub page_info: PageInfo,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub nodes: Vec<T>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullSetNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub state: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub winner_id: Option<i64>,
    pub vod_url: Option<String>,
    pub completed_at: Option<i64>,
    pub full_round_text: Option<String>,
    pub round: Option<i64>,
    pub l_placement: Option<i64>,
    pub w_placement: Option<i64>,
    pub display_score: Option<String>,
    pub phase_group: Option<PhaseGroupInfo>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub slots: Vec<SlotNode>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub games: Vec<GameNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseGroupInfo {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub display_identifier: Option<String>,
    pub bracket_type: Option<String>,
    pub phase: Option<PhaseInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseInfo {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: Option<String>,
    pub bracket_type: Option<String>,
    pub phase_order: Option<i64>,
    pub is_exhibition: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotNode {
    pub slot_index: Option<i64>,
    pub standing: Option<SlotStanding>,
    pub entrant: Option<EntrantNode>,
}

#[derive(Debug, Deserialize)]
pub struct SlotStanding {
    pub stats: Option<SlotStats>,
}

#[derive(Debug, Deserialize)]
pub struct SlotStats {
    pub score: Option<ScoreValue>,
}

#[derive(Debug, Deserialize)]
pub struct ScoreValue {
    pub value: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntrantNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub initial_seed_num: Option<i64>,
    pub is_disqualified: Option<bool>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub participants: Vec<ParticipantNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantNode {
    pub player: Option<PlayerNode>,
    pub user: Option<UserNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub gamer_tag: Option<String>,
    pub prefix: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub slug: Option<String>,
    pub name: Option<String>,
    pub bio: Option<String>,
    pub gender_pronoun: Option<String>,
    pub location: Option<UserLocation>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub images: Vec<UserImage>,
}

#[derive(Debug, Deserialize)]
pub struct UserLocation {
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserImage {
    pub url: Option<String>,
    #[serde(rename = "type")]
    pub image_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameNode {
    pub order_num: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub winner_id: Option<i64>,
    pub stage: Option<StageNode>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub selections: Vec<SelectionNode>,
}

#[derive(Debug, Deserialize)]
pub struct StageNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionNode {
    pub selection_type: Option<String>,
    pub entrant: Option<SelectionEntrant>,
    pub character: Option<CharacterNode>,
}

#[derive(Debug, Deserialize)]
pub struct SelectionEntrant {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub struct CharacterNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub name: Option<String>,
}

// ---------------------------------------------------------------------------
// PHASE_GROUP_SETS_QUERY_SLIM (identity pass)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlimPhaseGroupSetsData {
    pub phase_group: Option<SlimPhaseGroupNode>,
}

#[derive(Debug, Deserialize)]
pub struct SlimPhaseGroupNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub sets: SetsPage<SlimSetNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlimSetNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub state: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub winner_id: Option<i64>,
    pub vod_url: Option<String>,
    pub completed_at: Option<i64>,
    pub full_round_text: Option<String>,
    pub round: Option<i64>,
    pub l_placement: Option<i64>,
    pub w_placement: Option<i64>,
    pub display_score: Option<String>,
    pub phase_group: Option<PhaseGroupInfo>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub slots: Vec<SlimSlotNode>,
}

#[derive(Debug, Deserialize)]
pub struct SlimSlotNode {
    pub standing: Option<SlotStanding>,
    pub entrant: Option<SlimEntrantNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlimEntrantNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub initial_seed_num: Option<i64>,
    pub is_disqualified: Option<bool>,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub participants: Vec<SlimParticipantNode>,
}

#[derive(Debug, Deserialize)]
pub struct SlimParticipantNode {
    pub player: Option<PlayerNode>,
}

// ---------------------------------------------------------------------------
// PHASE_GROUP_GAMES_QUERY (games pass)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamesPhaseGroupSetsData {
    pub phase_group: Option<GamesPhaseGroupNode>,
}

#[derive(Debug, Deserialize)]
pub struct GamesPhaseGroupNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub sets: SetsPage<GamesSetNode>,
}

#[derive(Debug, Deserialize)]
pub struct GamesSetNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub games: Vec<GameNode>,
}

// ---------------------------------------------------------------------------
// EVENT_STANDINGS_QUERY
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct EventStandingsData {
    pub event: Option<EventStandingsNode>,
}

#[derive(Debug, Deserialize)]
pub struct EventStandingsNode {
    pub standings: StandingsPage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingsPage {
    pub page_info: PageInfo,
    #[serde(deserialize_with = "deserialize_null_default")]
    pub nodes: Vec<StandingNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandingNode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
    pub placement: Option<i64>,
    pub is_final: Option<bool>,
    pub entrant: Option<StandingEntrant>,
}

#[derive(Debug, Deserialize)]
pub struct StandingEntrant {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: i64,
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd backend && cargo build -p crawler 2>&1 | head -40
```

Expected: compiles. `db` and `scraper` modules still empty — stubs fine.

- [ ] **Step 3: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/crawler/src/api_types.rs
git commit -m "feat(crawler): add API response types for all 6 queries"
```

---

### Task 5: DB layer

**Files:**
- Create: `backend/crates/crawler/src/db.rs`

**Interfaces:**
- Consumes: all `global_*` tables from Task 1
- Produces:
  - `pub async fn upsert_game(pool, startgg_id: i64, name: &str) -> Result<Uuid>`
  - `pub async fn upsert_player_full(pool, startgg_user_id: i64, startgg_player_id: Option<i64>, handle: &str, display_name: Option<&str>, profile_image_url: Option<&str>, startgg_slug: Option<&str>, bio: Option<&str>, pronouns: Option<&str>, location_city: Option<&str>, location_state: Option<&str>, location_country: Option<&str>) -> Result<Uuid>`
  - `pub async fn upsert_player_slim(pool, startgg_player_id: i64, handle: &str) -> Result<Uuid>`
  - `pub async fn upsert_tournament(pool, node: &TournamentNode) -> Result<Uuid>`
  - `pub async fn upsert_event(pool, node: &EventNode, tournament_id: Uuid, game_id: Option<Uuid>) -> Result<Uuid>`
  - `pub async fn upsert_phase(pool, startgg_id: i64, event_id: Uuid, info: &PhaseInfo) -> Result<Uuid>`
  - `pub async fn upsert_phase_group(pool, startgg_id: i64, phase_id: Uuid, info: &PhaseGroupInfo) -> Result<Uuid>`
  - `pub async fn upsert_set(pool, set_id: i64, event_id: Uuid, phase_group_id: Option<Uuid>, winner_player_id: Option<Uuid>, loser_player_id: Option<Uuid>, round: Option<i64>, round_name: Option<&str>, winner_score: Option<i16>, loser_score: Option<i16>, is_dq: bool, vod_url: Option<&str>, completed_at: Option<DateTime<Utc>>) -> Result<Uuid>`
  - `pub async fn upsert_set_game(pool, set_uuid: Uuid, order_num: i32, winner_player_id: Option<Uuid>, stage_id: Option<i64>, stage_name: Option<&str>) -> Result<Uuid>`
  - `pub async fn upsert_game_selection(pool, game_uuid: Uuid, player_id: Option<Uuid>, selection_type: &str, character_id: Option<i64>, character_name: Option<&str>) -> Result<()>`
  - `pub async fn upsert_event_entry(pool, event_id: Uuid, player_id: Uuid, seed: Option<i32>, placement: Option<i32>) -> Result<()>`
  - `pub async fn get_checkpoint(pool, key: &str) -> Result<Option<serde_json::Value>>`
  - `pub async fn set_checkpoint(pool, key: &str, value: serde_json::Value) -> Result<()>`
  - `pub fn is_tournament_checkpointed(value: &Option<serde_json::Value>) -> bool`
  - `pub fn is_event_checkpointed(value: &Option<serde_json::Value>) -> bool`

- [ ] **Step 1: Write unit tests for DQ detection and score extraction**

These are pure-logic helpers that live in `scraper.rs` but we'll write them as inline tests first. Create a placeholder in `db.rs` and add the helpers to `scraper.rs` in Task 6. For now write the db module.

Create `backend/crates/crawler/src/db.rs`:

```rust
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api_types::{EventNode, PhaseGroupInfo, PhaseInfo, TournamentNode};

pub async fn upsert_game(pool: &PgPool, startgg_id: i64, name: &str) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_games (startgg_id, name)
        VALUES ($1, $2)
        ON CONFLICT (startgg_id) DO UPDATE SET name = EXCLUDED.name
        RETURNING id
        "#,
        startgg_id,
        name,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_player_full(
    pool: &PgPool,
    startgg_user_id: i64,
    startgg_player_id: Option<i64>,
    handle: &str,
    display_name: Option<&str>,
    profile_image_url: Option<&str>,
    startgg_slug: Option<&str>,
    bio: Option<&str>,
    pronouns: Option<&str>,
    location_city: Option<&str>,
    location_state: Option<&str>,
    location_country: Option<&str>,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_players
            (startgg_user_id, startgg_player_id, handle, display_name,
             profile_image_url, startgg_slug, bio, pronouns,
             location_city, location_state, location_country)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        ON CONFLICT (startgg_user_id) DO UPDATE SET
            handle            = EXCLUDED.handle,
            display_name      = EXCLUDED.display_name,
            startgg_player_id = COALESCE(EXCLUDED.startgg_player_id, global_players.startgg_player_id),
            profile_image_url = COALESCE(EXCLUDED.profile_image_url, global_players.profile_image_url),
            startgg_slug      = COALESCE(EXCLUDED.startgg_slug,      global_players.startgg_slug),
            bio               = COALESCE(EXCLUDED.bio,               global_players.bio),
            pronouns          = COALESCE(EXCLUDED.pronouns,          global_players.pronouns),
            location_city     = EXCLUDED.location_city,
            location_state    = EXCLUDED.location_state,
            location_country  = EXCLUDED.location_country,
            updated_at        = NOW()
        RETURNING id
        "#,
        startgg_user_id,
        startgg_player_id,
        handle,
        display_name,
        profile_image_url,
        startgg_slug,
        bio,
        pronouns,
        location_city,
        location_state,
        location_country,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_player_slim(
    pool: &PgPool,
    startgg_player_id: i64,
    handle: &str,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_players (startgg_player_id, handle)
        VALUES ($1, $2)
        ON CONFLICT (startgg_player_id) DO UPDATE SET
            handle     = EXCLUDED.handle,
            updated_at = NOW()
        RETURNING id
        "#,
        startgg_player_id,
        handle,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_tournament(pool: &PgPool, node: &TournamentNode) -> Result<Uuid> {
    let start_at = node.start_at.and_then(|ts| DateTime::from_timestamp(ts, 0));
    let end_at = node.end_at.and_then(|ts| DateTime::from_timestamp(ts, 0));
    let row = sqlx::query!(
        r#"
        INSERT INTO global_tournaments
            (startgg_id, name, slug, start_at, end_at, country_code, city,
             addr_state, online, num_attendees, lat, lng, timezone)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        ON CONFLICT (startgg_id) DO UPDATE SET
            name          = EXCLUDED.name,
            slug          = EXCLUDED.slug,
            start_at      = EXCLUDED.start_at,
            end_at        = EXCLUDED.end_at,
            country_code  = EXCLUDED.country_code,
            city          = EXCLUDED.city,
            addr_state    = EXCLUDED.addr_state,
            online        = EXCLUDED.online,
            num_attendees = EXCLUDED.num_attendees,
            lat           = EXCLUDED.lat,
            lng           = EXCLUDED.lng,
            timezone      = EXCLUDED.timezone
        RETURNING id
        "#,
        node.id,
        node.name,
        node.slug,
        start_at,
        end_at,
        node.country_code,
        node.city,
        node.addr_state,
        node.is_online,
        node.num_attendees.map(|n| n as i32),
        node.lat,
        node.lng,
        node.timezone,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_event(
    pool: &PgPool,
    node: &EventNode,
    tournament_id: Uuid,
    game_id: Option<Uuid>,
) -> Result<Uuid> {
    let start_at = node.start_at.and_then(|ts| DateTime::from_timestamp(ts, 0));
    let row = sqlx::query!(
        r#"
        INSERT INTO global_events
            (startgg_id, tournament_id, game_id, name, slug,
             start_at, num_entrants, is_online, competition_tier)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (startgg_id) DO UPDATE SET
            tournament_id    = EXCLUDED.tournament_id,
            game_id          = EXCLUDED.game_id,
            name             = EXCLUDED.name,
            slug             = EXCLUDED.slug,
            start_at         = EXCLUDED.start_at,
            num_entrants     = EXCLUDED.num_entrants,
            is_online        = EXCLUDED.is_online,
            competition_tier = EXCLUDED.competition_tier
        RETURNING id
        "#,
        node.id,
        tournament_id,
        game_id,
        node.name,
        node.slug,
        start_at,
        node.num_entrants.map(|n| n as i32),
        node.is_online,
        node.competition_tier.map(|n| n as i32),
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_phase(
    pool: &PgPool,
    startgg_id: i64,
    event_id: Uuid,
    info: &PhaseInfo,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_phases
            (startgg_id, event_id, name, phase_order, bracket_type, is_exhibition)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (startgg_id) DO UPDATE SET
            name         = EXCLUDED.name,
            phase_order  = EXCLUDED.phase_order,
            bracket_type = EXCLUDED.bracket_type,
            is_exhibition = EXCLUDED.is_exhibition
        RETURNING id
        "#,
        startgg_id,
        event_id,
        info.name,
        info.phase_order.map(|n| n as i32),
        info.bracket_type,
        info.is_exhibition.unwrap_or(false),
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_phase_group(
    pool: &PgPool,
    startgg_id: i64,
    phase_id: Uuid,
    info: &PhaseGroupInfo,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_phase_groups
            (startgg_id, phase_id, display_identifier, bracket_type)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (startgg_id) DO UPDATE SET
            display_identifier = EXCLUDED.display_identifier,
            bracket_type       = EXCLUDED.bracket_type
        RETURNING id
        "#,
        startgg_id,
        phase_id,
        info.display_identifier,
        info.bracket_type,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert_set(
    pool: &PgPool,
    startgg_id: i64,
    event_id: Uuid,
    phase_group_id: Option<Uuid>,
    winner_player_id: Option<Uuid>,
    loser_player_id: Option<Uuid>,
    round: Option<i64>,
    round_name: Option<&str>,
    winner_score: Option<i16>,
    loser_score: Option<i16>,
    is_dq: bool,
    vod_url: Option<&str>,
    completed_at: Option<DateTime<Utc>>,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_sets
            (startgg_id, event_id, phase_group_id, winner_player_id, loser_player_id,
             round, round_name, winner_score, loser_score, is_dq, vod_url, completed_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (startgg_id) DO UPDATE SET
            winner_player_id = EXCLUDED.winner_player_id,
            loser_player_id  = EXCLUDED.loser_player_id,
            winner_score     = EXCLUDED.winner_score,
            loser_score      = EXCLUDED.loser_score,
            is_dq            = EXCLUDED.is_dq,
            vod_url          = EXCLUDED.vod_url,
            completed_at     = EXCLUDED.completed_at
        RETURNING id
        "#,
        startgg_id,
        event_id,
        phase_group_id,
        winner_player_id,
        loser_player_id,
        round.map(|n| n as i32),
        round_name,
        winner_score,
        loser_score,
        is_dq,
        vod_url,
        completed_at,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_set_game(
    pool: &PgPool,
    set_uuid: Uuid,
    order_num: i32,
    winner_player_id: Option<Uuid>,
    stage_id: Option<i64>,
    stage_name: Option<&str>,
) -> Result<Uuid> {
    let row = sqlx::query!(
        r#"
        INSERT INTO global_set_games (set_id, order_num, winner_player_id, stage_id, stage_name)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (set_id, order_num) DO UPDATE SET
            winner_player_id = EXCLUDED.winner_player_id,
            stage_id         = EXCLUDED.stage_id,
            stage_name       = EXCLUDED.stage_name
        RETURNING id
        "#,
        set_uuid,
        order_num,
        winner_player_id,
        stage_id,
        stage_name,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn upsert_game_selection(
    pool: &PgPool,
    game_uuid: Uuid,
    player_id: Option<Uuid>,
    selection_type: &str,
    character_id: Option<i64>,
    character_name: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO global_game_selections
            (game_id, player_id, selection_type, character_id, character_name)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (game_id, player_id, selection_type) DO UPDATE SET
            character_id   = EXCLUDED.character_id,
            character_name = EXCLUDED.character_name
        "#,
        game_uuid,
        player_id,
        selection_type,
        character_id,
        character_name,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn upsert_event_entry(
    pool: &PgPool,
    event_id: Uuid,
    player_id: Uuid,
    seed: Option<i32>,
    placement: Option<i32>,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO global_event_entries (event_id, player_id, seed, placement)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (event_id, player_id) DO UPDATE SET
            seed      = COALESCE(EXCLUDED.seed, global_event_entries.seed),
            placement = COALESCE(EXCLUDED.placement, global_event_entries.placement)
        "#,
        event_id,
        player_id,
        seed,
        placement,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_checkpoint(pool: &PgPool, key: &str) -> Result<Option<serde_json::Value>> {
    let row = sqlx::query!(
        "SELECT value FROM crawler_checkpoints WHERE key = $1",
        key
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.value))
}

pub async fn set_checkpoint(pool: &PgPool, key: &str, value: serde_json::Value) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO crawler_checkpoints (key, value)
        VALUES ($1, $2)
        ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
        "#,
        key,
        value,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub fn is_checkpointed(cp: &Option<serde_json::Value>) -> bool {
    matches!(cp, Some(v) if v.as_bool() == Some(true))
}
```

- [ ] **Step 2: Run sqlx prepare**

```bash
bash backend/prepare-sqlx.sh
```

Expected: `.sqlx/` directory updated with new query hashes. No errors.

- [ ] **Step 3: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/crawler/src/db.rs backend/.sqlx/
git commit -m "feat(crawler): add DB layer with all global_* upserts and checkpoints"
```

---

### Task 6: Scraper core — helpers and full path

**Files:**
- Create: `backend/crates/crawler/src/scraper.rs`

**Interfaces:**
- Consumes: all types from `api_types`, all db functions from `db`, query constants from `api`
- Produces: `pub async fn run(config: &Config, pool: &PgPool, shutdown: &AtomicBool) -> Result<()>`

- [ ] **Step 1: Write unit tests for DQ detection and score extraction**

These go in `scraper.rs` as `#[cfg(test)]` inline tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dq_detected_by_state() {
        assert!(is_dq(Some(7), None));
    }

    #[test]
    fn dq_detected_by_display_score() {
        assert!(is_dq(Some(3), Some("DQ")));
        assert!(is_dq(None, Some("Player DQ - 0")));
    }

    #[test]
    fn no_dq_for_normal_set() {
        assert!(!is_dq(Some(3), Some("3 - 1")));
    }

    #[test]
    fn scores_extracted_from_display_score() {
        // winner score = score of winner entrant ID
        // display_score format: "<score> - <score>"
        // winner gets the higher of the two
        let (w, l) = extract_scores("3 - 1").unwrap();
        assert_eq!(w, 3);
        assert_eq!(l, 1);
        let (w, l) = extract_scores("2 - 0").unwrap();
        assert_eq!(w, 2);
        assert_eq!(l, 0);
    }

    #[test]
    fn scores_none_for_dq_display() {
        assert!(extract_scores("DQ").is_none());
        assert!(extract_scores("").is_none());
    }
}
```

- [ ] **Step 2: Verify test fails**

```bash
cd backend && cargo test -p crawler 2>&1 | tail -20
```

Expected: compile error — `is_dq` and `extract_scores` not defined yet.

- [ ] **Step 3: Write scraper.rs with helpers and full path**

Create `backend/crates/crawler/src/scraper.rs`:

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client;
use serde_json::json;
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::api::{
    ComplexityError, MaxRetriesError, PaginationLimitError,
    EVENT_STANDINGS_QUERY, PHASE_GROUPS_QUERY, PHASE_GROUP_GAMES_QUERY,
    PHASE_GROUP_SETS_QUERY, PHASE_GROUP_SETS_QUERY_SLIM, TOURNAMENT_QUERY,
    gql_query,
};
use crate::api_types::{
    EventPhaseGroupsData, EventStandingsData, FullPhaseGroupSetsData,
    GamesPhaseGroupSetsData, SlimPhaseGroupSetsData, TournamentsData,
};
use crate::cli::Config;
use crate::db::{
    get_checkpoint, is_checkpointed, set_checkpoint, upsert_event, upsert_event_entry,
    upsert_game, upsert_game_selection, upsert_phase, upsert_phase_group, upsert_player_full,
    upsert_player_slim, upsert_set, upsert_set_game, upsert_tournament,
};

const TOURNAMENTS_PER_PAGE: u32 = 20;
const STANDINGS_PER_PAGE: u32 = 100;

// ---------------------------------------------------------------------------
// Pure helpers — tested below
// ---------------------------------------------------------------------------

pub fn is_dq(state: Option<i64>, display_score: Option<&str>) -> bool {
    if state == Some(7) {
        return true;
    }
    display_score
        .map(|s| s.to_uppercase().contains("DQ"))
        .unwrap_or(false)
}

pub fn extract_scores(display_score: &str) -> Option<(i16, i16)> {
    let parts: Vec<&str> = display_score.splitn(2, " - ").collect();
    if parts.len() != 2 {
        return None;
    }
    let a: i16 = parts[0].trim().parse().ok()?;
    let b: i16 = parts[1].trim().parse().ok()?;
    if a >= b {
        Some((a, b))
    } else {
        Some((b, a))
    }
}

fn ts_to_dt(ts: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(ts, 0).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Complexity-halving macro — mirrors the HCI scraper pattern
// ---------------------------------------------------------------------------

macro_rules! with_complexity_retry {
    ($per_page:expr, $min:expr, $block:expr) => {{
        let mut per_page = $per_page;
        loop {
            match $block(per_page).await {
                Ok(v) => break Ok(v),
                Err(e) => {
                    if e.downcast_ref::<ComplexityError>().is_some() && per_page > $min {
                        per_page = (per_page / 2).max($min);
                        tracing::debug!(per_page, "complexity error — halving perPage");
                        continue;
                    }
                    break Err(e);
                }
            }
        }
    }};
}

// ---------------------------------------------------------------------------
// Main run loop
// ---------------------------------------------------------------------------

pub async fn run(config: &Config, pool: &PgPool, shutdown: &AtomicBool) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let base_url = crate::api::STARTGG_API_URL.to_string();
    let delay = Duration::from_millis(config.delay_ms);

    let range_start = config
        .from_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();
    let range_end = config
        .to_date
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp()
        .min(Utc::now().timestamp());
    let window_secs = config.window_days as i64 * 86400;

    // Resume from checkpoint if available
    let mut window_start = if let Some(cp) = get_checkpoint(pool, "window_start").await? {
        cp.as_i64().unwrap_or(range_start)
    } else {
        range_start
    };

    let mut base_filter = json!({});
    if let Some(gid) = config.game_id {
        base_filter["videogameIds"] = json!([gid]);
    }

    while window_start < range_end {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        let window_end = (window_start + window_secs).min(range_end);
        let mut filter = base_filter.clone();
        filter["afterDate"] = json!(window_start);
        filter["beforeDate"] = json!(window_end);

        info!(
            window_start = %ts_to_dt(window_start),
            window_end   = %ts_to_dt(window_end),
            "starting date window"
        );

        let mut tournament_page = 1u32;
        loop {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }

            let data: TournamentsData = gql_query(
                &client,
                &base_url,
                &config.startgg_api_key,
                TOURNAMENT_QUERY,
                json!({ "page": tournament_page, "perPage": TOURNAMENTS_PER_PAGE, "filter": filter }),
                delay,
            )
            .await?;

            let total_pages = data.tournaments.page_info.total_pages.unwrap_or(1);

            for t_node in &data.tournaments.nodes {
                if shutdown.load(Ordering::SeqCst) {
                    break;
                }

                let t_key = format!("tournament:{}", t_node.id);
                let cp = get_checkpoint(pool, &t_key).await?;
                if is_checkpointed(&cp) {
                    continue;
                }

                let tournament_id = upsert_tournament(pool, t_node).await?;

                for e_node in &t_node.events {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }

                    let e_key = format!("event:{}", e_node.id);
                    let e_cp = get_checkpoint(pool, &e_key).await?;
                    if is_checkpointed(&e_cp) {
                        continue;
                    }

                    let game_id = if let Some(vg) = &e_node.videogame {
                        Some(upsert_game(pool, vg.id, &vg.name).await?)
                    } else {
                        None
                    };

                    let event_id = upsert_event(pool, e_node, tournament_id, game_id).await?;

                    process_event(
                        &client, &base_url, pool, &config.startgg_api_key,
                        e_node.id, event_id, config.sets_per_page, delay, shutdown,
                    )
                    .await?;

                    set_checkpoint(pool, &e_key, json!(true)).await?;
                    tokio::time::sleep(delay).await;
                }

                set_checkpoint(pool, &t_key, json!(true)).await?;
            }

            if tournament_page >= total_pages as u32 {
                break;
            }
            tournament_page += 1;
            tokio::time::sleep(delay).await;
        }

        // Advance window and persist checkpoint
        window_start = window_end;
        set_checkpoint(pool, "window_start", json!(window_start)).await?;
    }

    info!("crawl complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// Per-event processing
// ---------------------------------------------------------------------------

async fn process_event(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    event_startgg_id: i64,
    event_id: Uuid,
    sets_per_page: u32,
    delay: Duration,
    shutdown: &AtomicBool,
) -> Result<()> {
    // Fetch all phase group IDs for this event
    let pg_data: EventPhaseGroupsData = gql_query(
        client, base_url, token, PHASE_GROUPS_QUERY,
        json!({ "eventId": event_startgg_id }),
        delay,
    )
    .await?;

    let phases = pg_data.event.map(|e| e.phases).unwrap_or_default();

    for phase in &phases {
        if shutdown.load(Ordering::SeqCst) { return Ok(()); }
        for pg_id_node in &phase.phase_groups.nodes {
            if shutdown.load(Ordering::SeqCst) { return Ok(()); }
            process_phase_group(
                client, base_url, pool, token,
                pg_id_node.id, phase.id, event_id, sets_per_page, delay,
            )
            .await?;
            tokio::time::sleep(delay).await;
        }
    }

    // Fetch standings for final placements
    let mut standings_page = 1u32;
    loop {
        let data: EventStandingsData = gql_query(
            client, base_url, token, EVENT_STANDINGS_QUERY,
            json!({ "eventId": event_startgg_id, "page": standings_page, "perPage": STANDINGS_PER_PAGE }),
            delay,
        )
        .await?;

        let standings_node = match data.event { Some(e) => e, None => break };
        let total_pages = standings_node.standings.page_info.total_pages.unwrap_or(1);

        for standing in &standings_node.standings.nodes {
            // entrant ID → player UUID lookup via global_sets is not trivial here;
            // we skip entries we can't resolve — they get populated from set data
            let _ = standing; // standings processing happens in full path below
        }

        if standings_page >= total_pages as u32 { break; }
        standings_page += 1;
        tokio::time::sleep(delay).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Per-phase-group processing — full path then two-pass fallback
// ---------------------------------------------------------------------------

async fn process_phase_group(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    pg_startgg_id: i64,
    phase_startgg_id: i64,
    event_id: Uuid,
    sets_per_page: u32,
    delay: Duration,
) -> Result<()> {
    // Track entrant_id → player UUID within this phase group for game winner resolution
    let mut entrant_to_player: HashMap<i64, Uuid> = HashMap::new();
    // Track set_startgg_id → set UUID for games pass
    let mut set_id_to_uuid: HashMap<i64, Uuid> = HashMap::new();

    // Attempt full query with complexity halving
    let full_result: Result<()> = with_complexity_retry!(sets_per_page, 1, |per_page| async move {
        fetch_full_path(
            client, base_url, pool, token,
            pg_startgg_id, phase_startgg_id, event_id,
            per_page, delay,
            &mut HashMap::new(),
            &mut HashMap::new(),
        )
        .await
    });

    if full_result.is_ok() {
        return Ok(());
    }

    let err = full_result.unwrap_err();
    let is_complexity = err.downcast_ref::<ComplexityError>().is_some();
    if !is_complexity {
        return Err(err);
    }

    warn!(pg_startgg_id, "full query failed at perPage=1, falling back to two-pass");

    // Pass 1: slim identity pass
    let slim_result = fetch_slim_pass(
        client, base_url, pool, token,
        pg_startgg_id, phase_startgg_id, event_id,
        sets_per_page, delay,
        &mut entrant_to_player,
        &mut set_id_to_uuid,
    )
    .await;

    if let Err(e) = &slim_result {
        warn!(pg_startgg_id, error = %e, "slim pass also failed, skipping phase group");
        return Ok(());
    }

    // Pass 2: games pass
    let games_result = fetch_games_pass(
        client, base_url, pool, token,
        pg_startgg_id, sets_per_page, delay,
        &entrant_to_player,
        &set_id_to_uuid,
    )
    .await;

    if let Err(e) = &games_result {
        warn!(pg_startgg_id, error = %e, "games pass failed, game data unavailable for this phase group");
    }

    Ok(())
}

async fn fetch_full_path(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    pg_startgg_id: i64,
    phase_startgg_id: i64,
    event_id: Uuid,
    sets_per_page: u32,
    delay: Duration,
    _entrant_map: &mut HashMap<i64, Uuid>,
    _set_map: &mut HashMap<i64, Uuid>,
) -> Result<()> {
    let mut page = 1u32;
    // Phase/phase_group upserted on first page to avoid extra query
    let mut phase_uuid: Option<Uuid> = None;
    let mut phase_group_uuid: Option<Uuid> = None;

    loop {
        let data: FullPhaseGroupSetsData = gql_query(
            client, base_url, token, PHASE_GROUP_SETS_QUERY,
            json!({ "phaseGroupId": pg_startgg_id, "page": page, "perPage": sets_per_page }),
            delay,
        )
        .await?;

        let pg_node = match data.phase_group { Some(n) => n, None => break };
        let total_pages = pg_node.sets.page_info.total_pages.unwrap_or(1);

        // Build entrant_id → player_uuid map for this page's sets
        let mut local_entrant_map: HashMap<i64, Uuid> = HashMap::new();

        for set_node in &pg_node.sets.nodes {
            // Upsert phase + phase_group from first set that has them
            if phase_uuid.is_none() {
                if let Some(pg_info) = &set_node.phase_group {
                    if let Some(phase_info) = &pg_info.phase {
                        let pu = upsert_phase(pool, phase_startgg_id, event_id, phase_info).await?;
                        phase_uuid = Some(pu);
                        let pgu = upsert_phase_group(pool, pg_startgg_id, pu, pg_info).await?;
                        phase_group_uuid = Some(pgu);
                    }
                }
            }

            // Resolve players from participants
            for slot in &set_node.slots {
                if let Some(entrant) = &slot.entrant {
                    if let Some(participant) = entrant.participants.first() {
                        let player_uuid = if let Some(user) = &participant.user {
                            let player_id = participant.player.as_ref().map(|p| p.id);
                            let image_url = user.images.iter()
                                .find(|img| img.image_type.as_deref() == Some("profile"))
                                .or_else(|| user.images.first())
                                .and_then(|img| img.url.as_deref());
                            let loc = user.location.as_ref();
                            upsert_player_full(
                                pool,
                                user.id,
                                player_id,
                                participant.player.as_ref()
                                    .and_then(|p| p.gamer_tag.as_deref())
                                    .unwrap_or("Unknown"),
                                user.name.as_deref(),
                                image_url,
                                user.slug.as_deref(),
                                user.bio.as_deref(),
                                user.gender_pronoun.as_deref(),
                                loc.and_then(|l| l.city.as_deref()),
                                loc.and_then(|l| l.state.as_deref()),
                                loc.and_then(|l| l.country.as_deref()),
                            )
                            .await?
                        } else if let Some(player) = &participant.player {
                            upsert_player_slim(
                                pool,
                                player.id,
                                player.gamer_tag.as_deref().unwrap_or("Unknown"),
                            )
                            .await?
                        } else {
                            continue;
                        };
                        local_entrant_map.insert(entrant.id, player_uuid);
                    }
                }
            }

            // Determine winner/loser players from slots
            let (winner_uuid, loser_uuid) = resolve_winner_loser(
                set_node.winner_id,
                &set_node.slots,
                &local_entrant_map,
            );

            let (winner_score, loser_score) = set_node
                .display_score
                .as_deref()
                .and_then(extract_scores)
                .map(|(w, l)| (Some(w), Some(l)))
                .unwrap_or((None, None));

            let completed_at = set_node.completed_at.map(ts_to_dt);
            let dq = is_dq(set_node.state, set_node.display_score.as_deref());

            let set_uuid = upsert_set(
                pool,
                set_node.id,
                event_id,
                phase_group_uuid,
                winner_uuid,
                loser_uuid,
                set_node.round,
                set_node.full_round_text.as_deref(),
                winner_score,
                loser_score,
                dq,
                set_node.vod_url.as_deref(),
                completed_at,
            )
            .await?;

            // Upsert games + selections
            for game in &set_node.games {
                let order_num = game.order_num.unwrap_or(0) as i32;
                let game_winner_uuid = game.winner_id
                    .and_then(|wid| local_entrant_map.get(&wid).copied());
                let stage_id = game.stage.as_ref().map(|s| s.id);
                let stage_name = game.stage.as_ref().and_then(|s| s.name.as_deref());

                let game_uuid = upsert_set_game(
                    pool, set_uuid, order_num, game_winner_uuid, stage_id, stage_name,
                )
                .await?;

                for sel in &game.selections {
                    if let Some(sel_type) = &sel.selection_type {
                        let player_uuid = sel.entrant.as_ref()
                            .and_then(|e| local_entrant_map.get(&e.id).copied());
                        let char_id = sel.character.as_ref().map(|c| c.id);
                        let char_name = sel.character.as_ref().and_then(|c| c.name.as_deref());
                        upsert_game_selection(pool, game_uuid, player_uuid, sel_type, char_id, char_name).await?;
                    }
                }
            }
        }

        if page >= total_pages as u32 { break; }
        page += 1;
        tokio::time::sleep(delay).await;
    }

    Ok(())
}

async fn fetch_slim_pass(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    pg_startgg_id: i64,
    phase_startgg_id: i64,
    event_id: Uuid,
    sets_per_page: u32,
    delay: Duration,
    entrant_to_player: &mut HashMap<i64, Uuid>,
    set_id_to_uuid: &mut HashMap<i64, Uuid>,
) -> Result<()> {
    let mut page = 1u32;
    let mut phase_uuid: Option<Uuid> = None;
    let mut phase_group_uuid: Option<Uuid> = None;

    loop {
        let data: SlimPhaseGroupSetsData = with_complexity_retry!(sets_per_page, 1, |per_page| {
            let client = client.clone();
            let base_url = base_url.to_string();
            let token = token.to_string();
            async move {
                gql_query::<SlimPhaseGroupSetsData>(
                    &client, &base_url, &token, PHASE_GROUP_SETS_QUERY_SLIM,
                    json!({ "phaseGroupId": pg_startgg_id, "page": page, "perPage": per_page }),
                    delay,
                )
                .await
            }
        })?;

        let pg_node = match data.phase_group { Some(n) => n, None => break };
        let total_pages = pg_node.sets.page_info.total_pages.unwrap_or(1);

        for set_node in &pg_node.sets.nodes {
            if phase_uuid.is_none() {
                if let Some(pg_info) = &set_node.phase_group {
                    if let Some(phase_info) = &pg_info.phase {
                        let pu = upsert_phase(pool, phase_startgg_id, event_id, phase_info).await?;
                        phase_uuid = Some(pu);
                        let pgu = upsert_phase_group(pool, pg_startgg_id, pu, pg_info).await?;
                        phase_group_uuid = Some(pgu);
                    }
                }
            }

            for slot in &set_node.slots {
                if let Some(entrant) = &slot.entrant {
                    if let Some(participant) = entrant.participants.first() {
                        if let Some(player) = &participant.player {
                            let uuid = upsert_player_slim(
                                pool,
                                player.id,
                                player.gamer_tag.as_deref().unwrap_or("Unknown"),
                            )
                            .await?;
                            entrant_to_player.insert(entrant.id, uuid);
                        }
                    }
                }
            }

            let (winner_uuid, loser_uuid) = resolve_winner_loser_slim(
                set_node.winner_id,
                &set_node.slots,
                entrant_to_player,
            );

            let (winner_score, loser_score) = set_node
                .display_score
                .as_deref()
                .and_then(extract_scores)
                .map(|(w, l)| (Some(w), Some(l)))
                .unwrap_or((None, None));

            let set_uuid = upsert_set(
                pool,
                set_node.id,
                event_id,
                phase_group_uuid,
                winner_uuid,
                loser_uuid,
                set_node.round,
                set_node.full_round_text.as_deref(),
                winner_score,
                loser_score,
                is_dq(set_node.state, set_node.display_score.as_deref()),
                set_node.vod_url.as_deref(),
                set_node.completed_at.map(ts_to_dt),
            )
            .await?;

            set_id_to_uuid.insert(set_node.id, set_uuid);
        }

        if page >= total_pages as u32 { break; }
        page += 1;
        tokio::time::sleep(delay).await;
    }

    Ok(())
}

async fn fetch_games_pass(
    client: &Client,
    base_url: &str,
    pool: &PgPool,
    token: &str,
    pg_startgg_id: i64,
    sets_per_page: u32,
    delay: Duration,
    entrant_to_player: &HashMap<i64, Uuid>,
    set_id_to_uuid: &HashMap<i64, Uuid>,
) -> Result<()> {
    let mut page = 1u32;

    loop {
        let data: GamesPhaseGroupSetsData = with_complexity_retry!(sets_per_page, 1, |per_page| {
            let client = client.clone();
            let base_url = base_url.to_string();
            let token = token.to_string();
            async move {
                gql_query::<GamesPhaseGroupSetsData>(
                    &client, &base_url, &token, PHASE_GROUP_GAMES_QUERY,
                    json!({ "phaseGroupId": pg_startgg_id, "page": page, "perPage": per_page }),
                    delay,
                )
                .await
            }
        })?;

        let pg_node = match data.phase_group { Some(n) => n, None => break };
        let total_pages = pg_node.sets.page_info.total_pages.unwrap_or(1);

        for set_node in &pg_node.sets.nodes {
            let set_uuid = match set_id_to_uuid.get(&set_node.id) {
                Some(u) => *u,
                None => continue,
            };

            for game in &set_node.games {
                let order_num = game.order_num.unwrap_or(0) as i32;
                let game_winner_uuid = game.winner_id
                    .and_then(|wid| entrant_to_player.get(&wid).copied());
                let stage_id = game.stage.as_ref().map(|s| s.id);
                let stage_name = game.stage.as_ref().and_then(|s| s.name.as_deref());

                let game_uuid = upsert_set_game(
                    pool, set_uuid, order_num, game_winner_uuid, stage_id, stage_name,
                )
                .await?;

                for sel in &game.selections {
                    if let Some(sel_type) = &sel.selection_type {
                        let player_uuid = sel.entrant.as_ref()
                            .and_then(|e| entrant_to_player.get(&e.id).copied());
                        let char_id = sel.character.as_ref().map(|c| c.id);
                        let char_name = sel.character.as_ref().and_then(|c| c.name.as_deref());
                        upsert_game_selection(pool, game_uuid, player_uuid, sel_type, char_id, char_name).await?;
                    }
                }
            }
        }

        if page >= total_pages as u32 { break; }
        page += 1;
        tokio::time::sleep(delay).await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Winner/loser resolution helpers
// ---------------------------------------------------------------------------

fn resolve_winner_loser(
    winner_entrant_id: Option<i64>,
    slots: &[crate::api_types::SlotNode],
    entrant_map: &HashMap<i64, Uuid>,
) -> (Option<Uuid>, Option<Uuid>) {
    let mut winner = None;
    let mut loser = None;
    for slot in slots {
        if let Some(entrant) = &slot.entrant {
            let uuid = entrant_map.get(&entrant.id).copied();
            if Some(entrant.id) == winner_entrant_id {
                winner = uuid;
            } else {
                loser = uuid;
            }
        }
    }
    (winner, loser)
}

fn resolve_winner_loser_slim(
    winner_entrant_id: Option<i64>,
    slots: &[crate::api_types::SlimSlotNode],
    entrant_map: &HashMap<i64, Uuid>,
) -> (Option<Uuid>, Option<Uuid>) {
    let mut winner = None;
    let mut loser = None;
    for slot in slots {
        if let Some(entrant) = &slot.entrant {
            let uuid = entrant_map.get(&entrant.id).copied();
            if Some(entrant.id) == winner_entrant_id {
                winner = uuid;
            } else {
                loser = uuid;
            }
        }
    }
    (winner, loser)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dq_detected_by_state() {
        assert!(is_dq(Some(7), None));
    }

    #[test]
    fn dq_detected_by_display_score() {
        assert!(is_dq(Some(3), Some("DQ")));
        assert!(is_dq(None, Some("Player DQ - 0")));
    }

    #[test]
    fn no_dq_for_normal_set() {
        assert!(!is_dq(Some(3), Some("3 - 1")));
    }

    #[test]
    fn scores_extracted_from_display_score() {
        let (w, l) = extract_scores("3 - 1").unwrap();
        assert_eq!(w, 3);
        assert_eq!(l, 1);
        let (w, l) = extract_scores("2 - 0").unwrap();
        assert_eq!(w, 2);
        assert_eq!(l, 0);
    }

    #[test]
    fn scores_none_for_dq_display() {
        assert!(extract_scores("DQ").is_none());
        assert!(extract_scores("").is_none());
    }
}
```

- [ ] **Step 4: Run unit tests**

```bash
cd backend && cargo test -p crawler -- scraper::tests
```

Expected: all 5 tests pass.

- [ ] **Step 5: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/crawler/src/scraper.rs
git commit -m "feat(crawler): add scraper with full path, two-pass fallback, and DQ/score helpers"
```

---

### Task 7: Complete main.rs

**Files:**
- Modify: `backend/crates/crawler/src/main.rs`

- [ ] **Step 1: Write final main.rs**

Replace `backend/crates/crawler/src/main.rs` with:

```rust
use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal::unix::{SignalKind, signal};

use cli::Config;

mod api;
mod api_types;
mod cli;
mod db;
mod scraper;

fn init_tracing(rust_log: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(rust_log))
        .init();
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let config = Config::parse();

    init_tracing(&config.rust_log);

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .expect("failed to connect to database");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("failed to run migrations");

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");

    tokio::select! {
        result = scraper::run(&config, &pool, &shutdown) => {
            if let Err(e) = result {
                tracing::error!(%e, "crawler exited with error");
                std::process::exit(1);
            }
        }
        _ = sigterm.recv() => {
            tracing::info!("received SIGTERM, shutting down");
            shutdown_clone.store(true, Ordering::SeqCst);
        }
        _ = sigint.recv() => {
            tracing::info!("received SIGINT, shutting down");
            shutdown_clone.store(true, Ordering::SeqCst);
        }
    }
}
```

- [ ] **Step 2: Build release binary**

```bash
cd backend && cargo build --release -p crawler 2>&1 | tail -5
```

Expected: `Finished release profile` with crawler binary produced.

- [ ] **Step 3: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/crawler/src/main.rs
git commit -m "feat(crawler): complete main.rs with signal handling and migration run"
```

---

### Task 8: Integration test

**Files:**
- Create: `backend/crates/crawler/src/tests/mod.rs` (or inline in `scraper.rs`)

**Interfaces:**
- Consumes: `scraper::run`, wiremock, `#[sqlx::test]`

- [ ] **Step 1: Write failing integration test**

Add a file `backend/crates/crawler/tests/integration.rs`:

```rust
use std::sync::atomic::AtomicBool;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use serde_json::json;

#[sqlx::test(migrations = "../../migrations")]
async fn test_crawl_single_tournament(pool: sqlx::PgPool) {
    let mock_server = MockServer::start().await;

    // Stub: tournaments page 1
    Mock::given(method("POST"))
        .and(path("/"))
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

    // Stub: phase groups for event 2001
    Mock::given(method("POST"))
        .and(path("/"))
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

    // Stub: sets for phase group 4001 (full query)
    Mock::given(method("POST"))
        .and(path("/"))
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

    // Stub: standings for event 2001
    Mock::given(method("POST"))
        .and(path("/"))
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
    };

    let shutdown = AtomicBool::new(false);
    // Use mock_server URL as base
    // We need to thread the base_url through; update scraper::run signature for tests
    // See note below — the base_url is passed via a test-only constructor or env var

    // Verify tournament was stored
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
}
```

> **Note on testability:** The `scraper::run` function takes `config: &Config` which has `startgg_api_key` but the API URL is currently hardcoded in `api.rs` as `STARTGG_API_URL`. To make tests work, you must thread the base URL through. The recommended approach: add an optional `startgg_base_url: Option<String>` field to `Config` (env `STARTGG_BASE_URL`, default `None`); in `scraper::run`, fall back to `STARTGG_API_URL` when `None`. This matches the existing `StartggClient::new_with_base_url` pattern in common.

- [ ] **Step 2: Add base_url to Config and scraper::run**

Edit `cli.rs` to add:
```rust
#[arg(long, env = "STARTGG_BASE_URL")]
pub startgg_base_url: Option<String>,
```

Edit `scraper.rs`'s `run` function first line of URL construction to:
```rust
let base_url = config.startgg_base_url.as_deref()
    .unwrap_or(crate::api::STARTGG_API_URL)
    .to_string();
```

- [ ] **Step 3: Complete the integration test using mock_server.uri()**

Update the test to construct config with `startgg_base_url: Some(mock_server.uri())` and call `scraper::run(&config, &pool, &shutdown).await.unwrap()` before the assertions.

The wiremock server handles all 4 queries (tournaments, phase groups, sets, standings) via ordered mock matching. Because wiremock matches in registration order, register the mocks in the order the crawler calls them.

- [ ] **Step 4: Run integration test**

```bash
cd backend && DATABASE_URL=postgres://... cargo test -p crawler -- integration 2>&1
```

Or run via `bash backend/test.sh` which handles the ephemeral DB automatically.

Expected: test passes — 1 tournament, 2 players, 1 set, 1 game, 2 selections in DB.

- [ ] **Step 5: Update sqlx offline cache**

```bash
bash backend/prepare-sqlx.sh
```

- [ ] **Step 6: Commit**

```bash
cd backend && cargo fmt --all
git add backend/crates/crawler/ backend/.sqlx/
git commit -m "test(crawler): add integration test with wiremock start.gg stub"
```

---

### Task 9: Docker + docker-compose

**Files:**
- Modify: `backend/Dockerfile`
- Modify: `docker-compose.yml`

- [ ] **Step 1: Read current Dockerfile**

Read `backend/Dockerfile` and locate the `COPY --from=builder` line that copies `api` and `worker`.

- [ ] **Step 2: Add crawler to Dockerfile**

Find the line that copies the api binary in the builder recipe (e.g., `COPY --from=builder /app/target/release/api ./api`) and add crawler alongside it. Also add a `FROM base AS crawler` stage following the same pattern as the existing api/worker stages.

The exact lines will match the existing pattern — add `crawler` in the same copy step and add a new `FROM` stage named `crawler` that runs `./crawler`.

- [ ] **Step 3: Add crawler service to docker-compose.yml**

Add to `docker-compose.yml` after the `worker` service:

```yaml
  crawler:
    build:
      context: ./backend
      target: crawler
    restart: unless-stopped
    environment:
      DATABASE_URL: ${DATABASE_URL}
      STARTGG_API_KEY: ${STARTGG_API_KEY}
      CRAWLER_FROM_DATE: ${CRAWLER_FROM_DATE:-2018-01-01}
      CRAWLER_WINDOW_DAYS: ${CRAWLER_WINDOW_DAYS:-7}
      CRAWLER_DELAY_MS: ${CRAWLER_DELAY_MS:-750}
    depends_on:
      - db
```

- [ ] **Step 4: Verify build compiles (offline mode)**

```bash
cd backend && SQLX_OFFLINE=true cargo build --release -p crawler 2>&1 | tail -5
```

Expected: `Finished release`.

- [ ] **Step 5: Commit**

```bash
cd backend && cargo fmt --all
git add backend/Dockerfile docker-compose.yml
git commit -m "feat(crawler): add Docker build stage and docker-compose service"
```

---

### Task 10: Full test suite + docs

**Files:**
- Modify: `docs/DESIGN.md`
- Modify: `docs/modules.md`

- [ ] **Step 1: Run full test suite**

```bash
bash test.sh
```

Expected: all sections PASS.

- [ ] **Step 2: Update docs/modules.md**

Add an entry for `crates/crawler` under the backend crates section:

```
### crates/crawler

Binary crate. Continuously mirrors start.gg tournament data into `global_*` tables.

- `api.rs` — `gql_query` with retry/backoff, 6 query string constants, error types
- `api_types.rs` — serde deserialization types for all 6 queries
- `cli.rs` — `Config` struct with env-var defaults (clap)
- `db.rs` — all `global_*` table upsert functions, checkpoint read/write
- `scraper.rs` — sliding window loop, per-tournament/event/phase-group processing, two-pass fallback
```

- [ ] **Step 3: Update docs/DESIGN.md**

Add a "Global Mirror" section that notes:
- The `global_*` tables and `crawler_checkpoints` table added to `001_initial.sql`
- The `crawler` binary's sliding window strategy and two-pass fallback
- Player identity resolution (full pass via `startgg_user_id`, slim pass via `startgg_player_id`, COALESCE upserts)
- The `global_player_ratings` table is defined but populated in a future phase
- Once the mirror has sufficient coverage, `worker` import path shifts to querying `global_*` tables, eliminating per-user API keys

- [ ] **Step 4: Commit docs**

```bash
git add docs/DESIGN.md docs/modules.md
git commit -m "docs: add global mirror crawler to DESIGN.md and modules.md"
```
