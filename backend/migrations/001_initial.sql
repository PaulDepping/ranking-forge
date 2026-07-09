-- Enums
CREATE TYPE job_kind AS ENUM ('import_tournaments', 'compute_ranking');
CREATE TYPE job_status AS ENUM ('pending', 'running', 'done', 'failed');

-- Users and sessions
CREATE TABLE users (
    id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT        NOT NULL UNIQUE,
    display_name    TEXT        NOT NULL,
    password_hash   TEXT        NOT NULL,
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

-- Projects
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

-- Rankings
CREATE TABLE rankings (
    id                       UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id               UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name                     TEXT        NOT NULL,
    description              TEXT,
    published                BOOLEAN     NOT NULL DEFAULT FALSE,
    algorithm                TEXT,
    algorithm_config         JSONB       NOT NULL DEFAULT '{}',
    include_external_results BOOLEAN     NOT NULL DEFAULT FALSE,
    result_sort              TEXT        NOT NULL DEFAULT 'upset_factor',
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX rankings_project_id_idx ON rankings(project_id);

-- Players
CREATE TABLE players (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id    UUID        NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name          TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX players_project_id_idx ON players(project_id);

CREATE TABLE ranking_players (
    ranking_id    UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    player_id     UUID    NOT NULL REFERENCES players(id) ON DELETE CASCADE,
    rank_position INTEGER NOT NULL DEFAULT 0,
    notes         TEXT,
    PRIMARY KEY (ranking_id, player_id)
);

CREATE INDEX ranking_players_player_id_idx ON ranking_players(player_id);

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

-- Per-player algorithm scores
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
    banner_url          TEXT,
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
    id                UUID             PRIMARY KEY DEFAULT gen_random_uuid(),
    startgg_id        BIGINT           NOT NULL UNIQUE,
    name              TEXT             NOT NULL,
    slug              TEXT             NOT NULL,
    short_slug        TEXT,
    start_at          TIMESTAMPTZ,
    end_at            TIMESTAMPTZ,
    country_code      TEXT,
    city              TEXT,
    addr_state        TEXT,
    venue_name        TEXT,
    venue_address     TEXT,
    online            BOOLEAN,
    num_attendees     INTEGER,
    lat               DOUBLE PRECISION,
    lng               DOUBLE PRECISION,
    timezone          TEXT,
    hashtag           TEXT,
    profile_image_url TEXT,
    banner_url        TEXT,
    created_at        TIMESTAMPTZ      NOT NULL DEFAULT NOW()
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
    state            TEXT,
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

-- ============================================================
-- Project → global bridge tables (depend on global_events/sets)
-- ============================================================

CREATE TABLE project_events (
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    global_event_id UUID NOT NULL REFERENCES global_events(id),
    PRIMARY KEY (project_id, global_event_id)
);
CREATE INDEX project_events_project_id_idx ON project_events(project_id);

CREATE TABLE ranking_events (
    ranking_id      UUID    NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    global_event_id UUID    NOT NULL REFERENCES global_events(id),
    included        BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY (ranking_id, global_event_id)
);
CREATE INDEX ranking_events_event_id_idx ON ranking_events(global_event_id);

CREATE TABLE ranking_set_results (
    ranking_id       UUID        NOT NULL REFERENCES rankings(id) ON DELETE CASCADE,
    global_set_id    UUID        NOT NULL REFERENCES global_sets(id) ON DELETE CASCADE,
    winner_player_id UUID        NOT NULL REFERENCES players(id),
    loser_player_id  UUID        NOT NULL REFERENCES players(id),
    global_event_id  UUID        NOT NULL REFERENCES global_events(id),
    completed_at     TIMESTAMPTZ,
    PRIMARY KEY (ranking_id, global_set_id)
);
CREATE INDEX ranking_set_results_winner_idx ON ranking_set_results(ranking_id, winner_player_id);
CREATE INDEX ranking_set_results_loser_idx  ON ranking_set_results(ranking_id, loser_player_id);
