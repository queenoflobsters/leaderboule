use serde::{Deserialize, Serialize};
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
    types::SurrealValue,
    Surreal,
};
use tokio::sync::OnceCell;

use crate::{
    api::db::{current_user::UserProfile, global::LeaderboardUserCard},
    server::auth::UserId,
};

pub static DB: OnceCell<Surreal<Client>> = OnceCell::const_new();

/// Initialize SurrealDB connection singleton
pub async fn get() -> &'static Surreal<Client> {
    DB.get_or_init(|| async {
        let db_url = std::env::var("DATABASE_URL").unwrap_or("127.0.0.1:8000".to_string());
        let db_user = std::env::var("DATABASE_USER").unwrap_or("root".to_string());
        let db_pass = std::env::var("DATABASE_PASS").unwrap_or("root".to_string());

        let db = Surreal::new::<Ws>(db_url)
            .await
            .expect("Failed to connect to SurrealDB");

        db.signin(Root {
            username: db_user,
            password: db_pass,
        })
        .await
        .expect("Failed to sign in to SurrealDB");

        db.use_ns("leaderboule")
            .use_db("leaderboule")
            .await
            .expect("Failed to select namespace/db");

        db.query("DEFINE TABLE IF NOT EXISTS users")
            .await
            .expect("Failed to create table \"users\"");
        // TODO add indexes for sorting
        db.query("DEFINE INDEX idx_users_leaderboard ON TABLE users FIELDS elo, games_played;")
            .await
            .expect("Failed to create leaderboard indexes");

        // email validation and uniqueness
        db.query(
            "
                DEFINE FIELD IF NOT EXISTS email ON TABLE users
                    TYPE string
                    VALUE string::trim($value)
                    ASSERT string::is_email($value);
                DEFINE INDEX IF NOT EXISTS idx_users_email_unique ON TABLE users FIELDS email UNIQUE;
            ",
        )
        .await
        .expect("Failed to create email constraints");

        db.query(
            "
                DEFINE FIELD IF NOT EXISTS username ON TABLE user 
                    TYPE string 
                    VALUE string::trim($value) 
                    ASSERT string::len($value) >= 3 AND string::len($value) <= 20;
                DEFINE INDEX IF NOT EXISTS idx_users_username_unique ON TABLE user FIELDS username UNIQUE;
            "
        ).await.expect("Failed to create username constraints");

        db
    })
    .await
}

/// User database model
#[derive(Serialize, Deserialize, Clone, SurrealValue)]
pub struct UserRecord {
    pub id: UserId,
    pub email: String,
    pub username: String,
    pub elo: u64,
    pub games_played: u64,
    pub games_won: u64,
}

impl From<UserRecord> for UserProfile {
    fn from(value: UserRecord) -> Self {
        Self {
            email: value.email,
            username: value.username,
            elo: value.elo,
            games_played: value.games_played,
            games_won: value.games_won,
        }
    }
}

impl From<UserRecord> for LeaderboardUserCard {
    fn from(value: UserRecord) -> Self {
        Self {
            username: value.username,
            elo: value.elo,
            games_played: value.games_played,
            games_won: value.games_won,
        }
    }
}
