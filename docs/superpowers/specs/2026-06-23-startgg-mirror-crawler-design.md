# Design: Start.gg Global Mirror — Crawler

**Date:** 2026-06-23
**Status:** Approved — awaiting implementation plan
**Predecessor spec:** `2026-06-13-ranking-algorithms-global-mirror-design.md` (Sub-project B)

---

## Overview

A new `crawler` binary that continuously mirrors start.gg tournament data into a set of `global_*` tables in the shared Postgres database. The crawler handles all the hard parts of the start.gg API (rate limits, complexity errors, pagination, checkpointing) and resolves player identity at write time so every set has direct player foreign keys.

This is the data-collection phase. Global rating computation (`global_player_ratings`) is a separate follow-up. The global schema is defined now (including `global_player_ratings`) so the schema is stable when rating computation is added.

### Strategic intent

Once the global mirror is populated, tournament imports for local rankings shift from live start.gg API calls to queries against the global tables. This eliminates the per-user API key requirement for imports. The transition from live-fetch to mirror-query is a separate step after the mirror has sufficient coverage.

---

## Schema

Global tables are added directly to `backend/migrations/001_initial.sql` (no production database exists, so a new migration file is not needed).

```sql
-- Video games
CREATE TABLE global_games (
    id         UUID   PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id BIGINT NOT NULL UNIQUE,
    name       TEXT   NOT NULL
);
CREATE INDEX global_games_startgg_id_idx ON global_games(startgg_id);

-- Global player registry, keyed by start.gg user ID.
-- startgg_player_id is a fallback key populated when the slim query is used and
-- user.id is unavailable. In practice user ↔ player is 1:1, so a later full-query
-- pass will merge the row via startgg_user_id.
-- profile_image_url is fetched from user.images in the full query; NULL otherwise.
-- Frontend loads images directly from the start.gg CDN URL — no hosting required.
-- The COALESCE upsert pattern ensures previously-stored values are never overwritten
-- by NULLs from a slim-query pass.
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

-- Tournament mirror
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

-- Event mirror
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

-- Bracket phases within an event (e.g. "Pools", "Top 8")
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

-- Individual brackets within a phase (e.g. "Pool A", "Pool B")
CREATE TABLE global_phase_groups (
    id                 UUID   PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id         BIGINT NOT NULL UNIQUE,
    phase_id           UUID   NOT NULL REFERENCES global_phases(id) ON DELETE CASCADE,
    display_identifier TEXT,
    bracket_type       TEXT
);
CREATE INDEX global_phase_groups_phase_id_idx   ON global_phase_groups(phase_id);
CREATE INDEX global_phase_groups_startgg_id_idx ON global_phase_groups(startgg_id);

-- Per-player event entry: seed and final placement
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

-- Set results with direct player FKs (identity resolved at import time)
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
    -- total_games omitted: derivable from winner_score + loser_score
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

-- Individual games within a set (game 1, game 2, ...)
-- stage_id / stage_name come directly from game.stage in the API response.
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

-- Character/stage selections per game per player.
-- character_id / character_name come from GameSelection.character { id, name } inline —
-- no separate character lookup table is needed.
-- selection_type is 'CHARACTER' or 'STAGE'.
CREATE TABLE global_game_selections (
    game_id        UUID   NOT NULL REFERENCES global_set_games(id) ON DELETE CASCADE,
    player_id      UUID   REFERENCES global_players(id),
    selection_type TEXT   NOT NULL,
    character_id   BIGINT,
    character_name TEXT,
    PRIMARY KEY (game_id, player_id, selection_type)
);
CREATE INDEX global_game_selections_player_id_idx ON global_game_selections(player_id);

-- Computed global ratings per player per game (schema defined now; populated in a later phase)
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

-- Crawler resumption state (one row per named checkpoint key)
CREATE TABLE crawler_checkpoints (
    key        TEXT        PRIMARY KEY,
    value      JSONB       NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

Checkpoint keys:
- `"window_start"` — last completed window boundary (unix timestamp), used to resume after restart
- `"tournament:{startgg_id}"` — marks a tournament fully processed
- `"event:{startgg_id}"` — marks an event fully processed

---

## Crawler Binary

### Location

New crate `backend/crates/crawler/` in the Rust workspace. Registered in `backend/Cargo.toml` as a workspace member.

### Port from HCI scraper

`hci_startgg_dataset/src/{api.rs, cli.rs, scraper.rs}` are the source. The following logic transfers without meaningful change:

| Component | Transfer strategy |
|---|---|
| `gql_query` function | Port verbatim — handles retry, exponential backoff, rate-limit `Retry-After`, up to 15 retries |
| Complexity error detection | Port verbatim |
| Sliding window date loop | Port verbatim |
| `perPage`-halving on complexity error | Port verbatim |
| Graceful shutdown (`AtomicBool` + tokio signal) | Port verbatim |
| Tournament/event checkpoint check | Adapt to `crawler_checkpoints` table |
| Tournament/event checkpoint write | Adapt to `crawler_checkpoints` table |
| CLI arguments (`from_date`, `to_date`, `window_days`, `delay`, `sets_per_page`, `game_id`) | Port; drop `tournaments_per_page` dynamic resize (keep static CLI arg) |
| Slim query fallback | **Replace** with two-pass fallback (see below) |

`db.rs` is rewritten from scratch: instead of writing to the HCI raw schema (entrants, participants, set_slots, games), the crawler writes to the global schema with identity resolved at write time.

### GraphQL queries

**TOURNAMENT_QUERY** — port unchanged. Fetches `lat`, `lng`, `timezone`, `countryCode`, `city`, `addrState`, `numAttendees`, `isOnline`, `startAt`, `endAt` on tournaments; `competitionTier` on events.

**PHASE_GROUPS_QUERY** — port unchanged.

**PHASE_GROUP_SETS_QUERY (full variant)** — adapted from HCI full query:
- Keep: `id`, `state`, `winnerId`, `vodUrl`, `completedAt`, `fullRoundText`, `round`, `lPlacement`, `wPlacement`, `displayScore`
- Keep: `phaseGroup { id, displayIdentifier, bracketType, phase { id, name, bracketType, phaseOrder, isExhibition } }`
- Keep: `slots { standing { stats { score { value } } }, entrant { id, initialSeedNum, isDisqualified, participants { player { id, gamerTag }, user { id, slug, name, bio, genderPronoun, location { city, state, country }, images { url, type } } } } }`
- Keep: `games { orderNum, winnerId, stage { id, name }, selections { selectionType, character { id, name } } }`
- Remove: `totalGames` (derivable), `startedAt`/`createdAt`/`updatedAt`/`setGamesType`/`identifier` (not stored), `entrant.name` (redundant with participant gamerTag), `bracketUrl`/`numRounds`/`startAt`/`state`/`numSeeds`/`groupCount` on phaseGroup/phase (not stored), `entrant1Score`/`entrant2Score` on games (not stored — winner/loser scores are on the set level)

**PHASE_GROUP_SETS_QUERY_SLIM (identity pass)** — drops `user { ... }` and `games { ... }` blocks entirely. Used as the first of two fallback passes when the full query is too complex at `perPage=1`. Gets set structure and `player.id` for fallback identity.

**PHASE_GROUP_GAMES_QUERY (games pass)** — new query. Requests only `set.id` and `games { orderNum, winnerId, stage { id, name }, selections { selectionType, character { id, name } } }` for each set in the phase group. Much lower complexity than the full query; used as the second fallback pass alongside the slim identity pass.

**EVENT_STANDINGS_QUERY** — port unchanged (populates `global_event_entries`).

### Fallback strategy

The crawler always attempts the full query first, halving `perPage` on complexity errors as in the HCI scraper. When `perPage=1` still fails, instead of a single slim query that permanently loses game/character data, two separate passes are made:

1. **Identity pass** (slim query, separate pagination): fetches set structure and player identity via `player.id`. Complexity is ~1/3 of the full query, so `perPage` can be substantially higher.
2. **Games pass** (games-only query, separate pagination): fetches game order, stage, and character selections per set. Also ~1/3 the complexity of the full query.

If the games pass also fails at `perPage=1` for a phase group, games and character data are skipped for that phase group only (logged as a warning). This is expected only for extraordinarily large events.

### Player identity resolution

**Full query pass:**
1. Extract `participants[0].user.id` as `startgg_user_id`, `player.id` as `startgg_player_id`
2. Extract `player.gamerTag` as `handle`, `user.name` as `display_name`, `user.slug` as `startgg_slug`, `user.bio`, `user.genderPronoun` as `pronouns`, `user.location` fields, first `user.images` URL as `profile_image_url`
3. `INSERT INTO global_players ... ON CONFLICT (startgg_user_id) DO UPDATE SET handle = EXCLUDED.handle, display_name = EXCLUDED.display_name, startgg_player_id = COALESCE(EXCLUDED.startgg_player_id, global_players.startgg_player_id), profile_image_url = COALESCE(EXCLUDED.profile_image_url, global_players.profile_image_url), startgg_slug = COALESCE(EXCLUDED.startgg_slug, global_players.startgg_slug), bio = COALESCE(EXCLUDED.bio, global_players.bio), pronouns = COALESCE(EXCLUDED.pronouns, global_players.pronouns), location_city = EXCLUDED.location_city, location_state = EXCLUDED.location_state, location_country = EXCLUDED.location_country, updated_at = NOW()`

**Slim identity pass (fallback):**
1. Extract `participants[0].player.id` as `startgg_player_id` (no `user.id` available)
2. Extract `player.gamerTag` as `handle`
3. `INSERT INTO global_players (startgg_player_id, handle) ... ON CONFLICT (startgg_player_id) DO UPDATE SET handle = EXCLUDED.handle, updated_at = NOW()`

A later full-query pass for the same player (appearing in a different event) merges the row via `startgg_user_id`, populating all NULL fields.

`COALESCE` throughout the upsert ensures previously-stored values are never overwritten by NULLs.

Entrants without a resolvable user or player ID (TBD slots) produce NULL player FKs on the set — normal and expected.

### Game and character data

After resolving and writing a set, for each game in `set.games`:
1. Upsert `global_set_games` with `set_id`, `order_num`, `winner_player_id` (looked up via `winnerId` → entrant → player), `stage_id`, `stage_name`
2. For each selection in `game.selections`: upsert `global_game_selections` with `game_id`, `player_id` (looked up via `entrant.id` → player already resolved above), `selection_type`, `character_id`, `character_name`

### DQ detection

A set is marked `is_dq = true` when `state = 7` (start.gg's DQ state code) or when `displayScore` contains "DQ".

### Write order within an event

1. Upsert `global_tournament`
2. Upsert `global_event` + `global_game` (videogame)
3. For each phase group: upsert `global_phase`, `global_phase_group`
4. For each set: upsert players, upsert `global_set`, upsert `global_set_games` + `global_game_selections`
5. After all phase groups: upsert `global_event_entries` from standings response
6. Write `crawler_checkpoints` event key
7. After all events in tournament: write `crawler_checkpoints` tournament key

### Configuration (env vars)

| Var | Purpose |
|---|---|
| `DATABASE_URL` | Postgres connection string |
| `STARTGG_API_KEY` | Platform-level start.gg API key (not per-user) |
| `CRAWLER_FROM_DATE` | Start of backfill window (e.g. `2018-01-01`). Defaults to `2015-01-01`. |
| `CRAWLER_TO_DATE` | End of crawl window. Defaults to today. |
| `CRAWLER_WINDOW_DAYS` | Days per sliding window. Defaults to `7`. |
| `CRAWLER_DELAY_MS` | Delay between API requests in ms. Defaults to `750`. |
| `CRAWLER_GAME_ID` | Optional start.gg game ID filter (crawl one game only). |

CLI args mirror the HCI scraper's args but read defaults from env vars so the Docker service needs zero CLI flags in the compose file.

### Continuous operation

After the initial backfill window completes, the crawler loops: it advances `window_start` to the most recent checkpoint and re-runs forward. The `crawler_checkpoints` table's `"window_start"` key ensures restarts resume cleanly.

---

## Docker

New `crawler` service in `docker-compose.yml`:

```yaml
crawler:
  build: ./backend
  command: ["./crawler"]
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

