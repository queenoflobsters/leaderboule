use std::{collections::HashMap, time::Duration};

use dioxus::server::ServerFnError;
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, DbErr,
    EntityTrait, FromQueryResult, JoinType, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
    RelationTrait, Set, Statement, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use tokio::sync::OnceCell;
use uuid::Uuid;

use crate::{
    api::db::{
        current_user::{GameSearchItem, UserProfile},
        global::{
            GameSendItem, LeaderboardSortMethod, LeaderboardUserCard, PlayerGameLog, UserSearchItem,
        },
    },
    server::{
        elo,
        entities::{game, game_player, user},
        migration::Migrator,
        utils,
    },
};

/// Référence à la base de données.
pub static DB: OnceCell<DatabaseConnection> = OnceCell::const_new();

const USER_PAGE_SIZE: u64 = 8;
const GAME_PAGE_SIZE: u64 = 5;
const GAME_REGISTRY_COOLDOWN: u64 = 15 * 60;

/// Petite structure pour le calcul d'elo.
#[derive(Clone)]
pub struct UserWithElo {
    pub id: Uuid,
    pub elo: u64,
}

/// Initialise la connexion PostgreSQL et applique les migrations en attente.
pub async fn get() -> &'static DatabaseConnection {
    DB.get_or_init(|| async {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://leaderboule:leaderboule@127.0.0.1:5432/leaderboule".to_string()
        });

        let mut options = ConnectOptions::new(database_url);
        options
            .max_connections(10)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(10))
            .sqlx_logging(false);

        let db = Database::connect(options)
            .await
            .expect("Impossible de se connecter à PostgreSQL");

        Migrator::up(&db, None)
            .await
            .expect("Impossible d'appliquer les migrations PostgreSQL");

        db
    })
    .await
}

/// Convertit une erreur SeaORM en erreur de server function.
pub fn db_err(error: DbErr) -> ServerFnError {
    ServerFnError::new(error.to_string())
}

/// Récupère un utilisateur par son identifiant.
pub async fn get_user(user_id: Uuid) -> Result<Option<user::Model>, ServerFnError> {
    user::Entity::find_by_id(user_id)
        .one(get().await)
        .await
        .map_err(db_err)
}

/// Récupère un utilisateur par son adresse e-mail.
pub async fn get_user_by_email(email: &str) -> Result<Option<user::Model>, ServerFnError> {
    user::Entity::find()
        .filter(user::Column::Email.eq(email))
        .one(get().await)
        .await
        .map_err(db_err)
}

/// Récupère le nom d'utilisateur d'un utilisateur.
pub async fn get_username(user_id: Uuid) -> Result<Option<String>, ServerFnError> {
    user::Entity::find_by_id(user_id)
        .select_only()
        .column(user::Column::Username)
        .into_tuple::<String>()
        .one(get().await)
        .await
        .map_err(db_err)
}

/// Récupère le profil complet d'un utilisateur pour la page Mon Compte.
pub async fn get_profile(user_id: Uuid) -> Result<UserProfile, ServerFnError> {
    let user = get_user(user_id)
        .await?
        .ok_or_else(|| ServerFnError::new("Utilisateur introuvable"))?;

    let games_lost = user.games_lost();
    let win_ratio = user.win_ratio();
    let rank = get_elo_rank(user.elo as u64).await?;

    Ok(UserProfile {
        email: user.email,
        username: user.username,
        member_since: user.member_since as u64,
        elo: user.elo as u64,
        best_elo: user.best_elo as u64,
        games_played: user.games_played as u64,
        games_won: user.games_won as u64,
        games_lost,
        win_ratio,
        rank: Some(rank),
    })
}

/// Parle avec la base de données et récupère les infos sur les utilisateurs.
pub async fn get_leaderboard_cards(
    search_query: String,
    sort_method: LeaderboardSortMethod,
    current_page: u64,
) -> Result<Vec<LeaderboardUserCard>, ServerFnError> {
    // Le rang est calculé sur la totalité des joueurs, puis la recherche et la
    // pagination sont appliquées : le classement affiché reste donc global.
    let sort_clause = match sort_method {
        LeaderboardSortMethod::Elo => "elo DESC, games_played DESC",
        LeaderboardSortMethod::GamesPlayed => "games_played DESC, elo DESC",
        LeaderboardSortMethod::GamesWon => "games_won DESC, elo DESC",
        LeaderboardSortMethod::WinRatio => "win_ratio DESC, elo DESC",
    };

    let sql = format!(
        r#"
        WITH ranked_users AS (
            SELECT
                username,
                elo,
                best_elo,
                games_played,
                games_won,
                games_lost,
                win_ratio,
                RANK() OVER (ORDER BY elo DESC) AS rank
            FROM users
        )
        SELECT * FROM ranked_users
        WHERE ($1 = '' OR username ILIKE $2)
        ORDER BY {sort_clause}
        LIMIT $3 OFFSET $4
        "#
    );

    let pattern = format!("%{}%", escape_like(&search_query));
    let offset = current_page.saturating_mul(USER_PAGE_SIZE) as i64;

    let rows = LeaderboardRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        [
            search_query.into(),
            pattern.into(),
            (USER_PAGE_SIZE as i64).into(),
            offset.into(),
        ],
    ))
    .all(get().await)
    .await
    .map_err(db_err)?;

    Ok(rows
        .into_iter()
        .map(|row| LeaderboardUserCard {
            username: row.username,
            elo: row.elo.max(0) as u64,
            best_elo: row.best_elo.max(0) as u64,
            games_played: row.games_played.max(0) as u64,
            games_won: row.games_won.max(0) as u64,
            games_lost: row.games_lost.max(0) as u64,
            win_ratio: row.win_ratio as f32,
            rank: Some(row.rank.max(1) as u64),
        })
        .collect())
}

