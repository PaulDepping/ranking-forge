# Design: Ranking Algorithms & Global Start.gg Mirror

**Date:** 2026-06-13
**Status:** Approved — awaiting implementation plan

---

## Overview

Two sub-projects that are independent but designed to connect:

- **Sub-project A:** Algorithmic ranking modes (Elo, Glicko-2) for existing local rankings
- **Sub-project B:** Platform-wide start.gg mirror with global player ratings (separate phase, higher complexity)

Sub-project A is fully specified and ready to implement. Sub-project B is specified at architectural depth — enough to start implementation later without re-deriving decisions.

---

## Sub-project A: Ranking Algorithms

### Goals

- Rankings can be manual (existing behavior) or algorithmic (Elo, Glicko-2).
- Multiple rankings per project can coexist: some manual, some algorithmic, some with different algorithms.
- Algorithmic rankings are ordered by computed rating, not by manually set `rank_position`.
- Adding a new algorithm in future requires zero schema changes.
- Calculation state, display state, and algorithm configuration are kept strictly separate.
- Stats views (wins/losses lists) can optionally be sorted by global player rating instead of upset factor — applies to both manual and algorithmic rankings, falls back to upset factor if global data is unavailable.
- Event inclusion changes on the tournaments page are batched: the user accumulates toggles locally and submits them all at once via a save button, triggering a single recompute instead of one per toggle.
- Stats (wins/losses lists) and H2H set records are pre-computed and stored when ranking data changes, not recalculated at runtime on every page load.

### Schema Changes

#### `rankings` table — new columns

```sql
ALTER TABLE rankings
  ADD COLUMN algorithm            TEXT,        -- NULL = manual, 'elo', 'glicko2', ...
  ADD COLUMN algorithm_config     JSONB NOT NULL DEFAULT '{}',
  ADD COLUMN include_external_results BOOLEAN NOT NULL DEFAULT FALSE,
  ADD COLUMN result_sort          TEXT  NOT NULL DEFAULT 'upset_factor';
```

- `algorithm`: NULL means manual (existing behaviour unchanged). Non-null values are algorithm identifiers registered in the algorithm registry.
- `algorithm_config`: Per-algorithm tuning parameters. Examples:
  - Elo: `{"k_factor": 32}`
  - Glicko-2: `{"tau": 0.5, "initial_rd": 350.0, "initial_volatility": 0.06}`
- `include_external_results`: When true and Sub-project B is running, sets against players not in the ranking are included in computation using their global rating as a starting seed. Silently ignored (treated as false) until global data exists.
- `result_sort`: `'upset_factor'` (default, existing behaviour) or `'global_rating'`. Controls how individual wins/losses are ordered in the stats view. When `'global_rating'`, opponent ratings are looked up from `global_player_ratings` via `startgg_user_id`; rows without a global rating fall to the bottom.

#### New table: `ranking_player_scores`

```sql
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
```

Three concerns kept strictly separate:

| Field | Purpose | Who reads it |
|---|---|---|
| `computed_rating` | Uniform ordering key — one float, all algorithms | SQL `ORDER BY`, API ranking endpoint |
| `display_data` | What to render in the UI | Frontend only |
| `algorithm_state` | Internal state for incremental recomputation | Worker only |

`ranking_players.rank_position` remains the source of truth for manual rankings and is not written for algorithmic rankings.

#### New table: `ranking_set_results`

Pre-computed per-ranking set list. Populated by the `compute_ranking` job for **all** rankings (manual and algorithmic). The stats and H2H endpoints read directly from this table rather than re-joining the full set graph at runtime.

```sql
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
```

This table contains only sets where both the winner and loser are members of `ranking_players` and the event is included (`ranking_events.included = true`). DQ sets are excluded. Upset factor is computed once at write time using the existing algorithm.

H2H counts are derived from this table at query time (`GROUP BY winner_player_id, loser_player_id, COUNT(*)`) — no separate H2H table is needed since the GROUP BY is cheap on the pre-filtered set.

