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

-- Tournaments (project-scoped; same startgg_id may appear in multiple projects)
CREATE TABLE tournaments (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id     UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    startgg_id     BIGINT      NOT NULL,
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

CREATE UNIQUE INDEX tournaments_project_startgg_idx ON tournaments(project_id, startgg_id);
CREATE INDEX tournaments_project_id_idx ON tournaments(project_id);

CREATE TABLE events (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    tournament_id UUID        NOT NULL REFERENCES tournaments(id) ON DELETE CASCADE,
    startgg_id    BIGINT      NOT NULL,
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
CREATE UNIQUE INDEX events_tournament_startgg_idx ON events(tournament_id, startgg_id);

CREATE TABLE phases (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id    BIGINT      NOT NULL,
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
CREATE UNIQUE INDEX phases_event_startgg_idx ON phases(event_id, startgg_id);

CREATE TABLE phase_groups (
    id                 UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id         BIGINT      NOT NULL,
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
CREATE UNIQUE INDEX phase_groups_phase_startgg_idx ON phase_groups(phase_id, startgg_id);

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
