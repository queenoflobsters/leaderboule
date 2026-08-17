use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

pub mod current_user {

    use super::*;

    /// User database model
    #[derive(Serialize, Deserialize, Clone, Default)]
    pub struct UserProfile {
        pub email: String,
        pub username: String,
        pub elo: u64,
        pub games_played: u64,
        pub games_won: u64,
        pub games_lost: u64,
        pub win_ratio: u64,
    }

    #[server]
    pub async fn get_username() -> Result<Option<String>, ServerFnError> {
        use crate::server::{auth, db};
        if let Some(session_record) = auth::session::get_from_extension() {
            let db = db::get().await;
            let username: Option<String> = db
                .query("SELECT VALUE username FROM ONLY $user_id")
                .bind(("user_id", session_record.user_id.0))
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?
                .take(0)
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            Ok(username)
        } else {
            Ok(None)
        }
    }

    #[server]
    pub async fn get_profile() -> Result<Option<UserProfile>, ServerFnError> {
        use crate::server::{auth, db};
        if let Some(session_record) = auth::session::get_from_extension() {
            let db = db::get().await;
            let user_profile: Option<UserProfile> = db
                .select(&session_record.user_id.0)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            Ok(user_profile)
        } else {
            Ok(None)
        }
    }
}

pub mod global {


use super::*;

    #[derive(Serialize, Deserialize, PartialEq, Clone)]
    pub struct LeaderboardUserCard {
        pub username: String, // maybe later use the Rc<str> thing ?
        pub elo: u64,
        pub games_played: u64,
        pub games_won: u64,
        pub games_lost: u64,
        pub win_ratio: u64,
    }


    #[derive(Serialize, Deserialize, PartialEq, Clone)]
    pub enum LeaderboardSortMethod {
        Elo,
        GamesPlayed,
        GamesWon,
        WinRatio,
    }

    #[server]
    pub async fn get_leaderboard_cards(
        search_query: String,
        sort_method: LeaderboardSortMethod,
        page: u64,
        page_size: u64,
    ) -> Result<Vec<LeaderboardUserCard>, ServerFnError> {
        use crate::server::{auth, db};
        if auth::session::get_from_extension().is_none() {
            return Ok(vec![]);
        }
        db::get_leaderboard_cards(search_query, sort_method, page, page_size).await
    }
}
