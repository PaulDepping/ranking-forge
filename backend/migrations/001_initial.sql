-- Enums
CREATE TYPE job_kind AS ENUM ('import_tournaments');
CREATE TYPE job_status AS ENUM ('pending', 'running', 'done', 'failed');

-- Users and sessions
CREATE TABLE users (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    username      TEXT        NOT NULL UNIQUE,
    password_hash TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE sessions (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX sessions_user_id_idx ON sessions(user_id);

-- Ranking projects
CREATE TABLE ranking_projects (
    id          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT        NOT NULL,
    game_id     BIGINT,                -- start.gg videogame ID
    game_name   TEXT,                  -- cached display name
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX ranking_projects_user_id_idx ON ranking_projects(user_id);

-- Players (project-scoped)
CREATE TABLE players (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID        NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    name       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX players_project_id_idx ON players(project_id);

-- start.gg accounts linked to players
CREATE TABLE startgg_accounts (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    player_id       UUID        NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    startgg_user_id BIGINT      NOT NULL,
    slug            TEXT        NOT NULL,  -- e.g. "user/abc123"
    display_name    TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (player_id, startgg_user_id)
);

CREATE INDEX startgg_accounts_player_id_idx ON startgg_accounts(player_id);
-- Used when matching imported entrants back to our players
CREATE INDEX startgg_accounts_user_id_idx ON startgg_accounts(startgg_user_id);

-- Tournaments (imported from start.gg, shared across projects)
CREATE TABLE tournaments (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id     BIGINT      NOT NULL UNIQUE,
    name           TEXT        NOT NULL,
    slug           TEXT        NOT NULL,
    city           TEXT,
    addr_state     TEXT,
    country_code   TEXT,
    venue_name     TEXT,
    venue_address  TEXT,
    timezone       TEXT,        -- IANA tz, e.g. "America/Chicago"; needed to display set times correctly
    online         BOOLEAN     NOT NULL DEFAULT FALSE,
    num_attendees  INTEGER,
    start_at       TIMESTAMPTZ,
    end_at         TIMESTAMPTZ,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Events within tournaments (e.g. "Singles", "Doubles")
-- start_at is stored on the event (not just the tournament) because a multi-day
-- tournament's events may fall on different days, and time-range filtering should
-- key off the specific event time.
CREATE TABLE events (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID        NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    startgg_id    BIGINT      NOT NULL UNIQUE,
    name          TEXT        NOT NULL,
    game_id       BIGINT,
    game_name     TEXT,
    num_entrants  INTEGER,
    start_at      TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX events_start_at_idx ON events(start_at);

CREATE INDEX events_tournament_id_idx ON events(tournament_id);

-- Per-project event inclusion (default: included)
CREATE TABLE project_events (
    project_id UUID    NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    event_id   UUID    NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    included   BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (project_id, event_id)
);

-- Entrants: one player's participation in one event
-- player_id is NULL when the entrant's start.gg user isn't in our tracked list
CREATE TABLE entrants (
    id                  UUID    PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id            UUID    NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    player_id           UUID    REFERENCES players(id) ON DELETE SET NULL,
    startgg_entrant_id  BIGINT  NOT NULL,
    startgg_user_id     BIGINT,           -- for matching against startgg_accounts
    seed                INTEGER,
    display_name        TEXT    NOT NULL,
    is_disqualified     BOOLEAN NOT NULL DEFAULT FALSE,
    final_placement     INTEGER,          -- finish position in the event (from Standing where isFinal=true)
    UNIQUE (event_id, startgg_entrant_id)
);

CREATE INDEX entrants_event_id_idx ON entrants(event_id);
CREATE INDEX entrants_player_id_idx ON entrants(player_id);
CREATE INDEX entrants_startgg_user_id_idx ON entrants(startgg_user_id);

-- Sets: match results (winner/loser entrant pairs)
-- is_dq is denormalized here for cheap filtering; a set can be a DQ independently
-- of entrant.is_disqualified (which marks a full event DQ). DQ sets are excluded
-- from upset factor calculations.
CREATE TABLE sets (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id          UUID        NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    startgg_set_id    BIGINT      NOT NULL,
    winner_entrant_id UUID        NOT NULL REFERENCES entrants(id),
    loser_entrant_id  UUID        NOT NULL REFERENCES entrants(id),
    round             INTEGER,           -- raw round number from start.gg
    round_name        TEXT,              -- display name, e.g. "Winners Finals", "Grand Finals"
    best_of           SMALLINT,          -- format: 3, 5, or 7
    winner_score      SMALLINT,          -- games won by winner
    loser_score       SMALLINT,          -- games won by loser
    is_dq             BOOLEAN     NOT NULL DEFAULT FALSE,
    vod_url           TEXT,
    completed_at      TIMESTAMPTZ,       -- when the set finished
    UNIQUE (event_id, startgg_set_id)
);

CREATE INDEX sets_event_id_idx ON sets(event_id);
CREATE INDEX sets_winner_entrant_id_idx ON sets(winner_entrant_id);
CREATE INDEX sets_loser_entrant_id_idx ON sets(loser_entrant_id);
CREATE INDEX sets_completed_at_idx ON sets(completed_at);

-- Job queue for background worker
CREATE TABLE jobs (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    kind       job_kind    NOT NULL,
    project_id UUID        NOT NULL REFERENCES ranking_projects(id) ON DELETE CASCADE,
    status     job_status  NOT NULL DEFAULT 'pending',
    error      TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX jobs_status_idx ON jobs(status) WHERE status IN ('pending', 'running');
CREATE INDEX jobs_project_id_idx ON jobs(project_id);
