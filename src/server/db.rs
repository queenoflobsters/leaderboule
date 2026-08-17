use serde::{Deserialize, Serialize};
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
    types::SurrealValue,
    Surreal,
};
use tokio::sync::OnceCell;

use crate::{
    api::db::{
        current_user::UserProfile,
        global::{LeaderboardSortMethod, LeaderboardUserCard},
    },
    server::auth::UserId,
};

pub static DB: OnceCell<Surreal<Client>> = OnceCell::const_new();
const INIT_SQL: &'static str = include_str!("../../db_init.sql");

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

        db.query(INIT_SQL)
            .await
            .expect("Fail to initialize database");

        db
    })
    .await
}

/// Here are the SQL variables needed to be defined to use this :
/// $search -> the search query
/// $limit -> the maximum amount of records returned
/// $start -> the start of the search
pub fn construct_leaderboard_query(
    do_query: bool,
    sort_method: LeaderboardSortMethod,
) -> String {
    let base_select = match sort_method {
        LeaderboardSortMethod::WinRatio => "SELECT *, (IF games_played > 0 THEN games_won / games_played ELSE 0.0 END) AS win_ratio FROM user",
        _ => "SELECT * FROM user",
    };

    let query_clause = if do_query {
        "WHERE username @1@ $search"
    } else {
        ""
    };

    let sort_clause = match sort_method {
        LeaderboardSortMethod::Elo => "ORDER BY elo DESC, games_played DESC",
        LeaderboardSortMethod::GamesPlayed => "ORDER BY games_played DESC, elo DESC",
        LeaderboardSortMethod::GamesWon => "ORDER BY games_won DESC, elo DESC",
        LeaderboardSortMethod::WinRatio => "ORDER BY win_ratio DESC, elo DESC",
    };

    let limit_clause = "LIMIT $limit START $start";
    format!("{base_select} {query_clause} {sort_clause} {limit_clause}")
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