#[derive(FromQueryResult)]
struct LeaderboardRow {
    username: String,
    elo: i64,
    best_elo: i64,
    games_played: i64,
    games_won: i64,
    games_lost: i64,
    win_ratio: f64,
    rank: i64,
}

/// Recherche un utilisateur dans la base de données.
/// Retourne une liste de `USER_PAGE_SIZE` utilisateurs qui correspondent à la recherche.
pub async fn search_user(search_query: &str) -> Result<Vec<UserSearchItem>, ServerFnError> {
    let pattern = format!("%{}%", escape_like(search_query));

    let rows = user::Entity::find()
        .select_only()
        .column(user::Column::Username)
        .column(user::Column::Elo)
        .filter(user::Column::Username.ilike(pattern))
        .order_by_desc(user::Column::Elo)
        .limit(USER_PAGE_SIZE)
        .into_tuple::<(String, i64)>()
        .all(get().await)
        .await
        .map_err(db_err)?;

    Ok(rows
        .into_iter()
        .map(|(username, elo)| UserSearchItem {
            username,
            elo: elo.max(0) as u64,
        })
        .collect())
}

/// Retourne le classement à partir de l'elo, il n'y a que besoin de l'elo.
pub async fn get_elo_rank(elo: u64) -> Result<u64, ServerFnError> {
    let higher_count = user::Entity::find()
        .filter(user::Column::Elo.gt(elo as i64))
        .count(get().await)
        .await
        .map_err(db_err)?;

    Ok(higher_count + 1)
}

/// Enregistre une partie dans la base de données.
pub async fn register_game(
    user_id: Uuid,
    sent_game: GameSendItem,
) -> Result<Result<(), String>, ServerFnError> {
    if let Some(recent_game) = get_recent_game(user_id).await? {
        let remaining_secs = (recent_game.played_at as u64 + GAME_REGISTRY_COOLDOWN)
            .saturating_sub(utils::current_time_secs());
        return Ok(Err(format!(
            "Tu pourras entrer une nouvelle partie dans {:02}:{:02}",
            remaining_secs / 60,
            remaining_secs % 60
        )));
    }

    if let Err(e) = elo::verify_game(&sent_game) {
        return Ok(Err(e));
    }

    let [left_team_users, right_team_users] =
        match map_usernames(sent_game.left_team, sent_game.right_team).await? {
            Ok(teams) => teams,
            Err(e) => return Ok(Err(e)),
        };

    if !left_team_users.iter().any(|user| user.id == user_id)
        && !right_team_users.iter().any(|user| user.id == user_id)
    {
        return Ok(Err("L'utilisateur entrant la partie doit jouer".to_string()));
    }

    let updates = elo::compute_changes(
        sent_game.left_score,
        sent_game.right_score,
        left_team_users,
        right_team_users,
    );

    let game_id = Uuid::now_v7();
    let won_score = sent_game.left_score.max(sent_game.right_score);
    let lost_score = sent_game.left_score.min(sent_game.right_score);
    let played_at = utils::current_time_secs();
    let game = game::ActiveModel {
        id: Set(game_id),
        won_score: Set(won_score as i64),
        lost_score: Set(lost_score as i64),
        recorded_by: Set(user_id),
        played_at: Set(played_at as i64),
    };

    let db = get().await;
    let transaction = db.begin().await.map_err(db_err)?;

    game::Entity::insert(game)
        .exec(&transaction)
        .await
        .map_err(db_err)?;

    game_player::Entity::insert_many(updates.iter().map(|update| game_player::ActiveModel {
        game_id: Set(game_id),
        user_id: Set(update.id),
        elo_change: Set(update.elo_change),
        won: Set(update.won),
    }))
    .exec(&transaction)
    .await
    .map_err(db_err)?;

    for update in &updates {
        // Les compteurs et l'elo sont mis à jour de façon atomique pour éviter
        // qu'une partie simultanée n'écrase les statistiques de l'autre.
        transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                UPDATE users
                SET elo = GREATEST(0, elo + $1),
                    games_played = games_played + 1,
                    games_won = games_won + $2
                WHERE id = $3
                "#,
                [
                    update.elo_change.into(),
                    i64::from(update.won).into(),
                    update.id.into(),
                ],
            ))
            .await
            .map_err(db_err)?;
    }

    transaction.commit().await.map_err(db_err)?;

    Ok(Ok(()))
}

