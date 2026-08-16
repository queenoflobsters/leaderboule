-- I. define the user table
DEFINE TABLE IF NOT EXISTS user;

-- email field constraints
DEFINE FIELD IF NOT EXISTS email ON TABLE user
    TYPE string
    VALUE string::trim($value)
    ASSERT string::is_email($value);

-- username field constraints
DEFINE FIELD IF NOT EXISTS username ON TABLE user 
    TYPE string 
    VALUE string::trim($value) 
    ASSERT string::len($value) >= 3 AND string::len($value) <= 50;

-- Indexes for unique values and fast searching
DEFINE INDEX IF NOT EXISTS idx_user_leaderboard ON TABLE user FIELDS elo, games_played;
DEFINE INDEX IF NOT EXISTS idx_user_username_unique ON TABLE user FIELDS username UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_user_email_unique ON TABLE user FIELDS email UNIQUE;

-- II. define the session table
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
DEFINE INDEX IF NOT EXISTS idx_session_expires ON TABLE session FIELDS expries_at;

-- III. define the user_cred table
DEFINE TABLE IF NOT EXISTS user_cred SCHEMAFULL;

-- link to the user table
DEFINE FIELD IF NOT EXISTS user_id ON TABLE user_cred
    TYPE record<user>
    ASSERT $value != NONE AND record::exists($value)
    REFERENCE ON DELETE CASCADE;

-- ensure a user can only have ONE cred record
DEFINE INDEX IF NOT EXISTS idx_user_cred_user_id_unique ON TABLE user_cred
    FIELDS user_id UNIQUE;

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