Example values per algorithm:

**Elo**
- `computed_rating`: `1543.0`
- `display_data`: `{"rating": 1543}`
- `algorithm_state`: `{}` (stateless between periods — rating is the state)

**Glicko-2**
- `computed_rating`: `1487.0` (the converted public rating `r = 173.7178 * μ + 1500`)
- `display_data`: `{"rating": 1487, "rd": 45}` (converted `RD = 173.7178 * φ`)
- `algorithm_state`: `{"mu": -0.075, "phi": 0.259, "sigma": 0.06}` (raw internal scale)

### Algorithm Architecture (Rust)

Lives in `common`. A trait enforces the uniform contract:

```rust
pub trait RankingAlgorithm: Send + Sync {
    fn name(&self) -> &'static str;
    fn compute(
        &self,
        config: &serde_json::Value,
        sets: &[ScoredSet],
    ) -> Result<Vec<PlayerScore>, AlgorithmError>;
}

pub struct ScoredSet {
    pub winner_id: Uuid,
    pub loser_id: Uuid,
    pub completed_at: DateTime<Utc>,
    // Populated when include_external_results = true and global data exists.
    // None means external player with no global rating — set may be skipped or
    // treated as neutral depending on algorithm implementation.
    pub winner_global_rating: Option<f64>,
    pub loser_global_rating: Option<f64>,
    pub is_external_winner: bool,
    pub is_external_loser: bool,
}

pub struct PlayerScore {
    pub player_id: Uuid,
    pub computed_rating: f64,      // ordering key
    pub display_data: serde_json::Value,
    pub algorithm_state: serde_json::Value,
}
```

A `AlgorithmRegistry` in `common` maps name strings to `Box<dyn RankingAlgorithm>`. The worker calls `registry.get(algorithm_name)?.compute(config, sets)` — no algorithm-specific logic leaks into worker or api code. Sets are passed in `completed_at` ascending order; algorithms may process them however they require.

#### Implementing Elo

Standard Elo with configurable K-factor. Each set is processed in order; both players start at `config.initial_rating` (default 1500) if no prior score exists. `computed_rating` = current rating. `algorithm_state` is empty (rating itself is the state).

#### Implementing Glicko-2

Follows Glishman's Glicko-2 algorithm. Players start at `μ=0, φ=2.014523 (RD=350), σ=config.initial_volatility`. Rating periods are defined by the set of all included events. `algorithm_state` stores `{mu, phi, sigma}` in the internal scale; `display_data` stores converted `{rating, rd}` for the UI. `computed_rating` = converted rating `r`.

### Job System

New `job_kind` variant: `compute_ranking`. This applies to **all** rankings — manual and algorithmic — since every ranking now has pre-computed set results to maintain.

The `compute_ranking` job is enqueued automatically by the API when:
- An `import_tournaments` job completes for a project (one job per ranking in the project)
- A bulk event inclusion save is submitted (see Batch Event Save below)
- A player is added to or removed from a ranking
- `include_external_results` is toggled on an algorithmic ranking

The API also exposes a manual trigger (editor/owner only).

The worker processes a `compute_ranking` job in two phases:

**Phase 1 — set results (all rankings):**
1. Load all sets from included events where both the winner and loser are ranking members, in `completed_at` ascending order. Exclude DQ sets.
2. Compute upset factor for each set using the existing algorithm.
3. Wipe and rewrite `ranking_set_results` for the ranking in one transaction.

**Phase 2 — algorithm scores (algorithmic rankings only):**
4. If `include_external_results = true`, join `global_player_ratings` via `startgg_user_id` for each external opponent in the included event set. Populate `winner_global_rating` / `loser_global_rating` on `ScoredSet` where available.
5. Call `registry.get(algorithm)?.compute(config, sets)`.
6. Wipe and rewrite `ranking_player_scores` for the ranking in one transaction.

### Batch Event Save

