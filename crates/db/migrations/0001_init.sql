-- Run It Back: initial schema.
-- gen_random_uuid() is built into PostgreSQL 13+ core, no extension needed.

CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username        TEXT NOT NULL UNIQUE,
    email           TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at   TIMESTAMPTZ
);

CREATE TABLE refresh_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash      TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL,
    revoked         BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);

-- Raw upload, kept verbatim so re-parsing (e.g. after a parser bugfix) never
-- loses information.
CREATE TABLE hand_histories (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    site                TEXT NOT NULL,
    original_filename   TEXT,
    raw_text            TEXT NOT NULL,
    hand_count          INT NOT NULL DEFAULT 0,
    status              TEXT NOT NULL DEFAULT 'pending', -- pending | parsing | parsed | failed
    error               TEXT,
    uploaded_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    parsed_at           TIMESTAMPTZ
);
CREATE INDEX idx_hand_histories_user ON hand_histories(user_id);

-- One row per hand, in our canonical cross-site schema (see rib-parser's
-- `ParsedHand`).
CREATE TABLE parsed_hands (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hand_history_id     UUID NOT NULL REFERENCES hand_histories(id) ON DELETE CASCADE,
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    site                TEXT NOT NULL,
    site_hand_id        TEXT,
    game_type           TEXT NOT NULL DEFAULT 'nlhe',
    table_size          INT NOT NULL,
    hero_position       TEXT,
    big_blind           DOUBLE PRECISION,
    played_at           TIMESTAMPTZ,
    board               TEXT[] NOT NULL DEFAULT '{}',
    hero_cards          TEXT[] NOT NULL DEFAULT '{}',
    actions             JSONB NOT NULL,           -- Vec<ParsedAction>
    result_bb           DOUBLE PRECISION NOT NULL DEFAULT 0,
    went_to_showdown    BOOLEAN NOT NULL DEFAULT FALSE,
    won_hand            BOOLEAN NOT NULL DEFAULT FALSE,
    tags                TEXT[] NOT NULL DEFAULT '{}', -- vpip,pfr,three_bet,cbet_flop,...
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_parsed_hands_user ON parsed_hands(user_id);
CREATE INDEX idx_parsed_hands_history ON parsed_hands(hand_history_id);
CREATE UNIQUE INDEX idx_parsed_hands_dedupe ON parsed_hands(user_id, site, site_hand_id) WHERE site_hand_id IS NOT NULL;

-- Aggregated stats, recomputed incrementally as new hands are parsed.
CREATE TABLE player_stats (
    id                      UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                 UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_type               TEXT NOT NULL DEFAULT 'nlhe',
    sample_size             INT NOT NULL DEFAULT 0,
    vpip                    DOUBLE PRECISION,
    pfr                     DOUBLE PRECISION,
    three_bet               DOUBLE PRECISION,
    fold_to_three_bet       DOUBLE PRECISION,
    cbet_flop               DOUBLE PRECISION,
    fold_to_cbet_flop       DOUBLE PRECISION,
    cbet_turn               DOUBLE PRECISION,
    wtsd                    DOUBLE PRECISION,
    won_at_showdown         DOUBLE PRECISION,
    aggression_factor       DOUBLE PRECISION,
    net_bb_per_100          DOUBLE PRECISION,
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, game_type)
);

-- Precomputed / cached solver output, keyed by `rib_solver::SpotKey::cache_key()`.
CREATE TABLE solved_spots (
    cache_key           TEXT PRIMARY KEY,
    game_type           TEXT NOT NULL,
    pot_type            TEXT NOT NULL,
    stack_bb            INT NOT NULL,
    hero_position       TEXT NOT NULL,
    villain_position    TEXT NOT NULL,
    board               TEXT[] NOT NULL DEFAULT '{}',
    response            JSONB NOT NULL, -- serialized SolveResponse
    iterations           INT NOT NULL,
    solved_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_solved_spots_lookup ON solved_spots(game_type, pot_type, stack_bb);

-- Async solve requests submitted through the worker pool (live, on-demand
-- custom spots that aren't in the library).
CREATE TABLE solve_jobs (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID REFERENCES users(id) ON DELETE CASCADE,
    request         JSONB NOT NULL,
    status          TEXT NOT NULL DEFAULT 'queued', -- queued | running | done | failed
    progress        REAL NOT NULL DEFAULT 0,
    result          JSONB,
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at    TIMESTAMPTZ
);
CREATE INDEX idx_solve_jobs_user ON solve_jobs(user_id);

CREATE TABLE ranges (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID REFERENCES users(id) ON DELETE CASCADE, -- NULL = system preset
    name            TEXT NOT NULL,
    game_type       TEXT NOT NULL DEFAULT 'nlhe',
    range_string    TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_ranges_user ON ranges(user_id);

CREATE TABLE drills (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id             UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_type           TEXT NOT NULL DEFAULT 'nlhe',
    category            TEXT NOT NULL, -- 'preflop_open','cbet','three_bet_pot','blockers',...
    spot_key            TEXT,           -- references solved_spots.cache_key when applicable
    spot_snapshot       JSONB NOT NULL, -- full spot description shown to the user
    dealt_hand          TEXT[] NOT NULL,
    correct_strategy    JSONB NOT NULL, -- {action_label: frequency} for dealt_hand
    correct_ev_bb       DOUBLE PRECISION NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_drills_user ON drills(user_id);

CREATE TABLE drill_attempts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    drill_id        UUID NOT NULL REFERENCES drills(id) ON DELETE CASCADE,
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    chosen_action   TEXT NOT NULL,
    ev_loss_bb      DOUBLE PRECISION NOT NULL DEFAULT 0,
    is_correct      BOOLEAN NOT NULL,
    explanation     TEXT NOT NULL,
    answered_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_drill_attempts_user ON drill_attempts(user_id);
CREATE INDEX idx_drill_attempts_drill ON drill_attempts(drill_id);

-- Per (user, category) rolling accuracy/EV-loss, the input to the adaptive
-- drill generator's weakness-weighted spot selection.
CREATE TABLE weakness_profiles (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    game_type       TEXT NOT NULL DEFAULT 'nlhe',
    category        TEXT NOT NULL,
    attempts        INT NOT NULL DEFAULT 0,
    correct         INT NOT NULL DEFAULT 0,
    avg_ev_loss_bb  DOUBLE PRECISION NOT NULL DEFAULT 0,
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, game_type, category)
);
