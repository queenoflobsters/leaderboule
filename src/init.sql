-- I. define the user table
DEFINE TABLE IF NOT EXISTS user;

-- email field constraints
DEFINE FIELD IF NOT EXISTS email ON TABLE user
    VALUE string::trim($value)
    TYPE string
    ASSERT string::is_email($value);

-- username field constraints
DEFINE FIELD IF NOT EXISTS username ON TABLE user 
    VALUE string::trim($value) 
    TYPE string 
    ASSERT string::len($value) >= 3 AND string::len($value) <= 32;

-- elo constraint (must be >= 0)
DEFINE FIELD IF NOT EXISTS elo ON TABLE user
    TYPE int
    DEFAULT 400
    ASSERT $value >= 0;

-- games played constraint (>= 0 and >= games won)
DEFINE FIELD IF NOT EXISTS games_played ON TABLE user
    TYPE int
    DEFAULT 0
    ASSERT $value >= 0 AND $value >= $this.games_won;

-- games won constraints (>= 0 and <= games played)
DEFINE FIELD IF NOT EXISTS games_won ON TABLE user
    TYPE int
    DEFAULT 0
    ASSERT $value >= 0 AND $value <= $this.games_played;

-- define username analyzer for searching
DEFINE ANALYZER IF NOT EXISTS user_search_analyzer
    TOKENIZERS class
    FILTERS lowercase, ascii, edgengram(1, 20);

-- AUTO-COMPUTED: games_lost 
DEFINE FIELD IF NOT EXISTS games_lost ON TABLE user
    COMPUTED games_played - games_won
    TYPE int;

-- AUTO-COMPUTED: win_ratio 
DEFINE FIELD IF NOT EXISTS win_ratio ON TABLE user
    COMPUTED IF games_played > 0
        THEN (games_won * 100.0) / games_played
        ELSE 0.0
    END
    TYPE float;


-- AUTO-COMPUTED: best elo
DEFINE FIELD IF NOT EXISTS best_elo ON TABLE user
    VALUE IF $before.best_elo == NONE 
        THEN $this.elo 
        ELSE math::max([$this.elo, $before.best_elo]) 
    END
    DEFAULT 400
    TYPE int;

-- Indexes for unique values and fast searching
DEFINE INDEX IF NOT EXISTS idx_user_leaderboard ON TABLE user FIELDS elo, games_played;
DEFINE INDEX IF NOT EXISTS idx_user_username_unique ON TABLE user FIELDS username UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_user_email_unique ON TABLE user FIELDS email UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_user_search ON TABLE user FIELDS username FULLTEXT ANALYZER user_search_analyzer;


-- II. define the game table
DEFINE TABLE IF NOT EXISTS game SCHEMALESS;

DEFINE FIELD IF NOT EXISTS won_score ON TABLE game
    TYPE int
    ASSERT $value >= 0 AND $value <= 13 AND $value > $this.lost_score;

DEFINE FIELD IF NOT EXISTS lost_score ON TABLE game
    TYPE int
    ASSERT $value >= 0 AND $value <= 13 AND $value < $this.won_score;

DEFINE FIELD IF NOT EXISTS players ON TABLE game
    TYPE array
    ASSERT array::len($value) >= 2;

DEFINE FIELD IF NOT EXISTS recorded_by ON TABLE game 
    TYPE record<user>
    ASSERT $value != NONE AND record::exists($value);

DEFINE FIELD IF NOT EXISTS played_at ON TABLE game
    TYPE int;

-- AUTO-COMPUTED : games won
DEFINE FIELD IF NOT EXISTS won_players ON TABLE game
    COMPUTED $this.players[WHERE won = true];
-- AUTO-COMPUTED : games lost
DEFINE FIELD IF NOT EXISTS lost_players ON TABLE game
    COMPUTED $this.players[WHERE won = false];

DEFINE INDEX IF NOT EXISTS idx_game_played_at ON TABLE game FIELDS played_at;
DEFINE INDEX IF NOT EXISTS idx_game_players ON TABLE game FIELDS players.*.id;

-- III. define the session table
DEFINE TABLE IF NOT EXISTS session SCHEMALESS;

-- user_id field constraints
DEFINE FIELD IF NOT EXISTS user_id ON TABLE session
    TYPE record<user>
    ASSERT $value != NONE AND record::exists($value)
    REFERENCE ON DELETE CASCADE;

-- expires_at field constraints
DEFINE FIELD IF NOT EXISTS expires_at ON TABLE session
    TYPE number
    ASSERT $value > 0;

DEFINE INDEX IF NOT EXISTS idx_session_user ON TABLE session FIELDS user_id;
DEFINE INDEX IF NOT EXISTS idx_session_expires ON TABLE session FIELDS expires_at;


-- IV. define the user_cred table
DEFINE TABLE IF NOT EXISTS user_cred SCHEMAFULL;

-- link to the user table
DEFINE FIELD IF NOT EXISTS user_id ON TABLE user_cred
    TYPE record<user>
    ASSERT $value != NONE AND record::exists($value)
    REFERENCE ON DELETE CASCADE;

-- ensure a user can only have ONE cred record
DEFINE INDEX IF NOT EXISTS idx_user_cred_user_id_unique ON TABLE user_cred FIELDS user_id UNIQUE;

-- password hash constraints
DEFINE FIELD IF NOT EXISTS password_hash ON TABLE user_cred
    TYPE string
    ASSERT string::starts_with($value, '$argon2');

-- 4. Audit timestamps
-- DEFINE FIELD IF NOT EXISTS created_at ON TABLE user_auth 
--     TYPE datetime 
--     DEFAULT time::now() 
--     READONLY;

-- DEFINE FIELD IF NOT EXISTS updated_at ON TABLE user_auth 
--     TYPE datetime 
--     DEFAULT time::now() 
--     VALUE time::now();