The existing `PATCH /projects/:id/rankings/:rid/events/:eid` endpoint (single-event toggle) is **replaced** by a bulk endpoint:

`PUT /projects/:id/rankings/:rid/events` — accepts an array of `{event_id, included}` pairs representing the full desired inclusion state. Writes all changes atomically, then enqueues a single `compute_ranking` job. Returns 202 Accepted.

The frontend accumulates inclusion toggles locally on the tournaments page without making any API calls. A save button submits the full changed state in one request. A discard button reverts local changes to the last saved state. This prevents multiple rapid recompute jobs from queuing up when the user is making bulk adjustments.

### API Changes

| Method | Path | Change |
|---|---|---|
| POST | `/projects/:id/rankings` | Accept `algorithm`, `algorithm_config`, `include_external_results`, `result_sort` |
| PATCH | `/projects/:id/rankings/:rid` | Accept same fields |
| POST | `/projects/:id/rankings/:rid/recompute` | New — enqueues `compute_ranking` job; 202 Accepted |
| PUT | `/projects/:id/rankings/:rid/events` | New — bulk event inclusion save; replaces per-event PATCH |
| GET | `/projects/:id/rankings/:rid/ranking` | For algorithmic rankings: ordered by `computed_rating DESC`; response includes `display_data` per player |
| GET | `/projects/:id/rankings/:rid/stats` | Reads from `ranking_set_results`; `result_sort` applied server-side; includes `display_data` when algorithmic |
| GET | `/projects/:id/rankings/:rid/stats/:pid` | Same |
| GET | `/projects/:id/rankings/:rid/head-to-head` | Reads from `ranking_set_results` (GROUP BY pair); no runtime set join |

The ranking list response (`GET /projects/:id/rankings`) includes `algorithm` and `result_sort` so the frontend can render algorithm labels and sort indicators.

### Unchanged

- `ranking_players` and its `rank_position` field — untouched for manual rankings.
- Upset factor calculation logic — unchanged; computed at write time into `ranking_set_results` rather than at read time.
- The `PATCH /projects/:id/rankings/:rid/events/:eid` endpoint is removed; all event inclusion changes go through the new bulk `PUT` endpoint.

---

## Sub-project B: Global Start.gg Mirror

### Goals

- Maintain a continuously updated mirror of all start.gg tournament data across all games.
- Compute global Elo/Glicko-2 ratings for every start.gg player.
- Provide external opponent strength lookup for Sub-project A (`include_external_results`).
- Serve as the data foundation for future per-player and per-tournament career stat pages (LuckyStats / Supermajor.gg style).
- Player identity is resolved at import time: sets store direct player FKs rather than intermediate entrant chains.

### Prior Art: HCI Scraper

A battle-tested start.gg scraper already exists at `../hci_startgg_dataset` (a separate university research project). It handles the hard parts:

- **Sliding window pagination** over date ranges to stay under start.gg's 10,000-entry API limit
- **Complexity retry**: halves `perPage` on `query complexity is too high` errors
- **Slim query fallback**: drops user profile fields and character selections when even `perPage=1` is too complex
- **Graceful shutdown**: SIGTERM/SIGINT finishes the current event then exits cleanly
- **Checkpointing**: two tables (`scraper_checkpoints`, `scraper_tournament_checkpoints`) allow resuming an interrupted run without re-fetching completed events
- **Rate limit handling**: respects `Retry-After` headers on 429, exponential backoff on network/server errors, up to 15 retries per request
- **GraphQL query set**: TOURNAMENT_QUERY → PHASE_GROUPS_QUERY → PHASE_GROUP_SETS_QUERY (full) / PHASE_GROUP_SETS_QUERY_SLIM (fallback) → EVENT_STANDINGS_QUERY

The new `crawler` binary in RankingForge should port the scraper logic from `hci_startgg_dataset/src/{scraper.rs, api.rs, cli.rs}` directly, adapting the `db.rs` layer to write to the new global schema instead of the HCI raw tables. The `gql_query` function and all retry/backoff/pagination logic is reusable as-is.

