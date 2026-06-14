# Ranking Algorithms (Elo & Glicko-2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add algorithmic rankings (Elo, Glicko-2) alongside manual rankings; pre-compute win/loss and H2H tables on ranking changes; replace per-event toggle API calls with a batch save.

**Architecture:** A new `algorithms` module in `common` defines a `RankingAlgorithm` trait implemented by `Elo` and `Glicko2`. A `compute_ranking` worker job (new `job_kind` variant) runs in two phases for every ranking: Phase 1 populates `ranking_set_results` (pre-filtered set list for fast stats/H2H queries); Phase 2, only for algorithmic rankings, populates `ranking_player_scores` via the algorithm trait. The frontend accumulates event inclusion changes locally and submits them in one bulk `PUT /events` request, triggering a single recompute.

**Tech Stack:** Rust (sqlx, serde_json, uuid, chrono, std::f64::consts::PI), Axum, PostgreSQL, SvelteKit/TypeScript.

---

## File Structure

**Create:**
- `backend/crates/common/src/algorithms/mod.rs` — trait, `ScoredSet`, `PlayerScore`, `AlgorithmError`, `AlgorithmRegistry`
- `backend/crates/common/src/algorithms/elo.rs` — Elo implementation
- `backend/crates/common/src/algorithms/glicko2.rs` — Glicko-2 implementation
- `backend/crates/worker/src/compute.rs` — `compute_ranking` job handler (Phase 1 + Phase 2)

**Modify:**
- `backend/migrations/001_initial.sql` — merged migration with all new schema (merge allowed: no prod DB)
- `backend/crates/common/src/lib.rs` — expose `algorithms` module
- `backend/crates/common/src/models/mod.rs` — extend `Ranking` struct; add `RankingPlayerScore`
- `backend/crates/common/src/jobs.rs` — add `ComputeRankingParams`, `enqueue_compute_ranking`
- `backend/crates/worker/src/main.rs` — dispatch `compute_ranking` jobs to `compute::run`
- `backend/crates/worker/src/import.rs` — enqueue `compute_ranking` for all rankings after import completes
- `backend/crates/api/src/routes/rankings.rs` — new fields in create/patch/list/get; `recompute` endpoint; enqueue on player add/remove; `get_computed_ranking` endpoint
- `backend/crates/api/src/routes/tournaments.rs` — replace `patch_event` with bulk `put_events`; update `get_stats`/`get_player_stats`/`get_head_to_head`/`get_h2h_sets` to read from `ranking_set_results`
- `web/src/routes/projects/[id]/rankings/[rid]/tournaments/+page.svelte` — batch save/discard UI
- `backend/openapi.yaml` — API contract updates
- `docs/DESIGN.md` — architecture updates
- `docs/routes.md` — access control table updates

---

## Task 1: Merge and Extend Migration

Merge `backend/migrations/001_initial.sql` into a single file that adds the new `compute_ranking` job kind, new columns on `rankings`, and the two new tables.

**Files:**
- Modify: `backend/migrations/001_initial.sql`

- [ ] **Step 1: Replace the migration file**

Replace `backend/migrations/001_initial.sql` with the complete merged content below. The only schema changes vs. the original are: (1) `compute_ranking` added to the `job_kind` enum, (2) four new columns on `rankings`, (3) two new tables.

```sql
-- Enums
CREATE TYPE job_kind AS ENUM ('import_tournaments', 'compute_ranking');
CREATE TYPE job_status AS ENUM ('pending', 'running', 'done', 'failed');

-- Users and sessions
CREATE TABLE users (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT        NOT NULL UNIQUE,
    display_name    TEXT        NOT NULL,
    password_hash   TEXT        NOT NULL,
    startgg_api_key TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sessions (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX sessions_user_id_idx ON sessions(user_id);

CREATE TYPE project_member_role AS ENUM ('editor', 'viewer');

-- Projects (container for rankings, players, members)
CREATE TABLE projects (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    game_id     BIGINT,
    game_name   TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX projects_owner_id_idx ON projects(owner_id);

CREATE TABLE project_members (
    project_id  UUID                NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id     UUID                NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL,
    joined_at   TIMESTAMPTZ         NOT NULL DEFAULT NOW(),
    PRIMARY KEY (project_id, user_id)
);

CREATE INDEX project_members_user_id_idx ON project_members(user_id);

CREATE TABLE project_invite_links (
    id          UUID                PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id  UUID                NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    role        project_member_role NOT NULL,
    created_by  UUID                NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at  TIMESTAMPTZ,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ         NOT NULL DEFAULT NOW()
);

CREATE INDEX project_invite_links_project_id_idx ON project_invite_links(project_id);

-- Rankings (one or more per project; each is an independent ranking view)
CREATE TABLE rankings (
    id                      UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id              UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name                    TEXT        NOT NULL,
    description             TEXT,
    published               BOOLEAN     NOT NULL DEFAULT FALSE,
    algorithm               TEXT,
    algorithm_config        JSONB       NOT NULL DEFAULT '{}',
    include_external_results BOOLEAN    NOT NULL DEFAULT FALSE,
    result_sort             TEXT        NOT NULL DEFAULT 'upset_factor',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX rankings_project_id_idx ON rankings(project_id);

-- Players (project-scoped pool; rankings select a subset via ranking_players)
CREATE TABLE players (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id    UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX players_project_id_idx ON players(project_id);

-- Per-ranking player membership with ordering and notes
CREATE TABLE ranking_players (
    ranking_id    UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    player_id     UUID    NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    rank_position INTEGER NOT NULL DEFAULT 0,
    notes         TEXT,
    PRIMARY KEY (ranking_id, player_id)
);

CREATE INDEX ranking_players_player_id_idx ON ranking_players(player_id);

-- start.gg accounts linked to players
CREATE TABLE startgg_accounts (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    player_id       UUID        NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    startgg_user_id BIGINT      NOT NULL,
    handle          TEXT        NOT NULL,
    display_name    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (player_id, startgg_user_id)
);

CREATE INDEX startgg_accounts_player_id_idx ON startgg_accounts(player_id);
CREATE INDEX startgg_accounts_user_id_idx   ON startgg_accounts(startgg_user_id);

-- Tournaments (global, shared across all projects)
CREATE TABLE tournaments (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id     BIGINT      NOT NULL UNIQUE,
    name           TEXT        NOT NULL,
    handle         TEXT        NOT NULL,
    city           TEXT,
    addr_state     TEXT,
    country_code   TEXT,
    venue_name     TEXT,
    venue_address  TEXT,
    timezone       TEXT,
    online         BOOLEAN     NOT NULL DEFAULT FALSE,
    num_attendees  INTEGER,
    start_at       TIMESTAMPTZ,
    end_at         TIMESTAMPTZ,
    lat            DOUBLE PRECISION,
    lng            DOUBLE PRECISION,
    state          INTEGER,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE events (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID        NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    startgg_id    BIGINT      NOT NULL UNIQUE,
    name          TEXT        NOT NULL,
    handle        TEXT        NOT NULL,
    state         TEXT,
    is_online     BOOLEAN,
    event_type    INTEGER,
    min_team_size INTEGER,
    max_team_size INTEGER,
    game_id       BIGINT,
    game_name     TEXT,
    num_entrants  INTEGER,
    start_at      TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX events_start_at_idx      ON events(start_at);
CREATE INDEX events_tournament_id_idx ON events(tournament_id);

CREATE TABLE phases (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT      NOT NULL UNIQUE,
    event_id      UUID        NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    name          TEXT,
    bracket_type  TEXT,
    phase_order   INTEGER,
    num_seeds     INTEGER,
    group_count   INTEGER,
    state         TEXT,
    is_exhibition BOOLEAN     NOT NULL DEFAULT FALSE
);

CREATE INDEX phases_event_id_idx ON phases(event_id);

CREATE TABLE phase_groups (
    id                 UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id         BIGINT      NOT NULL UNIQUE,
    phase_id           UUID        NOT NULL REFERENCES phases(id) ON DELETE CASCADE,
    display_identifier TEXT,
    bracket_type       TEXT,
    bracket_url        TEXT,
    num_rounds         INTEGER,
    start_at           TIMESTAMPTZ,
    first_round_time   TIMESTAMPTZ,
    state              INTEGER
);

CREATE INDEX phase_groups_phase_id_idx ON phase_groups(phase_id);

-- Per-ranking event inclusion (imported by worker per ranking; default included)
CREATE TABLE ranking_events (
    ranking_id UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    event_id   UUID    NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    included   BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (ranking_id, event_id)
);

CREATE INDEX ranking_events_event_id_idx ON ranking_events(event_id);

CREATE TABLE entrants (
    id                  UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id            UUID    NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    player_id           UUID    REFERENCES players(id) ON DELETE SET NULL,
    startgg_entrant_id  BIGINT  NOT NULL,
    startgg_user_id     BIGINT,
    seed                INTEGER,
    display_name        TEXT    NOT NULL,
    is_disqualified     BOOLEAN NOT NULL DEFAULT FALSE,
    final_placement     INTEGER,
    UNIQUE (event_id, startgg_entrant_id)
);

CREATE INDEX entrants_event_id_idx          ON entrants(event_id);
CREATE INDEX entrants_player_id_idx         ON entrants(player_id);
CREATE INDEX entrants_startgg_user_id_idx   ON entrants(startgg_user_id);

CREATE TABLE sets (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id          UUID        NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    phase_group_id    UUID        REFERENCES phase_groups(id),
    startgg_set_id    BIGINT      NOT NULL,
    winner_entrant_id UUID        NOT NULL REFERENCES entrants(id),
    loser_entrant_id  UUID        NOT NULL REFERENCES entrants(id),
    round             INTEGER,
    round_name        TEXT,
    total_games       SMALLINT,
    winner_score      SMALLINT,
    loser_score       SMALLINT,
    is_dq             BOOLEAN     NOT NULL DEFAULT FALSE,
    has_placeholder   BOOLEAN     NOT NULL DEFAULT FALSE,
    state             INTEGER,
    identifier        TEXT,
    vod_url           TEXT,
    completed_at      TIMESTAMPTZ,
    UNIQUE (event_id, startgg_set_id)
);

CREATE INDEX sets_event_id_idx           ON sets(event_id);
CREATE INDEX sets_phase_group_id_idx     ON sets(phase_group_id);
CREATE INDEX sets_winner_entrant_id_idx  ON sets(winner_entrant_id);
CREATE INDEX sets_loser_entrant_id_idx   ON sets(loser_entrant_id);
CREATE INDEX sets_completed_at_idx       ON sets(completed_at);

-- Pre-computed per-ranking set list (populated by compute_ranking job)
-- Contains only sets where both players are ranking members and the event is included.
-- The stats and H2H endpoints read from this table instead of joining the full set graph.
CREATE TABLE ranking_set_results (
    ranking_id       UUID        NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    set_id           UUID        NOT NULL REFERENCES sets(id)     ON DELETE CASCADE,
    winner_player_id UUID        NOT NULL REFERENCES players(id),
    loser_player_id  UUID        NOT NULL REFERENCES players(id),
    event_id         UUID        NOT NULL REFERENCES events(id),
    upset_factor     FLOAT,
    completed_at     TIMESTAMPTZ,
    PRIMARY KEY (ranking_id, set_id)
);

CREATE INDEX ranking_set_results_winner_idx ON ranking_set_results(ranking_id, winner_player_id);
CREATE INDEX ranking_set_results_loser_idx  ON ranking_set_results(ranking_id, loser_player_id);

-- Per-player algorithm scores (only for algorithmic rankings)
CREATE TABLE ranking_player_scores (
    ranking_id      UUID        NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    player_id       UUID        NOT NULL REFERENCES players(id)  ON DELETE CASCADE,
    computed_rating FLOAT       NOT NULL,
    display_data    JSONB       NOT NULL DEFAULT '{}',
    algorithm_state JSONB       NOT NULL DEFAULT '{}',
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (ranking_id, player_id)
);

CREATE INDEX ranking_player_scores_player_id_idx ON ranking_player_scores(player_id);

-- Job queue
CREATE TABLE jobs (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    kind       job_kind    NOT NULL,
    project_id UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    params     JSONB       NOT NULL DEFAULT '{}',
    result     JSONB,
    progress   JSONB,
    status     job_status  NOT NULL DEFAULT 'pending',
    error      TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX jobs_status_idx     ON jobs(status) WHERE status IN ('pending', 'running');
CREATE INDEX jobs_project_id_idx ON jobs(project_id);
```