The crawler binary is added to the existing `backend/Dockerfile` `COPY --from=builder` step alongside `api` and `worker`.

---

## Testing

Unit tests for the identity resolution logic and DQ detection live in `crates/crawler/src/`. Integration tests use `#[sqlx::test]` with a wiremock start.gg server (same pattern as `crates/api`). A full end-to-end test verifies that a synthetic tournament fixture is correctly mirrored into the global tables, including game/character data and the two-pass fallback path.

No new Playwright tests are needed (crawler is backend-only).

---

## Future: Global Ratings Integration

The `global_player_ratings` table is defined in this migration but not populated by the crawler. A future `compute_global_ratings` phase will:

1. Walk `global_sets` for a given game in `completed_at` ascending order
2. Feed them through the `RankingAlgorithm` trait (same trait used by Sub-project A's `compute_ranking` job)
3. Upsert results into `global_player_ratings`

Once populated, two Sub-project A features activate automatically:

- **`include_external_results = true`** on a ranking: the `compute_ranking` worker joins `global_player_ratings` via `startgg_user_id` to seed external-opponent strength
- **`result_sort = 'global_rating'`** on a ranking: the stats endpoint sorts wins/losses by opponent global rating

The per-user `startgg_api_key` field on `users` becomes redundant for tournament import once the mirror has sufficient coverage. At that point, the worker's import path shifts from live start.gg API calls to queries against `global_*` tables, and the API key gate on project creation can be removed.

---

## Out of Scope

- Per-player and per-tournament frontend pages — separate feature spec
- Multiple API keys for higher crawl throughput — separate operational concern
- Global rating computation — separate implementation phase