**Key adaptation:** The HCI scraper stores the full entrant → participants → player/user chain as separate tables and resolves identity at query time. The RankingForge crawler resolves identity at write time: during set processing, it extracts `participant.user.id` (the `startgg_user_id`) and upserts a `global_players` row immediately, then writes `winner_player_id` / `loser_player_id` directly onto the set. This collapses the 5-table join chain into a direct FK on every set.

### Global Schema

All global tables use UUID primary keys for consistency with the rest of the database. Start.gg native IDs are stored as `BIGINT` natural keys with `UNIQUE NOT NULL` constraints, indexed for upsert operations.

```sql
CREATE TABLE global_games (
    id          UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id  BIGINT  NOT NULL UNIQUE,
    name        TEXT    NOT NULL
);
CREATE INDEX global_games_startgg_id_idx ON global_games(startgg_id);

-- Global player registry, keyed by start.gg user ID
CREATE TABLE global_players (
    id                UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_user_id   BIGINT  NOT NULL UNIQUE,
    handle            TEXT    NOT NULL,
    display_name      TEXT,
    profile_image_url TEXT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_players_startgg_user_id_idx ON global_players(startgg_user_id);

-- Tournament mirror
CREATE TABLE global_tournaments (
    id            UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT  NOT NULL UNIQUE,
    name          TEXT    NOT NULL,
    slug          TEXT    NOT NULL,
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
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_tournaments_start_at_idx ON global_tournaments(start_at);

-- Event mirror
CREATE TABLE global_events (
    id            UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT  NOT NULL UNIQUE,
    tournament_id UUID    NOT NULL REFERENCES global_tournaments(id) ON DELETE CASCADE,
    game_id       UUID    REFERENCES global_games(id),
    name          TEXT    NOT NULL,
    slug          TEXT,
    start_at      TIMESTAMPTZ,
    num_entrants  INTEGER,
    is_online     BOOLEAN,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_events_tournament_id_idx ON global_events(tournament_id);
CREATE INDEX global_events_game_id_start_at_idx ON global_events(game_id, start_at);

-- Bracket phases within an event (e.g. "Pools", "Top 8")
CREATE TABLE global_phases (
    id           UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id   BIGINT  NOT NULL UNIQUE,
    event_id     UUID    NOT NULL REFERENCES global_events(id) ON DELETE CASCADE,
    name         TEXT,
    phase_order  INTEGER,
    bracket_type TEXT,
    is_exhibition BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX global_phases_event_id_idx ON global_phases(event_id);

-- Individual brackets within a phase (e.g. "Pool A", "Pool B")
CREATE TABLE global_phase_groups (
    id                 UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id         BIGINT  NOT NULL UNIQUE,
    phase_id           UUID    NOT NULL REFERENCES global_phases(id) ON DELETE CASCADE,
    display_identifier TEXT,
    bracket_type       TEXT
);
CREATE INDEX global_phase_groups_phase_id_idx ON global_phase_groups(phase_id);

-- Per-player event entry: seed and final placement
CREATE TABLE global_event_entries (
    id         UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id   UUID    NOT NULL REFERENCES global_events(id) ON DELETE CASCADE,
    player_id  UUID    NOT NULL REFERENCES global_players(id) ON DELETE CASCADE,
    seed       INTEGER,
    placement  INTEGER,
    UNIQUE (event_id, player_id)
);
CREATE INDEX global_event_entries_player_id_idx ON global_event_entries(player_id);
CREATE INDEX global_event_entries_event_id_idx  ON global_event_entries(event_id);

-- Set results with direct player FKs (identity resolved at import time)
CREATE TABLE global_sets (
    id                UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id        BIGINT  NOT NULL UNIQUE,
    event_id          UUID    NOT NULL REFERENCES global_events(id) ON DELETE CASCADE,
    phase_group_id    UUID    REFERENCES global_phase_groups(id),
    winner_player_id  UUID    REFERENCES global_players(id),
    loser_player_id   UUID    REFERENCES global_players(id),
    round             INTEGER,
    round_name        TEXT,
    winner_score      SMALLINT,
    loser_score       SMALLINT,
    total_games       SMALLINT,
    is_dq             BOOLEAN NOT NULL DEFAULT FALSE,
    completed_at      TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX global_sets_event_id_idx          ON global_sets(event_id);
CREATE INDEX global_sets_phase_group_id_idx    ON global_sets(phase_group_id);
CREATE INDEX global_sets_winner_player_id_idx  ON global_sets(winner_player_id);
CREATE INDEX global_sets_loser_player_id_idx   ON global_sets(loser_player_id);
CREATE INDEX global_sets_completed_at_idx      ON global_sets(completed_at);

-- Computed global ratings per player per game
CREATE TABLE global_player_ratings (
    player_id       UUID    NOT NULL REFERENCES global_players(id) ON DELETE CASCADE,
    game_id         UUID    NOT NULL REFERENCES global_games(id),
    computed_rating FLOAT   NOT NULL,
    display_data    JSONB   NOT NULL DEFAULT '{}',
    algorithm_state JSONB   NOT NULL DEFAULT '{}',
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (player_id, game_id)
);
CREATE INDEX global_player_ratings_game_id_idx  ON global_player_ratings(game_id);

-- Crawler resumption state (one row per named checkpoint)
CREATE TABLE crawler_checkpoints (
    key        TEXT        PRIMARY KEY,
    value      JSONB       NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### Player Identity Resolution

Start.gg's data model: a `User` (account) has one `Player` (competitive profile). Each entrant in a bracket links through `participants → player → user`. RankingForge uses `user.id` as the canonical player identifier (consistent with `startgg_accounts.startgg_user_id` in the project-scoped schema).

At crawl time, for each set slot the crawler:
1. Extracts `slot.entrant.participants[0].user.id` as the `startgg_user_id`
2. Extracts `slot.entrant.participants[0].player.gamerTag` and `user.name` for display
3. Upserts a `global_players` row
4. Uses the resulting `global_players.id` as `winner_player_id` / `loser_player_id` on the set

Entrants without a resolvable user ID (TBD slots, placeholder entrants) are stored with NULL player FKs.

### Tournament Run Reconstruction

A player's sequential tournament run can be reconstructed for any event:

```sql
SELECT
    gph.name           AS phase_name,
    gph.phase_order,
    gpg.display_identifier AS pool,
    gs.round,
    gs.round_name,
    gs.winner_player_id,
    gs.loser_player_id,
    gs.winner_score,
    gs.loser_score,
    gs.completed_at