- [ ] **Step 2: Verify migration parses (no DB needed)**

```bash
cd backend && grep -c "CREATE TABLE" migrations/001_initial.sql
```
Expected: `15` (13 original + `ranking_set_results` + `ranking_player_scores`)

- [ ] **Step 3: Commit**

```bash
git add backend/migrations/001_initial.sql
git commit -m "feat: merge migration + add ranking_set_results, ranking_player_scores, compute_ranking job kind"
```

---

## Task 2: Extend Ranking Model and Add RankingPlayerScore

Update the `Ranking` struct to reflect the new columns. Add `RankingPlayerScore`. Update every sqlx query that selects from `rankings` to include the new columns.

**Files:**
- Modify: `backend/crates/common/src/models/mod.rs`

- [ ] **Step 1: Write the failing test**

In `backend/crates/common/src/jobs.rs`, add at the bottom of the `#[cfg(test)]` block:

```rust
    #[sqlx::test(migrations = "../../migrations")]
    async fn ranking_has_algorithm_fields(pool: PgPool) {
        let project_id = setup_project(&pool).await;
        let ranking_id: Uuid = sqlx::query_scalar!(
            "INSERT INTO rankings (project_id, name, algorithm, algorithm_config, result_sort)
             VALUES ($1, 'Elo Test', 'elo', '{\"k_factor\": 32}', 'upset_factor')
             RETURNING id",
            project_id,
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let row = sqlx::query!(
            r#"SELECT algorithm, result_sort FROM rankings WHERE id = $1"#,
            ranking_id,
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.algorithm.as_deref(), Some("elo"));
        assert_eq!(row.result_sort, "upset_factor");
    }
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd backend && cargo test -p common -- ranking_has_algorithm_fields 2>&1 | tail -5
```
Expected: compile error because the new columns aren't in `001_initial.sql` .sqlx cache yet. (The migration file is updated, but `.sqlx/` is stale — that's fine for now; we'll regenerate in Task 12.)

Actually it may compile in offline mode. The test itself should PASS because the migration creates the columns and `sqlx::test` runs the migration. Run and note the result.

- [ ] **Step 3: Update `Ranking` struct and add `RankingPlayerScore`**

In `backend/crates/common/src/models/mod.rs`, replace the `Ranking` struct and add `RankingPlayerScore`:

```rust
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Ranking {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub published: bool,
    pub algorithm: Option<String>,
    pub algorithm_config: serde_json::Value,
    pub include_external_results: bool,
    pub result_sort: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct RankingPlayerScore {
    pub ranking_id: Uuid,
    pub player_id: Uuid,
    pub computed_rating: f64,
    pub display_data: serde_json::Value,
    pub algorithm_state: serde_json::Value,
    pub computed_at: DateTime<Utc>,
}
```

- [ ] **Step 4: Verify compilation**

```bash
cd backend && cargo check -p common 2>&1 | head -20
```
Expected: errors in `api` crate (queries that select from rankings with the old column list). That's correct — those are fixed in Task 8.

- [ ] **Step 5: Commit**

```bash
git add backend/crates/common/src/models/mod.rs
git commit -m "feat(common): extend Ranking struct with algorithm fields; add RankingPlayerScore"
```

---

## Task 3: Algorithm Module in Common

Create the `algorithms` module with the trait, `AlgorithmRegistry`, and both implementations.

**Files:**
- Create: `backend/crates/common/src/algorithms/mod.rs`
- Create: `backend/crates/common/src/algorithms/elo.rs`
- Create: `backend/crates/common/src/algorithms/glicko2.rs`
- Modify: `backend/crates/common/src/lib.rs`

- [ ] **Step 1: Write failing unit tests for Elo**

Create `backend/crates/common/src/algorithms/elo.rs` (test-first, no impl yet):

```rust
use serde_json::json;
use uuid::Uuid;

use super::{AlgorithmError, RankingAlgorithm, ScoredSet};

pub struct Elo;

impl RankingAlgorithm for Elo {
    fn name(&self) -> &'static str {
        "elo"
    }
    fn compute(
        &self,
        _config: &serde_json::Value,
        _sets: &[ScoredSet],
    ) -> Result<Vec<super::PlayerScore>, AlgorithmError> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn set(winner: Uuid, loser: Uuid) -> ScoredSet {
        ScoredSet {
            winner_id: winner,
            loser_id: loser,
            completed_at: Utc::now(),
            winner_global_rating: None,
            loser_global_rating: None,
            is_external_winner: false,
            is_external_loser: false,
        }
    }

    #[test]
    fn elo_winner_gains_loser_loses() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let config = json!({"k_factor": 32.0});
        let scores = Elo
            .compute(&config, &[set(a, b)])
            .unwrap();

        let a_score = scores.iter().find(|s| s.player_id == a).unwrap();
        let b_score = scores.iter().find(|s| s.player_id == b).unwrap();

        assert!(a_score.computed_rating > 1500.0);
        assert!(b_score.computed_rating < 1500.0);
        // Rating change is symmetric when starting equal
        let delta = a_score.computed_rating - 1500.0;
        let loss = 1500.0 - b_score.computed_rating;
        assert!((delta - loss).abs() < 0.001);
    }

    #[test]
    fn elo_no_sets_returns_empty() {
        let scores = Elo.compute(&json!({}), &[]).unwrap();
        assert!(scores.is_empty());
    }

    #[test]
    fn elo_display_data_has_rating_key() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let scores = Elo.compute(&json!({}), &[set(a, b)]).unwrap();
        let a_score = scores.iter().find(|s| s.player_id == a).unwrap();
        assert!(a_score.display_data["rating"].is_number());
    }
}
```

- [ ] **Step 2: Write failing unit tests for Glicko-2**

Create `backend/crates/common/src/algorithms/glicko2.rs`:

```rust
use serde_json::json;
use uuid::Uuid;

use super::{AlgorithmError, RankingAlgorithm, ScoredSet};

pub struct Glicko2;

impl RankingAlgorithm for Glicko2 {
    fn name(&self) -> &'static str {
        "glicko2"
    }
    fn compute(
        &self,
        _config: &serde_json::Value,
        _sets: &[ScoredSet],
    ) -> Result<Vec<super::PlayerScore>, AlgorithmError> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn set(winner: Uuid, loser: Uuid) -> ScoredSet {
        ScoredSet {
            winner_id: winner,
            loser_id: loser,
            completed_at: Utc::now(),
            winner_global_rating: None,
            loser_global_rating: None,
            is_external_winner: false,
            is_external_loser: false,
        }
    }

    #[test]
    fn glicko2_winner_higher_than_loser() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let config = json!({"tau": 0.5, "initial_rd": 350.0, "initial_volatility": 0.06});
        let scores = Glicko2.compute(&config, &[set(a, b)]).unwrap();
        let a_score = scores.iter().find(|s| s.player_id == a).unwrap();
        let b_score = scores.iter().find(|s| s.player_id == b).unwrap();
        assert!(a_score.computed_rating > b_score.computed_rating);
    }

    #[test]
    fn glicko2_display_data_has_rating_and_rd() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let config = json!({"tau": 0.5, "initial_rd": 350.0, "initial_volatility": 0.06});
        let scores = Glicko2.compute(&config, &[set(a, b)]).unwrap();
        let a_score = scores.iter().find(|s| s.player_id == a).unwrap();
        assert!(a_score.display_data["rating"].is_number());
        assert!(a_score.display_data["rd"].is_number());
    }
}
```

- [ ] **Step 3: Create the mod.rs with trait, types, and registry**

Create `backend/crates/common/src/algorithms/mod.rs`:

```rust
pub mod elo;
pub mod glicko2;

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ScoredSet {
    pub winner_id: Uuid,
    pub loser_id: Uuid,
    pub completed_at: DateTime<Utc>,
    pub winner_global_rating: Option<f64>,
    pub loser_global_rating: Option<f64>,
    pub is_external_winner: bool,
    pub is_external_loser: bool,
}

#[derive(Debug, Clone)]
pub struct PlayerScore {
    pub player_id: Uuid,
    pub computed_rating: f64,
    pub display_data: serde_json::Value,
    pub algorithm_state: serde_json::Value,
}

#[derive(Debug, thiserror::Error)]
pub enum AlgorithmError {
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("computation failed: {0}")]
    ComputationFailed(String),
}

pub trait RankingAlgorithm: Send + Sync {
    fn name(&self) -> &'static str;
    fn compute(
        &self,
        config: &serde_json::Value,
        sets: &[ScoredSet],
    ) -> Result<Vec<PlayerScore>, AlgorithmError>;
}

pub struct AlgorithmRegistry {
    algorithms: HashMap<&'static str, Box<dyn RankingAlgorithm>>,
}

impl AlgorithmRegistry {
    pub fn new() -> Self {
        let mut r = Self {
            algorithms: HashMap::new(),
        };
        r.register(Box::new(elo::Elo));
        r.register(Box::new(glicko2::Glicko2));
        r
    }

    fn register(&mut self, algo: Box<dyn RankingAlgorithm>) {
        self.algorithms.insert(algo.name(), algo);
    }

    pub fn get(&self, name: &str) -> Option<&dyn RankingAlgorithm> {
        self.algorithms.get(name).map(|b| b.as_ref())
    }
}

impl Default for AlgorithmRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Expose algorithms in lib.rs**

Replace `backend/crates/common/src/lib.rs`:

```rust
pub mod algorithms;
pub mod db;
pub mod error;
pub mod jobs;
pub mod models;
pub mod startgg;
pub mod upset;
```

- [ ] **Step 5: Add `thiserror` dependency**

```bash
cd backend && cargo add thiserror -p common
```

- [ ] **Step 6: Run tests — both should fail with `unimplemented!()`**

```bash
cd backend && cargo test -p common -- algorithms 2>&1 | grep -E "(FAILED|panicked|unimplemented)"
```
Expected: tests panic with `not implemented`.

- [ ] **Step 7: Implement Elo**

Replace the body of `Elo::compute` in `backend/crates/common/src/algorithms/elo.rs`:

```rust
use std::collections::{HashMap, HashSet};
use serde_json::json;
use uuid::Uuid;

use super::{AlgorithmError, PlayerScore, RankingAlgorithm, ScoredSet};

pub struct Elo;

impl RankingAlgorithm for Elo {
    fn name(&self) -> &'static str {
        "elo"
    }

    fn compute(
        &self,
        config: &serde_json::Value,
        sets: &[ScoredSet],
    ) -> Result<Vec<PlayerScore>, AlgorithmError> {
        let k = config["k_factor"].as_f64().unwrap_or(32.0);
        let initial = config["initial_rating"].as_f64().unwrap_or(1500.0);

        let mut ratings: HashMap<Uuid, f64> = HashMap::new();

        for s in sets {
            let ra = *ratings.entry(s.winner_id).or_insert(initial);
            let rb = *ratings.entry(s.loser_id).or_insert(initial);
            let ea = 1.0 / (1.0 + 10.0_f64.powf((rb - ra) / 400.0));
            ratings.insert(s.winner_id, ra + k * (1.0 - ea));
            ratings.insert(s.loser_id, rb + k * (0.0 - (1.0 - ea)));
        }

        let player_ids: HashSet<Uuid> = sets
            .iter()
            .flat_map(|s| [s.winner_id, s.loser_id])
            .collect();

        let scores = player_ids
            .into_iter()
            .map(|pid| {
                let r = *ratings.get(&pid).unwrap_or(&initial);
                PlayerScore {
                    player_id: pid,
                    computed_rating: r,
                    display_data: json!({ "rating": r.round() as i64 }),
                    algorithm_state: json!({}),
                }
            })
            .collect();

        Ok(scores)
    }
}

#[cfg(test)]
mod tests {
    // ... (keep existing tests from Step 1)
}
```

- [ ] **Step 8: Implement Glicko-2**

Replace the body of `Glicko2::compute` in `backend/crates/common/src/algorithms/glicko2.rs`:

```rust
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use serde_json::json;
use uuid::Uuid;

use super::{AlgorithmError, PlayerScore, RankingAlgorithm, ScoredSet};

pub struct Glicko2;

const SCALE: f64 = 173.7178;

fn g(phi: f64) -> f64 {
    1.0 / (1.0 + 3.0 * phi * phi / (PI * PI)).sqrt()
}

fn e(mu: f64, mu_j: f64, phi_j: f64) -> f64 {
    1.0 / (1.0 + (-g(phi_j) * (mu - mu_j)).exp())
}

// Illinois algorithm to find σ' (convergence on volatility)
fn new_sigma(sigma: f64, phi: f64, v: f64, delta: f64, tau: f64) -> f64 {
    let a = (sigma * sigma).ln();
    let delta_sq = delta * delta;
    let phi_sq = phi * phi;

    let f = |x: f64| -> f64 {
        let ex = x.exp();
        let num = ex * (delta_sq - phi_sq - v - ex);
        let den = 2.0 * (phi_sq + v + ex).powi(2);
        num / den - (x - a) / (tau * tau)
    };

    let mut a_val = a;
    let mut b_val = if delta_sq > phi_sq + v {
        (delta_sq - phi_sq - v).ln()
    } else {
        let mut k = 1.0;
        while f(a - k * tau) < 0.0 {
            k += 1.0;
        }
        a - k * tau
    };

    let eps = 1e-6;
    let mut fa = f(a_val);
    let mut fb = f(b_val);

    while (b_val - a_val).abs() > eps {
        let c = a_val + (a_val - b_val) * fa / (fb - fa);
        let fc = f(c);
        if fc * fb < 0.0 {
            a_val = b_val;
            fa = fb;
        } else {
            fa /= 2.0;
        }
        b_val = c;
        fb = fc;
    }

    ((a_val + b_val) / 2.0 / 2.0).exp().sqrt()
}

impl RankingAlgorithm for Glicko2 {
    fn name(&self) -> &'static str {
        "glicko2"
    }

    fn compute(
        &self,
        config: &serde_json::Value,
        sets: &[ScoredSet],
    ) -> Result<Vec<PlayerScore>, AlgorithmError> {
        let tau = config["tau"].as_f64().unwrap_or(0.5);
        let initial_rd = config["initial_rd"].as_f64().unwrap_or(350.0);
        let initial_vol = config["initial_volatility"].as_f64().unwrap_or(0.06);

        // Internal Glicko-2 scale
        let initial_mu = 0.0_f64;
        let initial_phi = initial_rd / SCALE;

        // State: (mu, phi, sigma) per player
        let mut state: HashMap<Uuid, (f64, f64, f64)> = HashMap::new();

        for s in sets {
            let (mu_w, phi_w, sigma_w) = *state
                .entry(s.winner_id)
                .or_insert((initial_mu, initial_phi, initial_vol));
            let (mu_l, phi_l, sigma_l) = *state
                .entry(s.loser_id)
                .or_insert((initial_mu, initial_phi, initial_vol));

            // Update winner (score = 1)
            let g_l = g(phi_l);
            let e_w = e(mu_w, mu_l, phi_l);
            let v_w = 1.0 / (g_l * g_l * e_w * (1.0 - e_w));
            let delta_w = v_w * g_l * (1.0 - e_w);
            let sigma_w_new = new_sigma(sigma_w, phi_w, v_w, delta_w, tau);
            let phi_star_w = (phi_w * phi_w + sigma_w_new * sigma_w_new).sqrt();
            let phi_w_new = 1.0 / (1.0 / (phi_star_w * phi_star_w) + 1.0 / v_w).sqrt();
            let mu_w_new = mu_w + phi_w_new * phi_w_new * g_l * (1.0 - e_w);

            // Update loser (score = 0)
            let g_w = g(phi_w);
            let e_l = e(mu_l, mu_w, phi_w);
            let v_l = 1.0 / (g_w * g_w * e_l * (1.0 - e_l));
            let delta_l = v_l * g_w * (0.0 - e_l);
            let sigma_l_new = new_sigma(sigma_l, phi_l, v_l, delta_l, tau);
            let phi_star_l = (phi_l * phi_l + sigma_l_new * sigma_l_new).sqrt();
            let phi_l_new = 1.0 / (1.0 / (phi_star_l * phi_star_l) + 1.0 / v_l).sqrt();
            let mu_l_new = mu_l + phi_l_new * phi_l_new * g_w * (0.0 - e_l);

            state.insert(s.winner_id, (mu_w_new, phi_w_new, sigma_w_new));
            state.insert(s.loser_id, (mu_l_new, phi_l_new, sigma_l_new));
        }

        let player_ids: HashSet<Uuid> = sets
            .iter()
            .flat_map(|s| [s.winner_id, s.loser_id])
            .collect();

        let scores = player_ids
            .into_iter()
            .map(|pid| {
                let (mu, phi, sigma) = *state.get(&pid).unwrap_or(&(initial_mu, initial_phi, initial_vol));
                let r = SCALE * mu + 1500.0;
                let rd = (SCALE * phi).round() as i64;
                PlayerScore {
                    player_id: pid,
                    computed_rating: r,
                    display_data: json!({ "rating": r.round() as i64, "rd": rd }),
                    algorithm_state: json!({ "mu": mu, "phi": phi, "sigma": sigma }),
                }
            })
            .collect();

        Ok(scores)
    }
}

#[cfg(test)]
mod tests {
    // ... (keep existing tests from Step 2)
}
```

- [ ] **Step 9: Run algorithm tests — all should pass**

```bash
cd backend && cargo test -p common -- algorithms 2>&1 | grep -E "(test .* ok|FAILED)"
```
Expected: all tests pass.

- [ ] **Step 10: Commit**

```bash
git add backend/crates/common/src/algorithms/ backend/crates/common/src/lib.rs backend/crates/common/Cargo.toml
git commit -m "feat(common): add RankingAlgorithm trait, Elo and Glicko-2 implementations, AlgorithmRegistry"
```

---

## Task 4: Add compute_ranking Job Enqueue to Common

**Files:**
- Modify: `backend/crates/common/src/jobs.rs`

- [ ] **Step 1: Add `ComputeRankingParams` and `enqueue_compute_ranking`**

After the existing `enqueue` function in `backend/crates/common/src/jobs.rs`, add:

```rust
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ComputeRankingParams {
    pub ranking_id: Uuid,
}

impl ComputeRankingParams {
    pub fn from_job(job: &Job) -> Self {
        serde_json::from_value(job.params.clone()).unwrap_or_default()
    }
}

pub async fn enqueue_compute_ranking(
    pool: &PgPool,
    project_id: Uuid,
    ranking_id: Uuid,
) -> Result<Job, sqlx::Error> {
    let params_json = serde_json::to_value(&ComputeRankingParams { ranking_id }).unwrap_or_default();
    let job = sqlx::query_as!(
        Job,
        r#"INSERT INTO jobs (kind, project_id, params, status)
           VALUES ('compute_ranking', $1, $2, 'pending')
           RETURNING id, kind::text AS "kind!", project_id, params, result, progress,
                     status::text AS "status!", error, created_at, updated_at"#,
        project_id,
        params_json,
    )
    .fetch_one(pool)
    .await?;

    sqlx::query!("SELECT pg_notify('jobs', $1)", job.id.to_string())
        .execute(pool)
        .await?;

    Ok(job)
}
```

- [ ] **Step 2: Write a test for enqueue_compute_ranking**

In the `#[cfg(test)]` block of `jobs.rs`:

```rust
    #[sqlx::test(migrations = "../../migrations")]
    async fn enqueue_compute_ranking_creates_job(pool: PgPool) {
        let project_id = setup_project(&pool).await;
        let ranking_id: Uuid = sqlx::query_scalar!(
            "INSERT INTO rankings (project_id, name) VALUES ($1, 'Test') RETURNING id",
            project_id,
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let job = enqueue_compute_ranking(&pool, project_id, ranking_id)
            .await
            .unwrap();
        assert_eq!(job.kind, "compute_ranking");

        let params = ComputeRankingParams::from_job(&job);
        assert_eq!(params.ranking_id, ranking_id);
    }
```

- [ ] **Step 3: Run the test**

```bash
cd backend && cargo test -p common -- enqueue_compute_ranking 2>&1 | tail -5
```
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add backend/crates/common/src/jobs.rs
git commit -m "feat(common): add ComputeRankingParams and enqueue_compute_ranking"
```

---

## Task 5: Worker compute.rs — Phase 1 and Phase 2

Create the compute job handler. Phase 1 builds `ranking_set_results` for all rankings. Phase 2 runs the algorithm and writes `ranking_player_scores` for algorithmic rankings.

**Files:**
- Create: `backend/crates/worker/src/compute.rs`

- [ ] **Step 1: Create compute.rs**

Create `backend/crates/worker/src/compute.rs`:

```rust
use sqlx::PgPool;
use uuid::Uuid;

use common::algorithms::{AlgorithmRegistry, ScoredSet};
use common::upset::set_upset_factor;

pub async fn run(pool: &PgPool, ranking_id: Uuid) -> anyhow::Result<()> {
    // Load ranking to determine algorithm
    let ranking = sqlx::query!(
        r#"SELECT algorithm, algorithm_config, include_external_results
           FROM rankings WHERE id = $1"#,
        ranking_id,
    )
    .fetch_optional(pool)
    .await?;

    let Some(ranking) = ranking else {
        anyhow::bail!("ranking {ranking_id} not found");
    };

    phase1_set_results(pool, ranking_id).await?;

    if let Some(ref algo_name) = ranking.algorithm {
        phase2_algorithm_scores(
            pool,
            ranking_id,
            algo_name,
            &ranking.algorithm_config,
        )
        .await?;
    }

    Ok(())
}

