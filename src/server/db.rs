use dioxus::server::ServerFnError;
use serde::{Deserialize, Serialize};
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
    types::{RecordId, SurrealValue, Uuid},
    Surreal,
};
use tokio::sync::OnceCell;

use crate::{
    api::db::{
        current_user::{GameSearchItem, UserProfile},
        global::{GameSendItem, LeaderboardSortMethod, LeaderboardUserCard, UserSearchItem},
    },
    server::{
        auth::UserId,
        elo::{self, UserGameLog},
        utils,
    },
};

pub static DB: OnceCell<Surreal<Client>> = OnceCell::const_new();
const INIT_SQL: &'static str = include_str!("../../db_init.sql");
const USER_PAGE_SIZE: u64 = 8;
const GAME_REGISTRY_COOLDOWN: u64 = 15 * 60;

/// User database model
#[derive(Serialize, Deserialize, Clone, SurrealValue)]
pub struct UserRecord {
    pub id: UserId,
    pub email: String,
    pub username: String,
    pub member_since: u64,
    pub elo: u64,
    pub games_played: u64,
    pub games_won: u64,
}

#[derive(Serialize, Deserialize, Clone, SurrealValue)]
pub struct UserWithElo {
    pub id: UserId,
    pub elo: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, SurrealValue)]
#[serde(transparent)]
pub struct GameId(pub RecordId);

impl GameId {
    pub fn new_v7() -> Self {
        Self(RecordId::new("game", Uuid::new_v7()))
    }
}

// Game database model
#[derive(Serialize, Deserialize, Clone, SurrealValue)]
pub struct GameRecord {
    pub id: GameId,
    pub won_score: u64,
    pub lost_score: u64,
    pub players: Vec<UserGameLog>,
    pub recorded_by: UserId,
    pub played_at: u64,
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
        .bind(("limit", USER_PAGE_SIZE))
        .bind(("start", page * USER_PAGE_SIZE))
        .bind(("search", search_query))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fill with the rank
    for card in &mut cards {
        card.rank = Some(get_elo_rank(card.elo).await?)
    }

    Ok(cards)
}

pub async fn search_user(search_query: &str) -> Result<Vec<UserSearchItem>, ServerFnError> {
    let db = get().await;

    let items: Vec<UserSearchItem> = db
        .query("SELECT * FROM user WHERE username @1@ $search LIMIT $limit")
        .bind(("search", search_query))
        .bind(("limit", USER_PAGE_SIZE))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(items)
}

pub async fn get_elo_rank(elo: u64) -> Result<u64, ServerFnError> {
    let db = get().await;
    let higher_count: Option<u64> = db
        .query("count(SELECT VALUE id FROM user WHERE elo > $this_elo)+1")
        .bind(("this_elo", elo))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    higher_count.ok_or(ServerFnError::new(
        "Couldn't get the user's rank".to_string(),
    ))
}

pub async fn register_game(
    user_id: UserId,
    sent_game: GameSendItem,
) -> Result<Result<(), String>, ServerFnError> {
    let db = get().await;

    if let Some(game_record) = get_recent_game(&user_id).await? {
        let remaining_secs =
            game_record.played_at + GAME_REGISTRY_COOLDOWN - utils::current_time_secs();
        return Ok(Err(format!(
            "Tu pourras entrer une nouvelle partie dans {:.2}:{}",
            remaining_secs / 60,
            remaining_secs % 60
        )));
    }

    if let Err(e) = elo::verify_game(&sent_game) {
        return Ok(Err(e));
    }

    let [left_team_users, right_team_users] =
        match map_usernames(sent_game.left_team, sent_game.right_team).await? {
            Ok(vs) => vs,
            Err(e) => return Ok(Err(e)),
        };

    if left_team_users.iter().all(|u| u.id != user_id)
        && right_team_users.iter().all(|u| u.id != user_id)
    {
        return Ok(Err("L'utilisateur entrant la partie doit jouer".to_string()));
    }

    let updates = elo::compute_changes(
        sent_game.left_score,
        sent_game.right_score,
        left_team_users,
        right_team_users,
    );

    let game_id = GameId::new_v7();
    let won_score = std::cmp::max(sent_game.left_score, sent_game.right_score);
    let lost_score = std::cmp::min(sent_game.left_score, sent_game.right_score);
    let played_at = utils::current_time_secs();
    let game_record = GameRecord {
        id: game_id,
        lost_score,
        won_score,
        players: updates.clone(),
        recorded_by: user_id,
        played_at,
    };

    db.query(
        "BEGIN TRANSACTION;
            CREATE $game_id CONTENT $game_record;
            FOR $u IN $updates {
                UPDATE ONLY $u.id SET
                    elo = math::max([0, elo + $u.elo_change]),
                    games_played += 1,
                    games_won += IF $u.won THEN 1 ELSE 0 END;
            };
        COMMIT TRANSACTION;",
    )
    .bind(("updates", updates))
    .bind(("game_id", game_record.id.0.clone()))
    .bind(("game_record", game_record))
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(Ok(()))
}

async fn get_recent_game(user_id: &UserId) -> Result<Option<GameRecord>, ServerFnError> {
    let db = get().await;

    let cutoff_time = utils::current_time_secs() - GAME_REGISTRY_COOLDOWN;

    let recent_game: Option<GameRecord> = db
        .query(
            "SELECT * FROM ONLY game WHERE players.*.id CONTAINS $user_id AND played_at >= $cutoff_time ORDER BY played_at DESC LIMIT 1;",
        )
        .bind(("user_id", user_id.clone()))
        .bind(("cutoff_time", cutoff_time))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(recent_game)
}

async fn map_usernames(
    left_team: Vec<String>,
    right_team: Vec<String>,
) -> Result<Result<[Vec<UserWithElo>; 2], String>, ServerFnError> {
    let db = get().await;

    let left_team_len = left_team.len();
    let right_team_len = right_team.len();

    let mut response = db
        .query("SELECT id, elo FROM user WHERE username IN $left_team; SELECT id, elo FROM user WHERE username IN $right_team;")
        .bind(("left_team", left_team))
        .bind(("right_team", right_team))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let left_team_users: Vec<UserWithElo> = response
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let right_team_users: Vec<UserWithElo> = response
        .take(1)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if left_team_users.len() != left_team_len || right_team_users.len() != right_team_len {
        return Ok(Err(
            "Un ou plusieurs joueurs n'existent pas dans la base de données".to_string(),
        ));
    }

    Ok(Ok([left_team_users, right_team_users]))
}

pub async fn get_game_history(user_id: UserId) -> Result<Vec<GameSearchItem>, ServerFnError> {
    todo!()
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
                member_since: SurrealValue::from_value(
                    obj.remove("member_since").unwrap_or(Value::None),
                )?,
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
impl SurrealValue for UserSearchItem {
    fn kind_of() -> surrealdb::types::Kind {
        surrealdb::types::Kind::Any
    }

    fn into_value(self) -> surrealdb::types::Value {
        panic!("You should never write a UserSearchItem into the database");
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
            }),
            other => <()>::from_value(other).map(|_| unreachable!()),
        }
    }
}