FROM global_sets gs
JOIN global_phase_groups gpg ON gpg.id = gs.phase_group_id
JOIN global_phases gph       ON gph.id = gpg.phase_id
WHERE (gs.winner_player_id = $player_id OR gs.loser_player_id = $player_id)
  AND gph.event_id = $event_id
ORDER BY gph.phase_order, gs.round, gs.completed_at;
```

`phase_order` establishes phase sequence (pools → bracket → top 8). `round` (signed integer: positive = winners side, negative = losers side) establishes set order within a phase group. `completed_at` is a tiebreaker when present.

### Crawler Service Design

A new `crawler` binary in the RankingForge Rust workspace, ported from `hci_startgg_dataset`. Core logic from `hci_startgg_dataset/src/{api.rs, scraper.rs, cli.rs}` transfers directly:

- `gql_query` function (retry, backoff, rate limit, complexity error detection) — **port verbatim**
- Sliding window date loop — **port verbatim**
- Tournament/event checkpoint logic — **port with adapted DB queries**
- Complexity retry and slim-query fallback — **port verbatim**
- Graceful shutdown (SIGTERM/SIGINT) — **port verbatim**

Only `db.rs` changes substantially: instead of writing to the HCI raw schema (entrants, participants, set_slots, games, etc.), the crawler writes to the global schema above, resolving player identity at write time.

#### GraphQL queries adapted for the crawler

The TOURNAMENT_QUERY and PHASE_GROUPS_QUERY transfer unchanged. PHASE_GROUP_SETS_QUERY is adapted:
- Keep: `id`, `state`, `winnerId`, `totalGames`, `completedAt`, `fullRoundText`, `round`, `lPlacement`, `wPlacement`, `displayScore`, `phaseGroup { id, displayIdentifier, bracketType, phase { id, name, bracketType, phaseOrder, isExhibition } }`, `slots { standing { placement, stats { score { value } } }, entrant { id, name, initialSeedNum, isDisqualified, participants { player { id, gamerTag, prefix }, user { id, name } } } }`
- Remove: `games { ... }` (game-level data not stored), `slots.entrant.participants.user.{ bio, discriminator, genderPronoun, location }` (noise), `selections { ... }` (character data deferred)

A new slim variant omits the `user { ... }` block inside participants as fallback for complexity-heavy events.

#### Incremental operation

The crawler runs continuously (or on a schedule): it walks forward through time using the sliding window approach, processing only completed events. The `crawler_checkpoints` table stores named keys (e.g. `last_window_start`, `tournament:{id}`, `event:{id}`) so any restart resumes from where it left off without re-fetching already-processed events.

Historical backfill: the crawler starts from a configurable date. For a full start.gg backfill (all games, all time), rate limits (~80 req/min per key) make this a multi-month operation. A bulk historical seed (community data dumps or a negotiated start.gg export) can be loaded to bootstrap the dataset; the crawler then switches to incremental mode.

#### Global rating computation

A separate `compute_global_ratings` job (or a post-crawl phase) processes all `global_sets` for a given game in `completed_at` ascending order and runs the same `RankingAlgorithm` trait used by Sub-project A. Results are upserted into `global_player_ratings`. This job is game-scoped and can run in parallel per game.

### Connection to Sub-project A

The global data connects to Sub-project A through two paths:

**1. External opponent strength (`include_external_results = true`)**

When computing a local ranking, the worker joins `global_player_ratings` via `entrants.startgg_user_id → global_players.startgg_user_id → global_player_ratings` for any opponent not in `ranking_players`. The matched rating populates `ScoredSet.winner_global_rating` / `loser_global_rating`. Sub-project A's compute path is already wired for this — it just receives NULLs until global data exists.

**2. Stats sort by global rating (`result_sort = 'global_rating'`)**

The stats endpoint joins `global_player_ratings` per opponent via `startgg_user_id` to sort wins/losses lists by opponent global rating. Opponents without a global rating sort last. Falls back to upset factor order if no global data is available at all.

### Deferred: Per-player and Per-tournament Pages

The global schema supports LuckyStats / Supermajor.gg style pages without additional schema changes:

- **Per-player career page**: query `global_event_entries` for all events the player entered (seed, placement, tournament name, date, game) + `global_sets` for all their set results across events.
- **Per-tournament page**: query `global_events` for the event list, `global_phases` / `global_phase_groups` for bracket structure, `global_sets` for full bracket results, `global_event_entries` for final standings.

These are API and frontend additions only — no schema work required.

**Character data** (LuckyStats shows character usage per set) is explicitly deferred. It would require storing `game_selections` in the crawler, a schema migration (`global_game_selections`), and a re-crawl of sets. The design leaves room for this addition without conflicts.

---

## Decisions Not Made (Explicitly Deferred)

| Decision | When to make it |
|---|---|
| Which default K-factor / Glicko-2 τ to ship | During Sub-project A implementation; expose as config |
| Full historical backfill strategy (community dump vs. slow crawl) | During Sub-project B implementation |
| Per-player / per-tournament frontend pages | Separate feature spec when Sub-project B is live |
| Character data ingestion | Separate feature spec after career pages ship |
| Multiple start.gg API keys for higher crawl throughput | During Sub-project B implementation |