/// Vérifie si un utilisateur a rentré une partie récemment, pour éviter les spams.
async fn get_recent_game(user_id: Uuid) -> Result<Option<game::Model>, ServerFnError> {
    let cutoff_time = utils::current_time_secs().saturating_sub(GAME_REGISTRY_COOLDOWN) as i64;

    game::Entity::find()
        .join(JoinType::InnerJoin, game::Relation::GamePlayer.def())
        .filter(game_player::Column::UserId.eq(user_id))
        .filter(game::Column::PlayedAt.gte(cutoff_time))
        .order_by_desc(game::Column::PlayedAt)
        .one(get().await)
        .await
        .map_err(db_err)
}

/// Associe à chaque nom d'utilisateur son `user_id` et son `elo`,
/// pour le calcul d'elo par la suite.
async fn map_usernames(
    left_team: Vec<String>,
    right_team: Vec<String>,
) -> Result<Result<[Vec<UserWithElo>; 2], String>, ServerFnError> {
    let usernames: Vec<String> = left_team.iter().chain(right_team.iter()).cloned().collect();

    let users = user::Entity::find()
        .filter(user::Column::Username.is_in(usernames))
        .all(get().await)
        .await
        .map_err(db_err)?;

    let mut users_by_name: HashMap<String, UserWithElo> = users
        .into_iter()
        .map(|user| {
            (
                user.username,
                UserWithElo {
                    id: user.id,
                    elo: user.elo.max(0) as u64,
                },
            )
        })
        .collect();

    let take_team = |names: Vec<String>, users_by_name: &mut HashMap<String, UserWithElo>| {
        names
            .into_iter()
            .map(|name| users_by_name.remove(&name))
            .collect::<Option<Vec<_>>>()
    };

    let Some(left_team_users) = take_team(left_team, &mut users_by_name) else {
        return Ok(Err(
            "Un ou plusieurs joueurs n'existent pas dans la base de données".to_string(),
        ));
    };
    let Some(right_team_users) = take_team(right_team, &mut users_by_name) else {
        return Ok(Err(
            "Un ou plusieurs joueurs n'existent pas dans la base de données".to_string(),
        ));
    };

    Ok(Ok([left_team_users, right_team_users]))
}

/// Récupère l'historique des parties d'un utilisateur.
pub async fn get_game_history(
    user_id: Uuid,
    current_page: u64,
) -> Result<Vec<GameSearchItem>, ServerFnError> {
    let offset = current_page.saturating_mul(GAME_PAGE_SIZE) as i64;

    let games = GameHistoryRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT g.id, gp.elo_change, g.won_score, g.lost_score, g.played_at
        FROM games g
        JOIN game_players gp ON gp.game_id = g.id
        WHERE gp.user_id = $1
        ORDER BY g.played_at DESC
        LIMIT $2 OFFSET $3
        "#,
        [
            user_id.into(),
            (GAME_PAGE_SIZE as i64).into(),
            offset.into(),
        ],
    ))
    .all(get().await)
    .await
    .map_err(db_err)?;

    if games.is_empty() {
        return Ok(Vec::new());
    }

    let game_ids: Vec<Uuid> = games.iter().map(|game| game.id).collect();
    let participants = game_player::Entity::find()
        .find_also_related(user::Entity)
        .filter(game_player::Column::GameId.is_in(game_ids))
        .all(get().await)
        .await
        .map_err(db_err)?;

    let mut players_by_game: HashMap<Uuid, (Vec<PlayerGameLog>, Vec<PlayerGameLog>)> =
        HashMap::new();

    for (participation, user) in participants {
        let Some(user) = user else {
            continue;
        };
        let player = PlayerGameLog {
            username: user.username,
            elo_change: participation.elo_change,
        };
        let entry = players_by_game.entry(participation.game_id).or_default();
        if participation.won {
            entry.0.push(player);
        } else {
            entry.1.push(player);
        }
    }

    Ok(games
        .into_iter()
        .map(|game| {
            let (mut won_players, mut lost_players) =
                players_by_game.remove(&game.id).unwrap_or_default();
            won_players.sort_by(|a, b| a.username.cmp(&b.username));
            lost_players.sort_by(|a, b| a.username.cmp(&b.username));

            GameSearchItem {
                elo_change: game.elo_change,
                won_score: game.won_score.max(0) as u64,
                lost_score: game.lost_score.max(0) as u64,
                won_players,
                lost_players,
                played_at: game.played_at.max(0) as u64,
            }
        })
        .collect())
}

#[derive(FromQueryResult)]
struct GameHistoryRow {
    id: Uuid,
    elo_change: i64,
    won_score: i64,
    lost_score: i64,
    played_at: i64,
}

/// Échappe les caractères spéciaux d'un motif `LIKE`/`ILIKE`.
fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
