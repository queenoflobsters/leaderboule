use dioxus::server::ServerFnError;
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

pub async fn get_leaderboard_cards(
    search_query: String,
    sort_method: LeaderboardSortMethod,
    page: u64,
    page_size: u64,
) -> Result<Vec<LeaderboardUserCard>, ServerFnError> {
    let db = get().await;

    let query_clause = if !search_query.is_empty() {
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

    let query =
        format!("SELECT * FROM user {query_clause} {sort_clause} LIMIT $limit START $start");

    let mut cards: Vec<LeaderboardUserCard> = db
        .query(query)
        .bind(("limit", page_size))
        .bind(("start", page * page_size))
        .bind(("search", search_query))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fill with the rank
    for card in &mut cards {
        card.rank = Some(get_user_elo_rank(card.elo).await?)
    }

    Ok(cards)
}

pub async fn get_user_elo_rank(elo: u64) -> Result<u64, ServerFnError> {
    let db = get().await;
    let higher_count: Option<u64> = db
        .query("count(SELECT VALUE id FROM user WHERE elo > $this_elo)")
        .bind(("this_elo", elo))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    higher_count.ok_or(ServerFnError::new(
        "Couldn't get the user's rank".to_string(),
    ))
}

impl SurrealValue for LeaderboardUserCard {
    fn kind_of() -> surrealdb::types::Kind {
        surrealdb::types::Kind::Any
    }

    fn into_value(self) -> surrealdb::types::Value {
        panic!("You should never write a LeaderboardUserCard into the database");
    }

    fn from_value(value: surrealdb::types::Value) -> Result<Self, surrealdb::Error>
    where
        Self: Sized,
    {
        use surrealdb::types::{SurrealValue, Value};

        match value {
            Value::Object(mut obj) => Ok(Self {
                username: SurrealValue::from_value(obj.remove("username").unwrap_or(Value::None))?,
                elo: SurrealValue::from_value(obj.remove("elo").unwrap_or(Value::None))?,
                best_elo: SurrealValue::from_value(obj.remove("best_elo").unwrap_or(Value::None))?,
                games_played: SurrealValue::from_value(
                    obj.remove("games_played").unwrap_or(Value::None),
                )?,
                games_won: SurrealValue::from_value(
                    obj.remove("games_won").unwrap_or(Value::None),
                )?,
                games_lost: SurrealValue::from_value(
                    obj.remove("games_lost").unwrap_or(Value::None),
                )?,
                win_ratio: SurrealValue::from_value(
                    obj.remove("win_ratio").unwrap_or(Value::None),
                )?,
                rank: None,
            }),
            other => <()>::from_value(other).map(|_| unreachable!()),
        }
    }
}
impl SurrealValue for UserProfile {
    fn kind_of() -> surrealdb::types::Kind {
        surrealdb::types::Kind::Any
    }

    fn into_value(self) -> surrealdb::types::Value {
        panic!("You should never write a UserProfile into the database");
    }

    fn from_value(value: surrealdb::types::Value) -> Result<Self, surrealdb::Error>
    where
        Self: Sized,
    {
        use surrealdb::types::{SurrealValue, Value};

        match value {
            Value::Object(mut obj) => Ok(Self {
                email: SurrealValue::from_value(obj.remove("email").unwrap_or(Value::None))?,
                username: SurrealValue::from_value(obj.remove("username").unwrap_or(Value::None))?,
                elo: SurrealValue::from_value(obj.remove("elo").unwrap_or(Value::None))?,
                best_elo: SurrealValue::from_value(obj.remove("best_elo").unwrap_or(Value::None))?,
                games_played: SurrealValue::from_value(
                    obj.remove("games_played").unwrap_or(Value::None),
                )?,
                games_won: SurrealValue::from_value(
                    obj.remove("games_won").unwrap_or(Value::None),
                )?,
                games_lost: SurrealValue::from_value(
                    obj.remove("games_lost").unwrap_or(Value::None),
                )?,
                win_ratio: SurrealValue::from_value(
                    obj.remove("win_ratio").unwrap_or(Value::None),
                )?,
                rank: None,
            }),
            other => <()>::from_value(other).map(|_| unreachable!()),
        }
    }
}
