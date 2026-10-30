-- Schéma PostgreSQL de Leaderboule.
--
-- Les dates sont stockées en secondes UNIX (BIGINT) pour rester compatibles
-- avec le reste de l'application, et les anciens champs calculés de SurrealDB
-- sont désormais des colonnes générées par PostgreSQL.

CREATE TABLE users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL,
    username TEXT NOT NULL,
    member_since BIGINT NOT NULL,
    elo BIGINT NOT NULL DEFAULT 400,
    best_elo BIGINT NOT NULL DEFAULT 400,
    games_played BIGINT NOT NULL DEFAULT 0,
    games_won BIGINT NOT NULL DEFAULT 0,
    games_lost BIGINT GENERATED ALWAYS AS (games_played - games_won) STORED,
    win_ratio DOUBLE PRECISION GENERATED ALWAYS AS (
        CASE
            WHEN games_played > 0 THEN games_won::double precision * 100.0 / games_played
            ELSE 0.0
        END
    ) STORED,
    CONSTRAINT users_email_unique UNIQUE (email),
    CONSTRAINT users_username_unique UNIQUE (username),
    CONSTRAINT users_email_format CHECK (
        email ~ '^[^@[:space:]]+@[^@[:space:]]+\.[^@[:space:]]+$'
    ),
    CONSTRAINT users_username_length CHECK (char_length(username) BETWEEN 3 AND 32),
    CONSTRAINT users_elo_positive CHECK (elo >= 0),
    CONSTRAINT users_best_elo_positive CHECK (best_elo >= 0),
    CONSTRAINT users_games_positive CHECK (games_played >= 0),
    CONSTRAINT users_wins_valid CHECK (games_won >= 0 AND games_won <= games_played)
);

CREATE INDEX users_leaderboard_elo_idx ON users (elo DESC, games_played DESC);
CREATE INDEX users_leaderboard_games_played_idx ON users (games_played DESC, elo DESC);
CREATE INDEX users_leaderboard_games_won_idx ON users (games_won DESC, elo DESC);
CREATE INDEX users_win_ratio_idx ON users (win_ratio DESC, elo DESC);

-- Maintient le record d'élo sans avoir à y penser dans le code applicatif.
CREATE FUNCTION users_sync_best_elo() RETURNS trigger AS $$
BEGIN
    NEW.best_elo := GREATEST(NEW.best_elo, NEW.elo);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER users_sync_best_elo
    BEFORE INSERT OR UPDATE OF elo ON users
    FOR EACH ROW
    EXECUTE FUNCTION users_sync_best_elo();

CREATE TABLE games (
    id UUID PRIMARY KEY,
    won_score BIGINT NOT NULL,
    lost_score BIGINT NOT NULL,
    recorded_by UUID NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    played_at BIGINT NOT NULL,
    CONSTRAINT games_scores_valid CHECK (
        won_score BETWEEN 0 AND 13
        AND lost_score BETWEEN 0 AND 13
        AND won_score > lost_score
    )
);

CREATE INDEX games_played_at_idx ON games (played_at DESC);

CREATE TABLE game_players (
    game_id UUID NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    elo_change BIGINT NOT NULL,
    won BOOLEAN NOT NULL,
    PRIMARY KEY (game_id, user_id)
);

CREATE INDEX game_players_user_id_idx ON game_players (user_id, game_id);

CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at BIGINT NOT NULL,
    CONSTRAINT sessions_expires_at_positive CHECK (expires_at > 0)
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id);
CREATE INDEX sessions_expires_at_idx ON sessions (expires_at);

CREATE TABLE user_creds (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL UNIQUE REFERENCES users (id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL,
    CONSTRAINT user_creds_password_hash_valid CHECK (password_hash LIKE '$argon2%')
);