/// Phase 1: build ranking_set_results for this ranking.
/// Loads all sets from included events where both entrants are ranking members,
/// computes upset factor, and atomically replaces the stored results.
async fn phase1_set_results(pool: &PgPool, ranking_id: Uuid) -> anyhow::Result<()> {
    struct SetRow {
        set_id: Uuid,
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        event_id: Uuid,
        winner_seed: Option<i32>,
        loser_seed: Option<i32>,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let sets = sqlx::query_as!(
        SetRow,
        r#"
        SELECT
            s.id            AS set_id,
            we.player_id    AS "winner_player_id!: Uuid",
            le.player_id    AS "loser_player_id!: Uuid",
            s.event_id,
            we.seed         AS winner_seed,
            le.seed         AS loser_seed,
            s.completed_at
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN ranking_players rwp ON rwp.player_id = we.player_id AND rwp.ranking_id = $1
        JOIN ranking_players rlp ON rlp.player_id = le.player_id AND rlp.ranking_id = $1
        JOIN ranking_events re ON re.event_id = s.event_id AND re.ranking_id = $1
        WHERE re.included       = true
          AND s.is_dq           = false
          AND s.has_placeholder = false
          AND we.player_id IS NOT NULL
          AND le.player_id IS NOT NULL
        ORDER BY s.completed_at ASC NULLS LAST
        "#,
        ranking_id,
    )
    .fetch_all(pool)
    .await?;

    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM ranking_set_results WHERE ranking_id = $1",
        ranking_id,
    )
    .execute(&mut *tx)
    .await?;

    for row in &sets {
        let upset_factor = match (row.winner_seed, row.loser_seed) {
            (Some(ws), Some(ls)) => Some(set_upset_factor(ws, ls) as f64),
            _ => None,
        };

        sqlx::query!(
            r#"
            INSERT INTO ranking_set_results
                (ranking_id, set_id, winner_player_id, loser_player_id, event_id, upset_factor, completed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            ranking_id,
            row.set_id,
            row.winner_player_id,
            row.loser_player_id,
            row.event_id,
            upset_factor,
            row.completed_at,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    tracing::info!(%ranking_id, count = sets.len(), "phase1: wrote ranking_set_results");
    Ok(())
}

/// Phase 2: run the algorithm and write ranking_player_scores.
/// Reads raw sets directly (independent of Phase 1's output).
async fn phase2_algorithm_scores(
    pool: &PgPool,
    ranking_id: Uuid,
    algo_name: &str,
    config: &serde_json::Value,
) -> anyhow::Result<()> {
    let registry = AlgorithmRegistry::new();
    let algo = registry
        .get(algo_name)
        .ok_or_else(|| anyhow::anyhow!("unknown algorithm: {}", algo_name))?;

    struct SetRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        completed_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let rows = sqlx::query_as!(
        SetRow,
        r#"
        SELECT
            we.player_id AS "winner_player_id!: Uuid",
            le.player_id AS "loser_player_id!: Uuid",
            s.completed_at
        FROM sets s
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN ranking_players rwp ON rwp.player_id = we.player_id AND rwp.ranking_id = $1
        JOIN ranking_players rlp ON rlp.player_id = le.player_id AND rlp.ranking_id = $1
        JOIN ranking_events re ON re.event_id = s.event_id AND re.ranking_id = $1
        WHERE re.included       = true
          AND s.is_dq           = false
          AND s.has_placeholder = false
          AND we.player_id IS NOT NULL
          AND le.player_id IS NOT NULL
        ORDER BY s.completed_at ASC NULLS LAST
        "#,
        ranking_id,
    )
    .fetch_all(pool)
    .await?;

    let scored_sets: Vec<ScoredSet> = rows
        .into_iter()
        .map(|r| ScoredSet {
            winner_id: r.winner_player_id,
            loser_id: r.loser_player_id,
            completed_at: r.completed_at.unwrap_or_default(),
            winner_global_rating: None,
            loser_global_rating: None,
            is_external_winner: false,
            is_external_loser: false,
        })
        .collect();

    let scores = algo
        .compute(config, &scored_sets)
        .map_err(|e| anyhow::anyhow!("algorithm error: {e}"))?;

    let mut tx = pool.begin().await?;

    sqlx::query!(
        "DELETE FROM ranking_player_scores WHERE ranking_id = $1",
        ranking_id,
    )
    .execute(&mut *tx)
    .await?;

    for score in &scores {
        sqlx::query!(
            r#"
            INSERT INTO ranking_player_scores
                (ranking_id, player_id, computed_rating, display_data, algorithm_state)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            ranking_id,
            score.player_id,
            score.computed_rating,
            score.display_data,
            score.algorithm_state,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    tracing::info!(%ranking_id, count = scores.len(), "phase2: wrote ranking_player_scores");
    Ok(())
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd backend && cargo check -p worker 2>&1 | head -20
```
Expected: compile errors about `compute` not being in scope in main.rs — fix in Task 6.

- [ ] **Step 3: Commit**

```bash
git add backend/crates/worker/src/compute.rs
git commit -m "feat(worker): add compute_ranking job handler (Phase 1: set results, Phase 2: algorithm scores)"
```

---

## Task 6: Update Worker Dispatch + Import Trigger

Update `main.rs` to handle both job kinds, and update `import.rs` to enqueue `compute_ranking` for all rankings after an import completes.

**Files:**
- Modify: `backend/crates/worker/src/main.rs`
- Modify: `backend/crates/worker/src/import.rs`

- [ ] **Step 1: Update main.rs to declare compute module and dispatch by kind**

At line 9 in `backend/crates/worker/src/main.rs`, add `mod compute;`:

```rust
mod compute;
mod config;
mod import;
```

Then replace the job-claim-and-dispatch loop body (lines 76–130) with a version that dispatches by `job.kind`:

```rust
                Ok(Some(job)) => {
                    let pool2 = pool.clone();
                    let project_id = job.project_id;
                    let job_id = job.id;

                    let handle = match job.kind.as_str() {
                        "import_tournaments" => {
                            let import_params = common::jobs::ImportParams::from_job(&job);
                            let api_key = match sqlx::query_scalar!(
                                "SELECT u.startgg_api_key FROM projects rp
                                 JOIN users u ON u.id = rp.owner_id
                                 WHERE rp.id = $1",
                                project_id,
                            )
                            .fetch_optional(&pool)
                            .await
                            {
                                Ok(Some(Some(key))) => key,
                                Ok(_) => {
                                    tracing::error!(%job_id, %project_id, "project owner has no start.gg API key");
                                    let _ = common::jobs::mark_failed(
                                        &pool,
                                        job_id,
                                        "Project owner has no start.gg API key configured",
                                    )
                                    .await;
                                    continue;
                                }
                                Err(e) => {
                                    tracing::error!(%e, %job_id, "failed to look up owner API key");
                                    let _ = common::jobs::mark_failed(&pool, job_id, &e.to_string()).await;
                                    continue;
                                }
                            };
                            let startgg = common::startgg::StartggClient::new(api_key);
                            tracing::info!(%job_id, %project_id, "starting import");
                            tokio::spawn(async move {
                                match import::run(&pool2, &startgg, project_id, job_id, import_params).await {
                                    Ok(()) => {
                                        tracing::info!(%job_id, "import complete");
                                        if let Err(e) = common::jobs::mark_done(&pool2, job_id).await {
                                            tracing::error!(%e, %job_id, "failed to mark job done");
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(%e, %job_id, "import failed");
                                        if let Err(e2) = common::jobs::mark_failed(&pool2, job_id, &e.to_string()).await {
                                            tracing::error!(%e2, %job_id, "failed to mark job failed");
                                        }
                                    }
                                }
                            })
                        }
                        "compute_ranking" => {
                            let params = common::jobs::ComputeRankingParams::from_job(&job);
                            tracing::info!(%job_id, ranking_id = %params.ranking_id, "starting compute_ranking");
                            tokio::spawn(async move {
                                match compute::run(&pool2, params.ranking_id).await {
                                    Ok(()) => {
                                        tracing::info!(%job_id, "compute_ranking complete");
                                        if let Err(e) = common::jobs::mark_done(&pool2, job_id).await {
                                            tracing::error!(%e, %job_id, "failed to mark job done");
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!(%e, %job_id, "compute_ranking failed");
                                        if let Err(e2) = common::jobs::mark_failed(&pool2, job_id, &e.to_string()).await {
                                            tracing::error!(%e2, %job_id, "failed to mark job failed");
                                        }
                                    }
                                }
                            })
                        }
                        kind => {
                            tracing::warn!(%job_id, %kind, "unknown job kind, marking failed");
                            let _ = common::jobs::mark_failed(&pool, job_id, &format!("unknown job kind: {kind}")).await;
                            continue;
                        }
                    };
                    in_flight.push((job_id, handle));
                }
```

- [ ] **Step 2: Update import.rs to enqueue compute_ranking after import**

In `backend/crates/worker/src/import.rs`, at the end of `run()`, after `seed_ranking_by_winrate` and before `Ok(())`:

```rust
    // Enqueue compute_ranking for every ranking in this project
    let ranking_ids: Vec<Uuid> = sqlx::query_scalar!(
        "SELECT id FROM rankings WHERE project_id = $1",
        project_id,
    )
    .fetch_all(pool)
    .await?;

    for rid in ranking_ids {
        if let Err(e) = common::jobs::enqueue_compute_ranking(pool, project_id, rid).await {
            tracing::warn!(%e, %rid, "failed to enqueue compute_ranking after import");
        }
    }
```

- [ ] **Step 3: Verify the worker compiles**

```bash
cd backend && cargo check -p worker 2>&1 | head -20
```
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add backend/crates/worker/src/main.rs backend/crates/worker/src/import.rs
git commit -m "feat(worker): dispatch compute_ranking jobs; enqueue after import completes"
```

---

## Task 7: Update API Rankings Routes

Extend the `rankings.rs` routes to accept and persist the new ranking fields, add the `recompute` endpoint, enqueue compute on player add/remove, and add the `get_computed_ranking` endpoint.

**Files:**
- Modify: `backend/crates/api/src/routes/rankings.rs`

- [ ] **Step 1: Update all queries that SELECT from rankings**

Every `sqlx::query_as!(Ranking, ...)` must select the new columns. The `Ranking` struct now has `algorithm`, `algorithm_config`, `include_external_results`, `result_sort`.

Update the SELECT in `require_ranking_access`:
```rust
    let ranking = sqlx::query_as!(
        Ranking,
        r#"SELECT id, project_id, name, description, published,
                  algorithm, algorithm_config, include_external_results, result_sort,
                  created_at
           FROM rankings WHERE id = $1 AND project_id = $2"#,
        ranking_id,
        project_id,
    )
```

Update the inline struct and manual construction in `require_ranking_read_access`. The struct needs the new fields, and the manual `Ranking { ... }` construction needs them too:

```rust
    struct Row {
        id: Uuid,
        owner_id: Uuid,
        name: String,
        game_id: Option<i64>,
        game_name: Option<String>,
        created_at: DateTime<Utc>,
        ranking_id: Uuid,
        ranking_name: String,
        ranking_description: Option<String>,
        ranking_published: bool,
        ranking_algorithm: Option<String>,
        ranking_algorithm_config: serde_json::Value,
        ranking_include_external: bool,
        ranking_result_sort: String,
        ranking_created_at: DateTime<Utc>,
        member_role: Option<MemberRole>,
    }

    let row = sqlx::query_as!(
        Row,
        r#"SELECT p.id, p.owner_id, p.name, p.game_id, p.game_name, p.created_at,
                  r.id AS ranking_id, r.name AS ranking_name,
                  r.description AS ranking_description,
                  r.published AS ranking_published,
                  r.algorithm AS ranking_algorithm,
                  r.algorithm_config AS ranking_algorithm_config,
                  r.include_external_results AS ranking_include_external,
                  r.result_sort AS ranking_result_sort,
                  r.created_at AS ranking_created_at,
                  CASE WHEN pm.user_id IS NOT NULL THEN pm.role END AS "member_role: MemberRole"
           FROM projects p
           JOIN rankings r ON r.id = $2 AND r.project_id = p.id
           LEFT JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = $3
           WHERE p.id = $1"#,
        ...
    )
    ...
    let ranking = Ranking {
        id: row.ranking_id,
        project_id,
        name: row.ranking_name,
        description: row.ranking_description,
        published: row.ranking_published,
        algorithm: row.ranking_algorithm,
        algorithm_config: row.ranking_algorithm_config,
        include_external_results: row.ranking_include_external,
        result_sort: row.ranking_result_sort,
        created_at: row.ranking_created_at,
    };
```

Update `list_rankings`:
```rust
    let rankings = sqlx::query_as!(
        Ranking,
        r#"SELECT id, project_id, name, description, published,
                  algorithm, algorithm_config, include_external_results, result_sort,
                  created_at
           FROM rankings WHERE project_id = $1 ORDER BY created_at ASC"#,
        project_id,
    )
```

Update `create_ranking` RETURNING clause:
```rust
        r#"INSERT INTO rankings (project_id, name, description, algorithm, algorithm_config, include_external_results, result_sort)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, project_id, name, description, published,
                     algorithm, algorithm_config, include_external_results, result_sort, created_at"#,
```

Update `patch_ranking` RETURNING clause similarly.

- [ ] **Step 2: Extend request/response types**

Replace `CreateRankingRequest`, `PatchRankingRequest`, and `RankingResponse`:

```rust
#[derive(Deserialize)]
struct CreateRankingRequest {
    name: String,
    description: Option<String>,
    algorithm: Option<String>,
    algorithm_config: Option<serde_json::Value>,
    include_external_results: Option<bool>,
    result_sort: Option<String>,
}

#[derive(Deserialize)]
struct PatchRankingRequest {
    name: Option<String>,
    description: Option<String>,
    published: Option<bool>,
    algorithm: Option<serde_json::Value>,      // serde_json::Value for nullable patch: null clears it
    algorithm_config: Option<serde_json::Value>,
    include_external_results: Option<bool>,
    result_sort: Option<String>,
}

#[derive(Serialize)]
struct RankingResponse {
    id: Uuid,
    project_id: Uuid,
    name: String,
    description: Option<String>,
    published: bool,
    algorithm: Option<String>,
    algorithm_config: serde_json::Value,
    include_external_results: bool,
    result_sort: String,
    created_at: DateTime<Utc>,
    user_role: Option<UserRole>,
}

impl RankingResponse {
    fn from_ranking(r: Ranking, role: Option<UserRole>) -> Self {
        RankingResponse {
            id: r.id,
            project_id: r.project_id,
            name: r.name,
            description: r.description,
            published: r.published,
            algorithm: r.algorithm,
            algorithm_config: r.algorithm_config,
            include_external_results: r.include_external_results,
            result_sort: r.result_sort,
            created_at: r.created_at,
            user_role: role,
        }
    }
}
```

- [ ] **Step 3: Update create_ranking handler**

In `create_ranking`, update the INSERT to pass the new fields. Use `serde_json::json!({})` as default for `algorithm_config`:

```rust
    let config = body.algorithm_config.clone().unwrap_or_else(|| serde_json::json!({}));
    let result_sort = body.result_sort.as_deref().unwrap_or("upset_factor").to_string();

    let ranking = sqlx::query_as!(
        Ranking,
        r#"INSERT INTO rankings (project_id, name, description, algorithm, algorithm_config, include_external_results, result_sort)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, project_id, name, description, published,
                     algorithm, algorithm_config, include_external_results, result_sort, created_at"#,
        project_id,
        body.name.trim(),
        body.description.as_deref(),
        body.algorithm.as_deref(),
        config,
        body.include_external_results.unwrap_or(false),
        result_sort,
    )
```

- [ ] **Step 4: Update patch_ranking handler**

Update the UPDATE in `patch_ranking` to handle the new fields:

```rust
    let updated = sqlx::query_as!(
        Ranking,
        r#"UPDATE rankings
           SET name                     = $1,
               description              = $2,
               published                = $3,
               algorithm_config         = COALESCE($4, algorithm_config),
               include_external_results = COALESCE($5, include_external_results),
               result_sort              = COALESCE($6, result_sort)
           WHERE id = $7
           RETURNING id, project_id, name, description, published,
                     algorithm, algorithm_config, include_external_results, result_sort, created_at"#,
        new_name,
        body.description.as_deref().or(ranking.description.as_deref()),
        body.published.unwrap_or(ranking.published),
        body.algorithm_config.as_ref(),
        body.include_external_results,
        body.result_sort.as_deref(),
        path.rid,
    )
```

- [ ] **Step 5: Add recompute endpoint**

Add after `delete_ranking`:

```rust
async fn recompute_ranking(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    let (project, _, _) =
        require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;

    common::jobs::enqueue_compute_ranking(&state.db, project.id, path.rid).await?;
    Ok(StatusCode::ACCEPTED)
}
```

- [ ] **Step 6: Add get_computed_ranking endpoint**

Add a handler that returns players ordered by `computed_rating` for algorithmic rankings, falling back to `rank_position` for manual:

```rust
#[derive(Serialize)]
struct ComputedRankingPlayerResponse {
    player_id: Uuid,
    name: String,
    rank_position: i32,
    notes: Option<String>,
    computed_rating: Option<f64>,
    display_data: Option<serde_json::Value>,
}

async fn get_computed_ranking(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    let (_, ranking, _) =
        require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    if ranking.algorithm.is_some() {
        // Algorithmic: order by computed_rating DESC, join scores
        struct Row {
            player_id: Uuid,
            name: String,
            rank_position: i32,
            notes: Option<String>,
            computed_rating: Option<f64>,
            display_data: Option<serde_json::Value>,
        }
        let rows = sqlx::query_as!(
            Row,
            r#"
            SELECT rp.player_id, pl.name, rp.rank_position, rp.notes,
                   rps.computed_rating, rps.display_data
            FROM ranking_players rp
            JOIN players pl ON pl.id = rp.player_id
            LEFT JOIN ranking_player_scores rps ON rps.ranking_id = $1 AND rps.player_id = rp.player_id
            WHERE rp.ranking_id = $1
            ORDER BY rps.computed_rating DESC NULLS LAST, pl.created_at ASC
            "#,
            path.rid,
        )
        .fetch_all(&state.db)
        .await?;

        let resp: Vec<ComputedRankingPlayerResponse> = rows
            .into_iter()
            .map(|r| ComputedRankingPlayerResponse {
                player_id: r.player_id,
                name: r.name,
                rank_position: r.rank_position,
                notes: r.notes,
                computed_rating: r.computed_rating,
                display_data: r.display_data,
            })
            .collect();
        Ok(Json(resp))
    } else {
        // Manual: order by rank_position
        struct Row {
            player_id: Uuid,
            name: String,
            rank_position: i32,
            notes: Option<String>,
        }
        let rows = sqlx::query_as!(
            Row,
            r#"
            SELECT rp.player_id, pl.name, rp.rank_position, rp.notes
            FROM ranking_players rp
            JOIN players pl ON pl.id = rp.player_id
            WHERE rp.ranking_id = $1
            ORDER BY rp.rank_position ASC, pl.created_at ASC
            "#,
            path.rid,
        )
        .fetch_all(&state.db)
        .await?;

        let resp: Vec<ComputedRankingPlayerResponse> = rows
            .into_iter()
            .map(|r| ComputedRankingPlayerResponse {
                player_id: r.player_id,
                name: r.name,
                rank_position: r.rank_position,
                notes: r.notes,
                computed_rating: None,
                display_data: None,
            })
            .collect();
        Ok(Json(resp))
    }
}
```

- [ ] **Step 7: Enqueue compute_ranking after player add/remove**

In `add_ranking_player`, after the INSERT, add:
```rust
    // Enqueue recompute (ignore error — job will be retried on next import)
    let _ = common::jobs::enqueue_compute_ranking(&state.db, path.id, path.rid).await;
```

In `remove_ranking_player`, after the DELETE check, add:
```rust
    let _ = common::jobs::enqueue_compute_ranking(&state.db, path.id, path.rid).await;
```

(The `project_id` for `enqueue_compute_ranking` here is `path.id`.)

- [ ] **Step 8: Update router to add new routes**

In `rankings.rs` `router()`, update the route for `/{rid}/ranking` and add `/recompute`:

```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_rankings).post(create_ranking))
        .route(
            "/{rid}",
            get(get_ranking).patch(patch_ranking).delete(delete_ranking),
        )
        .route(
            "/{rid}/players",
            get(list_ranking_players).post(add_ranking_player),
        )
        .route(
            "/{rid}/players/{pid}",
            delete(remove_ranking_player).patch(patch_ranking_player),
        )
        .route(
            "/{rid}/ranking",
            get(get_computed_ranking).put(reorder_ranking_players),
        )
        .route("/{rid}/recompute", axum::routing::post(recompute_ranking))
        .nest("/{rid}", crate::routes::tournaments::router())
}
```

- [ ] **Step 9: Verify API compiles**

```bash
cd backend && cargo check -p api 2>&1 | head -30
```
Expected: clean or only errors in `tournaments.rs` (fixed in Task 8).

- [ ] **Step 10: Run existing ranking tests**

```bash
cd backend && cargo test -p api -- rankings 2>&1 | tail -10
```
Expected: all pass (the test helpers use minimal JSON bodies that still work).

- [ ] **Step 11: Commit**

```bash
git add backend/crates/api/src/routes/rankings.rs
git commit -m "feat(api): extend ranking CRUD with algorithm fields; add recompute and computed-ranking endpoints"
```

---

## Task 8: Update API Tournaments Routes

Replace `patch_event` with `put_events` (bulk save) and update stats/H2H endpoints to read from `ranking_set_results`.

**Files:**
- Modify: `backend/crates/api/src/routes/tournaments.rs`

- [ ] **Step 1: Replace patch_event with put_events**

Remove the `patch_event` handler and `PatchEventBody`. Add:

```rust
#[derive(Deserialize)]
pub struct EventInclusionItem {
    pub event_id: Uuid,
    pub included: bool,
}

pub async fn put_events(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(path): Path<RankingPath>,
    Json(body): Json<Vec<EventInclusionItem>>,
) -> Result<impl IntoResponse> {
    let (project, _, _) =
        require_ranking_access(&state.db, path.id, path.rid, user.id, UserRole::Editor).await?;

    let mut tx = state.db.begin().await?;
    for item in &body {
        sqlx::query!(
            r#"
            UPDATE ranking_events
            SET included = $1
            WHERE ranking_id = $2 AND event_id = $3
            "#,
            item.included,
            path.rid,
            item.event_id,
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    // Enqueue a single recompute
    let _ = common::jobs::enqueue_compute_ranking(&state.db, project.id, path.rid).await;

    Ok(StatusCode::ACCEPTED)
}
```

- [ ] **Step 2: Update get_stats to read from ranking_set_results**

Replace the `get_stats` handler. The core change: instead of a 9-table runtime JOIN with inline ranking_players filtering, start from `ranking_set_results` and JOIN outward for display data:

```rust
pub async fn get_stats(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    struct PlayerRow {
        id: Uuid,
        name: String,
    }
    let players = sqlx::query_as!(
        PlayerRow,
        r#"
        SELECT rp.player_id AS id, pl.name
        FROM ranking_players rp
        JOIN players pl ON pl.id = rp.player_id
        WHERE rp.ranking_id = $1
        ORDER BY rp.rank_position ASC, pl.created_at ASC
        "#,
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    struct RsrRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        upset_factor: Option<f64>,
        completed_at: Option<DateTime<Utc>>,
        winner_name: String,
        loser_name: String,
        winner_score: Option<i16>,
        loser_score: Option<i16>,
        round_name: Option<String>,
        is_dq: bool,
        vod_url: Option<String>,
        startgg_set_id: i64,
        winner_seed: Option<i32>,
        loser_seed: Option<i32>,
        winner_placement: Option<i32>,
        loser_placement: Option<i32>,
        event_name: String,
        event_handle: Option<String>,
        num_entrants: Option<i32>,
        tournament_name: String,
        tournament_handle: String,
        online: bool,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        phase_name: Option<String>,
        pool_identifier: Option<String>,
    }

    let rows = sqlx::query_as!(
        RsrRow,
        r#"
        SELECT
            rsr.winner_player_id,
            rsr.loser_player_id,
            rsr.upset_factor,
            rsr.completed_at,
            wp.name                             AS winner_name,
            lp.name                             AS loser_name,
            s.winner_score,
            s.loser_score,
            s.round_name,
            s.is_dq,
            s.vod_url,
            s.startgg_set_id,
            we.seed                             AS winner_seed,
            le.seed                             AS loser_seed,
            we.final_placement                  AS winner_placement,
            le.final_placement                  AS loser_placement,
            e.name                              AS event_name,
            e.handle                            AS "event_handle?: String",
            e.num_entrants,
            t.name                              AS tournament_name,
            t.handle                            AS tournament_handle,
            t.online,
            t.city,
            t.addr_state,
            t.country_code,
            ph.name                             AS "phase_name?: String",
            pg.display_identifier               AS "pool_identifier?: String"
        FROM ranking_set_results rsr
        JOIN sets s  ON s.id  = rsr.set_id
        JOIN players wp ON wp.id = rsr.winner_player_id
        JOIN players lp ON lp.id = rsr.loser_player_id
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN events e   ON e.id  = rsr.event_id
        JOIN tournaments t ON t.id = e.tournament_id
        LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
        LEFT JOIN phases ph       ON ph.id = pg.phase_id
        WHERE rsr.ranking_id = $1
        "#,
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    let player_order: Vec<Uuid> = players.iter().map(|p| p.id).collect();
    let mut stats: HashMap<Uuid, (String, Vec<SetRecord>, Vec<SetRecord>)> = players
        .into_iter()
        .map(|p| (p.id, (p.name, Vec::new(), Vec::new())))
        .collect();

    for row in rows {
        let uf = row.upset_factor.unwrap_or(0.0).round() as i64;
        let location = compute_location(
            row.online,
            row.city.as_deref(),
            row.addr_state.as_deref(),
            row.country_code.as_deref(),
        );
        let make_record = |opponent_id: Uuid, opponent_name: String| SetRecord {
            opponent_id,
            opponent_name,
            upset_factor: uf,
            winner_score: row.winner_score,
            loser_score: row.loser_score,
            tournament_name: row.tournament_name.clone(),
            tournament_handle: row.tournament_handle.clone(),
            event_name: row.event_name.clone(),
            round_name: row.round_name.clone(),
            completed_at: row.completed_at,
            is_dq: row.is_dq,
            vod_url: row.vod_url.clone(),
            startgg_set_id: row.startgg_set_id,
            winner_seed: row.winner_seed,
            loser_seed: row.loser_seed,
            phase_name: row.phase_name.clone(),
            pool_identifier: row.pool_identifier.clone(),
            winner_placement: row.winner_placement,
            loser_placement: row.loser_placement,
            location: location.clone(),
            num_entrants: row.num_entrants,
            event_handle: row.event_handle.clone(),
        };
        if let Some(entry) = stats.get_mut(&row.winner_player_id) {
            entry.1.push(make_record(row.loser_player_id, row.loser_name.clone()));
        }
        if let Some(entry) = stats.get_mut(&row.loser_player_id) {
            entry.2.push(make_record(row.winner_player_id, row.winner_name.clone()));
        }
    }

    for entry in stats.values_mut() {
        entry.1.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));
        entry.2.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));
    }

    let resp: Vec<PlayerStatsResponse> = player_order
        .iter()
        .filter_map(|&id| {
            stats.remove(&id).map(|(name, wins, losses)| PlayerStatsResponse {
                player_id: id,
                name,
                wins,
                losses,
            })
        })
        .collect();

    Ok(Json(resp))
}
```

- [ ] **Step 3: Update get_player_stats to read from ranking_set_results**

Replace `get_player_stats` with an equivalent that starts from `ranking_set_results` filtered by player:

```rust
pub async fn get_player_stats(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPlayerStatPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    let name: Option<String> = sqlx::query_scalar!(
        r#"SELECT pl.name FROM ranking_players rp
           JOIN players pl ON pl.id = rp.player_id
           WHERE rp.ranking_id = $1 AND rp.player_id = $2"#,
        path.rid,
        path.player_id,
    )
    .fetch_optional(&state.db)
    .await?;
    let name = name.ok_or(AppError::NotFound)?;

    // Same query as get_stats but filtered to sets involving this player
    struct RsrRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        upset_factor: Option<f64>,
        completed_at: Option<DateTime<Utc>>,
        winner_name: String,
        loser_name: String,
        winner_score: Option<i16>,
        loser_score: Option<i16>,
        round_name: Option<String>,
        is_dq: bool,
        vod_url: Option<String>,
        startgg_set_id: i64,
        winner_seed: Option<i32>,
        loser_seed: Option<i32>,
        winner_placement: Option<i32>,
        loser_placement: Option<i32>,
        event_name: String,
        event_handle: Option<String>,
        num_entrants: Option<i32>,
        tournament_name: String,
        tournament_handle: String,
        online: bool,
        city: Option<String>,
        addr_state: Option<String>,
        country_code: Option<String>,
        phase_name: Option<String>,
        pool_identifier: Option<String>,
    }

    let rows = sqlx::query_as!(
        RsrRow,
        r#"
        SELECT
            rsr.winner_player_id,
            rsr.loser_player_id,
            rsr.upset_factor,
            rsr.completed_at,
            wp.name                             AS winner_name,
            lp.name                             AS loser_name,
            s.winner_score,
            s.loser_score,
            s.round_name,
            s.is_dq,
            s.vod_url,
            s.startgg_set_id,
            we.seed                             AS winner_seed,
            le.seed                             AS loser_seed,
            we.final_placement                  AS winner_placement,
            le.final_placement                  AS loser_placement,
            e.name                              AS event_name,
            e.handle                            AS "event_handle?: String",
            e.num_entrants,
            t.name                              AS tournament_name,
            t.handle                            AS tournament_handle,
            t.online,
            t.city,
            t.addr_state,
            t.country_code,
            ph.name                             AS "phase_name?: String",
            pg.display_identifier               AS "pool_identifier?: String"
        FROM ranking_set_results rsr
        JOIN sets s  ON s.id  = rsr.set_id
        JOIN players wp ON wp.id = rsr.winner_player_id
        JOIN players lp ON lp.id = rsr.loser_player_id
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN events e   ON e.id  = rsr.event_id
        JOIN tournaments t ON t.id = e.tournament_id
        LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
        LEFT JOIN phases ph       ON ph.id = pg.phase_id
        WHERE rsr.ranking_id = $1
          AND (rsr.winner_player_id = $2 OR rsr.loser_player_id = $2)
        "#,
        path.rid,
        path.player_id,
    )
    .fetch_all(&state.db)
    .await?;

    let mut wins: Vec<SetRecord> = Vec::new();
    let mut losses: Vec<SetRecord> = Vec::new();

    for row in rows {
        let uf = row.upset_factor.unwrap_or(0.0).round() as i64;
        let location = compute_location(
            row.online,
            row.city.as_deref(),
            row.addr_state.as_deref(),
            row.country_code.as_deref(),
        );
        let rec = |opponent_id: Uuid, opponent_name: String| SetRecord {
            opponent_id,
            opponent_name,
            upset_factor: uf,
            winner_score: row.winner_score,
            loser_score: row.loser_score,
            tournament_name: row.tournament_name.clone(),
            tournament_handle: row.tournament_handle.clone(),
            event_name: row.event_name.clone(),
            round_name: row.round_name.clone(),
            completed_at: row.completed_at,
            is_dq: row.is_dq,
            vod_url: row.vod_url.clone(),
            startgg_set_id: row.startgg_set_id,
            winner_seed: row.winner_seed,
            loser_seed: row.loser_seed,
            phase_name: row.phase_name.clone(),
            pool_identifier: row.pool_identifier.clone(),
            winner_placement: row.winner_placement,
            loser_placement: row.loser_placement,
            location: location.clone(),
            num_entrants: row.num_entrants,
            event_handle: row.event_handle.clone(),
        };
        if row.winner_player_id == path.player_id {
            wins.push(rec(row.loser_player_id, row.loser_name));
        } else {
            losses.push(rec(row.winner_player_id, row.winner_name));
        }
    }

    wins.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));
    losses.sort_by(|a, b| b.upset_factor.cmp(&a.upset_factor));

    Ok(Json(PlayerStatsResponse {
        player_id: path.player_id,
        name,
        wins,
        losses,
    }))
}
```

- [ ] **Step 4: Update get_head_to_head to read from ranking_set_results**

Replace the entire `get_head_to_head` handler:

```rust
pub async fn get_head_to_head(
    State(state): State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(path): Path<RankingPath>,
) -> Result<impl IntoResponse> {
    require_ranking_read_access(&state.db, path.id, path.rid, user.map(|u| u.id)).await?;

    struct H2HRow {
        winner_player_id: Uuid,
        loser_player_id: Uuid,
        count: i64,
    }

    let rows = sqlx::query_as!(
        H2HRow,
        r#"
        SELECT
            winner_player_id AS "winner_player_id!: Uuid",
            loser_player_id  AS "loser_player_id!: Uuid",
            COUNT(*)         AS "count!: i64"
        FROM ranking_set_results
        WHERE ranking_id = $1
        GROUP BY winner_player_id, loser_player_id
        "#,
        path.rid,
    )
    .fetch_all(&state.db)
    .await?;

    let mut wins: HashMap<(Uuid, Uuid), i64> = HashMap::new();
    let mut pairs: std::collections::HashSet<(Uuid, Uuid)> = std::collections::HashSet::new();

    for row in rows {
        wins.insert((row.winner_player_id, row.loser_player_id), row.count);
        let pair = if row.winner_player_id < row.loser_player_id {
            (row.winner_player_id, row.loser_player_id)
        } else {
            (row.loser_player_id, row.winner_player_id)
        };
        pairs.insert(pair);
    }

    let mut resp: Vec<HeadToHeadEntry> = Vec::with_capacity(pairs.len() * 2);
    for (a, b) in pairs {
        let a_wins = *wins.get(&(a, b)).unwrap_or(&0);
        let b_wins = *wins.get(&(b, a)).unwrap_or(&0);
        resp.push(HeadToHeadEntry { player_id: a, opponent_id: b, wins: a_wins, losses: b_wins });
        resp.push(HeadToHeadEntry { player_id: b, opponent_id: a, wins: b_wins, losses: a_wins });
    }

    resp.sort_by(|x, y| x.player_id.cmp(&y.player_id).then(x.opponent_id.cmp(&y.opponent_id)));
    Ok(Json(resp))
}
```

- [ ] **Step 5: Update get_h2h_sets to read from ranking_set_results**

Replace the `get_h2h_sets` handler's query to start from `ranking_set_results`:

```rust
    let rows = sqlx::query_as!(
        H2HSetRow,
        r#"
        SELECT
            rsr.winner_player_id                AS "winner_player_id!: Uuid",
            wp.name                             AS "winner_name!",
            we.seed                             AS winner_seed,
            rsr.loser_player_id                 AS "loser_player_id!: Uuid",
            lp.name                             AS "loser_name!",
            le.seed                             AS loser_seed,
            s.winner_score,
            s.loser_score,
            e.name                              AS event_name,
            t.name                              AS tournament_name,
            t.handle                            AS tournament_handle,
            s.round_name,
            rsr.completed_at,
            s.is_dq,
            s.vod_url,
            s.startgg_set_id,
            ph.name                             AS "phase_name?: String",
            pg.display_identifier               AS "pool_identifier?: String",
            we.final_placement                  AS winner_placement,
            le.final_placement                  AS loser_placement,
            e.num_entrants,
            t.online,
            t.city,
            t.addr_state,
            t.country_code,
            e.handle                            AS "event_handle?: String"
        FROM ranking_set_results rsr
        JOIN sets s  ON s.id  = rsr.set_id
        JOIN players wp ON wp.id = rsr.winner_player_id
        JOIN players lp ON lp.id = rsr.loser_player_id
        JOIN entrants we ON we.id = s.winner_entrant_id
        JOIN entrants le ON le.id = s.loser_entrant_id
        JOIN events e   ON e.id  = rsr.event_id
        JOIN tournaments t ON t.id = e.tournament_id
        LEFT JOIN phase_groups pg ON pg.id = s.phase_group_id
        LEFT JOIN phases ph       ON ph.id = pg.phase_id
        WHERE rsr.ranking_id = $1
          AND (
              (rsr.winner_player_id = $2 AND rsr.loser_player_id = $3)
           OR (rsr.winner_player_id = $3 AND rsr.loser_player_id = $2)
          )
        ORDER BY rsr.completed_at DESC NULLS LAST
        "#,
        path.rid,
        path.pid_a,
        path.pid_b,
    )
```

The rest of the `get_h2h_sets` handler (building `H2HSet` vec) stays the same.

- [ ] **Step 6: Update router to use put_events instead of patch_event**

In `tournaments.rs` router(), change the events route:
```rust
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tournaments", get(list_tournaments))
        .route("/events", axum::routing::put(put_events))
        .route("/stats", get(get_stats))
        .route("/stats/{player_id}", get(get_player_stats))
        .route("/head-to-head", get(get_head_to_head))
        .route("/head-to-head/{pid_a}/{pid_b}/sets", get(get_h2h_sets))
}
```

Also remove `use axum::routing::patch;` from the imports if it becomes unused; add `common` to the imports:
```rust
use common::{jobs, models::UserRole, upset::set_upset_factor};
```

- [ ] **Step 7: Verify API compiles**

```bash
cd backend && cargo check -p api 2>&1 | head -30
```
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add backend/crates/api/src/routes/tournaments.rs
git commit -m "feat(api): replace patch_event with bulk put_events; stats/H2H read from ranking_set_results"
```

---

## Task 9: Frontend — Batch Save/Discard UI

Update the tournaments page to accumulate local changes and submit them in one bulk PUT instead of one PATCH per toggle.

**Files:**
- Modify: `web/src/routes/projects/[id]/rankings/[rid]/tournaments/+page.svelte`

- [ ] **Step 1: Write the test — confirm the bulk save exists in the DOM**

In `web/src/routes/projects/[id]/rankings/[rid]/tournaments/+page.svelte`, the `Save` and `Discard` buttons should only appear when there are unsaved changes. No existing test covers this, so we verify manually in Step 6 (the e2e tests don't support auth).

- [ ] **Step 2: Add pending-changes state and savedTournaments**

In the `<script lang="ts">` block, after `let tournaments = $state(...)`:

```ts
  // Track the server's last-saved state for discard
  let savedTournaments = $state(untrack(() => [...data.tournaments]));
  $effect(() => {
    // Keep savedTournaments in sync when server data reloads (after invalidateAll)
    savedTournaments = [...data.tournaments];
    tournaments = [...data.tournaments];
  });

  const hasPendingChanges = $derived(
    tournaments.some((t, ti) =>
      t.events.some((e, ei) => e.included !== savedTournaments[ti]?.events[ei]?.included),
    ),
  );
```

- [ ] **Step 3: Remove toggleEvent API call; make handleToggle local-only**

Remove the `toggleEvent` async function entirely. Replace `handleToggle` with a local state update only:

```ts
  function handleToggle(event: TournamentEvent) {
    tournaments = tournaments.map((t) => ({
      ...t,
      events: t.events.map((e) =>
        e.id === event.id ? { ...e, included: !e.included } : e,
      ),
    }));
  }
```

Remove the `async` from `bulkSetIncluded` and remove the `toggleEvent` call at the end — it should only update local state:

```ts
  function bulkSetIncluded(included: boolean) {
    const toChange = visibleTournaments.flatMap((t) => t.events).filter((e) => e.included !== included);
    if (toChange.length === 0) return;
    const idSet = new Set(toChange.map((e) => e.id));
    tournaments = tournaments.map((t) => ({
      ...t,
      events: t.events.map((e) => (idSet.has(e.id) ? { ...e, included } : e)),
    }));
  }
```

- [ ] **Step 4: Add saveChanges and discardChanges functions**

```ts
  let saving = $state(false);

  async function saveChanges() {
    saving = true;
    const api = makeApi(fetch);

    // Collect all events whose included state differs from savedTournaments
    const changes: { event_id: string; included: boolean }[] = [];
    for (const t of tournaments) {
      for (const e of t.events) {
        const saved = savedTournaments
          .flatMap((st) => st.events)
          .find((se) => se.id === e.id);
        if (saved && saved.included !== e.included) {
          changes.push({ event_id: e.id, included: e.included });
        }
      }
    }

    if (changes.length === 0) {
      saving = false;
      return;
    }

    const res = await api.put(
      `/projects/${data.project.id}/rankings/${data.ranking.id}/events`,
      changes,
    );
    saving = false;
    if (res.ok) {
      // Persist the new state as saved
      savedTournaments = tournaments.map((t) => ({ ...t, events: [...t.events] }));
    }
  }

  function discardChanges() {
    tournaments = savedTournaments.map((t) => ({ ...t, events: [...t.events] }));
  }
```

- [ ] **Step 5: Update the Checkbox onCheckedChange call and add save/discard buttons**

In the tournament event list, update the Checkbox handler to use the simplified `handleToggle`:
```svelte
                    <Checkbox
                      checked={event.included}
                      onCheckedChange={() => handleToggle(event)}
                    />
```

Add a save/discard bar above the tournament list, below the filter panel:

```svelte
    {#if canEdit && hasPendingChanges}
      <div class="flex items-center justify-between rounded-md border border-amber-500/40 bg-amber-950/20 px-4 py-2">
        <span class="text-sm text-amber-400">You have unsaved changes to event inclusion.</span>
        <div class="flex gap-2">
          <Button variant="outline" size="sm" onclick={discardChanges}>Discard</Button>
          <Button size="sm" onclick={saveChanges} disabled={saving}>
            {saving ? "Saving…" : "Save"}
          </Button>
        </div>
      </div>
    {/if}
```

- [ ] **Step 6: Format and verify**

```bash
cd web && npm run format && npm run check 2>&1 | tail -10
```
Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add web/src/routes/projects/[id]/rankings/[rid]/tournaments/+page.svelte
git commit -m "feat(web): batch event inclusion save/discard — local accumulation, single PUT on save"
```

---

## Task 10: Update API Key Import

The `api.ts` frontend client needs a `put` method if it doesn't already have one.

**Files:**
- Modify: `web/src/lib/api.ts` (if needed)

- [ ] **Step 1: Check if put method exists**

```bash
grep -n "put(" web/src/lib/api.ts | head -5
```

- [ ] **Step 2: Add put if missing**

If `api.ts` only has `get`, `post`, `patch`, `delete`, add:

```ts
  put: (path: string, body?: unknown) =>
    fetch(`${baseUrl}${path}`, {
      method: 'PUT',
      credentials: 'include',
      headers: body !== undefined ? { 'Content-Type': 'application/json' } : {},
      body: body !== undefined ? JSON.stringify(body) : undefined,
    }),
```

- [ ] **Step 3: Commit if changed**

```bash
git add web/src/lib/api.ts
git commit -m "feat(web): add put method to API client"
```

---

## Task 11: Update openapi.yaml and Documentation

**Files:**
- Modify: `backend/openapi.yaml`
- Modify: `docs/DESIGN.md`
- Modify: `docs/routes.md`

- [ ] **Step 1: Update openapi.yaml**

In `backend/openapi.yaml`, make the following changes:

**Schemas — extend `Ranking`:**
Add `algorithm`, `algorithm_config`, `include_external_results`, `result_sort` to the `Ranking` schema object. Add `RankingPlayerScore` schema with `ranking_id`, `player_id`, `computed_rating`, `display_data`, `algorithm_state`, `computed_at`.

**Paths — remove `PATCH /projects/{id}/rankings/{rid}/events/{eid}`** (deleted endpoint).

**Paths — add `PUT /projects/{id}/rankings/{rid}/events`:**
```yaml
    put:
      summary: Bulk set event inclusion
      tags: [Rankings]
      security: [{cookieAuth: []}]
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: array
              items:
                type: object
                required: [event_id, included]
                properties:
                  event_id: {type: string, format: uuid}
                  included: {type: boolean}
      responses:
        '202': {description: Accepted — compute_ranking job enqueued}
        '401': {description: Unauthorized}
        '403': {description: Forbidden}
```

**Paths — add `POST /projects/{id}/rankings/{rid}/recompute`:**
```yaml
    post:
      summary: Manually trigger ranking recompute
      tags: [Rankings]
      security: [{cookieAuth: []}]
      responses:
        '202': {description: Accepted}
        '401': {description: Unauthorized}
        '403': {description: Forbidden}
```

**Paths — add `GET /projects/{id}/rankings/{rid}/ranking`:**
```yaml
    get:
      summary: Get players in computed ranking order
      tags: [Rankings]
      responses:
        '200':
          description: Players ordered by computed_rating (algorithmic) or rank_position (manual)
          content:
            application/json:
              schema:
                type: array
                items: {$ref: '#/components/schemas/ComputedRankingPlayer'}
```

- [ ] **Step 2: Update DESIGN.md**

In `docs/DESIGN.md`, update the data model section to include `ranking_set_results` and `ranking_player_scores`, describe the `compute_ranking` job flow, and note the batch event save change.

- [ ] **Step 3: Update routes.md**

In `docs/routes.md`, note that the tournaments tab now uses a save button instead of immediate per-toggle updates. Add the `recompute` endpoint and `GET /ranking` endpoint.

- [ ] **Step 4: Commit**

```bash
git add backend/openapi.yaml docs/DESIGN.md docs/routes.md
git commit -m "docs: update openapi.yaml, DESIGN.md, routes.md for ranking algorithm features"
```

---

## Task 12: Regenerate sqlx Cache and Run Tests

**Files:**
- Modify: `backend/.sqlx/` (regenerated)

- [ ] **Step 1: Run prepare-sqlx.sh**

```bash
cd backend && bash prepare-sqlx.sh
```

Expected: runs migrations on a fresh container, then `cargo sqlx prepare --workspace -- --all-targets`. If any query fails to compile, fix the column mismatch before proceeding. Common issues: queries still selecting the old `rankings` column list without `algorithm` etc., or queries in `e2e` tests referencing old endpoints.

- [ ] **Step 2: Run the full backend test suite**

```bash
bash backend/test.sh --verbose 2>&1 | tail -40
```

Expected: all tests pass. If any test fails due to the changed `rankings` column list, update the query in that test to include the new columns.

- [ ] **Step 3: Run frontend unit tests**

```bash
cd web && npm run test:unit 2>&1 | tail -10
```

Expected: all pass.

- [ ] **Step 4: Run frontend e2e tests**

```bash
cd web && npm run test:e2e 2>&1 | tail -20
```

Expected: all pass. The e2e mock API at port 9999 may need updating if any mock returns the old ranking shape without `algorithm`/`result_sort` fields. If tests fail, update the mock response in `web/tests/` to include the new fields with their defaults.

- [ ] **Step 5: Commit updated .sqlx cache**

```bash
git add backend/.sqlx/
git commit -m "chore: regenerate sqlx offline query cache for ranking algorithm features"
```

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Covered by |
|---|---|
| Manual/algorithmic rankings coexist | Task 7 (algorithm column in rankings) |
| Elo and Glicko-2 algorithms | Task 3 (elo.rs, glicko2.rs) |
| Algorithm trait for extensibility | Task 3 (mod.rs, AlgorithmRegistry) |
| ranking_player_scores table | Task 1 (migration) + Task 5 (compute.rs Phase 2) |
| ranking_set_results table | Task 1 (migration) + Task 5 (compute.rs Phase 1) |
| compute_ranking job kind | Task 1 (migration enum) + Task 4 (jobs.rs) |
| Trigger compute after import | Task 6 (import.rs) |
| Trigger compute after bulk event save | Task 8 (put_events) |
| Trigger compute after player add/remove | Task 7 (add_ranking_player, remove_ranking_player) |
| Manual recompute endpoint | Task 7 (recompute_ranking) |
| Batch event save with save/discard | Task 9 (frontend) + Task 8 (put_events API) |
| Stats read from ranking_set_results | Task 8 (get_stats, get_player_stats) |
| H2H read from ranking_set_results | Task 8 (get_head_to_head, get_h2h_sets) |
| Algorithmic ranking order by computed_rating | Task 7 (get_computed_ranking) |
| result_sort field on ranking | Task 7 (patch/create handlers) |
| include_external_results field | Task 7 (create/patch) — computation stub wired (Phase 2 passes None for external ratings; full impl requires Sub-project B) |
| openapi.yaml updated | Task 11 |
| DESIGN.md, routes.md updated | Task 11 |
| Merge migration (user approved) | Task 1 |

**Placeholder scan:** No TBD or TODO sections found. All code blocks are complete.

**Type consistency:**
- `ScoredSet`, `PlayerScore`, `AlgorithmError`, `RankingAlgorithm`, `AlgorithmRegistry` defined in Task 3 and referenced consistently in Task 5 (`compute.rs`).
- `ComputeRankingParams` defined in Task 4 and consumed in Task 6 (`main.rs`).
- `enqueue_compute_ranking(pool, project_id, ranking_id)` signature used consistently in Tasks 6, 7, 8.
- `Ranking` struct new fields used consistently across `require_ranking_access`, `require_ranking_read_access`, `list_rankings`, `create_ranking`, `patch_ranking`.
- `put_events` replaces `patch_event` in both the handler and the router.
- `handleToggle` in Task 9 removed the third argument (`event: TournamentEvent`) — ensure the Checkbox call site passes `event` not `data.project.id, data.ranking.id, event`.

**One noted issue to verify:** In Task 9, `handleToggle` is simplified to take only `event`. The call site at line 699 of the original file was `handleToggle(data.project.id, data.ranking.id, event)` — update this to `handleToggle(event)`.

---

Plan complete and saved to `docs/superpowers/plans/2026-06-13-ranking-algorithms.md`.

**Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
