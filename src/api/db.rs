use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

/// Accéder aux données de l'utilisateur
pub mod current_user {

    use super::*;
    use crate::api::{
        db::global::PlayerGameLog,
        types::{UserRole, UserStatus},
    };

    /// Structure contenant des données sur l'utilisateur
    /// pour être afficher sur le page Mon Compte de l'utilisateur
    #[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
    pub struct UserProfile {
        pub email: String,
        pub username: String,
        pub member_since: u64,
        pub elo: u64,
        pub best_elo: u64, // maintenu par un trigger PostgreSQL
        pub games_played: u64,
        pub games_won: u64,
        pub games_lost: u64,   // colonne générée par PostgreSQL
        pub win_ratio: f32,    // colonne générée par PostgreSQL
        pub rank: Option<u64>, // calculé à la volée
    }

    /// Informations d'accès utilisées par la navigation et les gardes de route.
    #[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
    pub struct AccessInfo {
        pub username: String,
        pub role: UserRole,
        pub status: UserStatus,
        pub is_admin: bool,
    }

    /// Récupère le rôle effectif de l'utilisateur courant.
    #[server]
    pub async fn get_access() -> Result<AccessInfo, ServerFnError> {
        use crate::server::{access, auth, db};
        let Some(session_record) = auth::session::get_from_extension() else {
            return Err(ServerFnError::new("User is not authenticated"));
        };
        let user = db::get_user(session_record.user_id)
            .await?
            .ok_or_else(|| ServerFnError::new("Utilisateur introuvable"))?;
        let is_admin = access::is_admin(&user);
        Ok(AccessInfo {
            username: user.username,
            role: if is_admin {
                UserRole::Admin
            } else {
                UserRole::User
            },
            status: user.status,
            is_admin,
        })
    }

    /// Récupère le nom d'utilisateur de l'utilisateur dans la database
    #[server]
    pub async fn get_username() -> Result<String, ServerFnError> {
        use crate::server::{auth, db};
        let Some(session_record) = auth::session::get_from_extension() else {
            return Err(ServerFnError::new("User is not authenticated"));
        };
        let username = db::get_username(session_record.user_id)
            .await?
            .ok_or_else(|| ServerFnError::new("Did not find the user's username"))?;
        Ok(username)
    }

    /// Récupère le profile de l'utilisateur dans la databse
    #[server]
    pub async fn get_profile() -> Result<UserProfile, ServerFnError> {
        use crate::server::{auth, db};
        let Some(session_record) = auth::session::get_from_extension() else {
            return Err(ServerFnError::new("User is not authenticated"));
        };
        db::get_profile(session_record.user_id).await
    }

    /// Structure décrivant une partie,
    /// Pour être afficher sur l'historique
    #[derive(Serialize, Deserialize, Clone, PartialEq)]
    pub struct GameSearchItem {
        pub elo_change: i64,
        pub won_score: u64,
        pub lost_score: u64,
        pub won_players: Vec<PlayerGameLog>,
        pub lost_players: Vec<PlayerGameLog>,
        pub played_at: u64,
    }

    /// Récupère les parties de l'utilisateur depuis la databse
    #[server]
    pub async fn get_game_history(current_page: u64) -> Result<Vec<GameSearchItem>, ServerFnError> {
        use crate::server::{auth, db};
        let Some(session_record) = auth::session::get_from_extension() else {
            return Err(ServerFnError::new("User is not authenticated"));
        };

        db::get_game_history(session_record.user_id, current_page).await
    }
}

/// Accéder à des données globales
pub mod global {

    use super::*;

    /// Structure contenant les informations sur un utilisateur
    /// Pour être afficher sur la page Classement
    #[derive(Serialize, Deserialize, PartialEq, Clone)]
    pub struct LeaderboardUserCard {
        pub username: String, // maybe later use the Rc<str> thing ?
        pub elo: u64,
        pub best_elo: u64,
        pub games_played: u64,
        pub games_won: u64,
        pub games_lost: u64,
        pub win_ratio: f32,
        pub rank: Option<u64>,
    }

    /// Différentes méthodes de trier les utilisateurs dans la database
    #[derive(Serialize, Deserialize, PartialEq, Clone)]
    pub enum LeaderboardSortMethod {
        Elo,
        GamesPlayed,
        GamesWon,
        WinRatio,
    }

    /// Récupère les données des utilisateurs afin qu'elles soient affichées sur le classement
    #[server]
    pub async fn get_leaderboard_cards(
        search_query: String,
        sort_method: LeaderboardSortMethod,
        page: u64,
    ) -> Result<Vec<LeaderboardUserCard>, ServerFnError> {
        use crate::server::{auth, db};
        if auth::session::get_from_extension().is_none() {
            return Err(ServerFnError::new("User is not authenticated".to_string()));
        }
        db::get_leaderboard_cards(search_query, sort_method, page).await
    }

    /// Résultat de recherche d'un utilisateur dans le selecteur
    /// pour les nouvelles parties
    #[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
    pub struct UserSearchItem {
        pub username: String,
        pub elo: u64,
    }

    /// Structure représentant les performances d'un utilisateur durant une partie
    #[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
    pub struct PlayerGameLog {
        pub username: String,
        pub elo_change: i64,
    }

    /// Récupère une liste des utilisateurs qui correspondent à une recherche
    #[server]
    pub async fn search_user(search_query: String) -> Result<Vec<UserSearchItem>, ServerFnError> {
        use crate::server::{auth, db};
        if auth::session::get_from_extension().is_none() {
            return Err(ServerFnError::new("User is not authenticated".to_string()));
        }

        db::search_user(&search_query).await
    }

    /// Structure envoyée au serveur lors de la saisie d'une partie
    #[derive(Serialize, Deserialize, PartialEq, Clone)]
    pub struct GameSendItem {
        pub left_score: u64,
        pub right_score: u64,
        pub left_team: Vec<String>,
        pub right_team: Vec<String>,
    }

    impl GameSendItem {
        /// Construit un `GameSendItem` depuis les données récupérées de la database
        /// lors de la saisie d'une partie par l'utilisateur
        pub fn construct(
            left_score: u64,
            right_score: u64,
            left_team_members: &[UserSearchItem],
            right_team_members: &[UserSearchItem],
        ) -> Self {
            let left_team: Vec<String> = left_team_members
                .iter()
                .map(|item| item.username.clone())
                .collect();
            let right_team: Vec<String> = right_team_members
                .iter()
                .map(|item| item.username.clone())
                .collect();
            Self {
                left_score,
                right_score,
                left_team,
                right_team,
            }
        }
    }

    /// Enregistre une partie dans la database
    #[server]
    pub async fn register_game(game: GameSendItem) -> Result<Result<(), String>, ServerFnError> {
        use crate::server::{audit, auth, db};
        let Some(session_record) = auth::session::get_from_extension() else {
            return Err(ServerFnError::new("User is not authenticated"));
        };

        let result = db::register_game(session_record.user_id, game).await?;
        if result.is_ok() {
            audit::log_activity_best_effort(
                session_record.user_id,
                "game_recorded",
                "Partie enregistrée".to_string(),
            )
            .await;
        }
        Ok(result)
    }
}
