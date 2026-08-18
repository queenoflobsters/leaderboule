use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

pub mod current_user {

    use super::*;

    /// User database model
    #[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
    pub struct UserProfile {
        pub email: String,
        pub username: String,
        pub member_since: u64,
        pub elo: u64,
        pub best_elo: u64,
        pub games_played: u64,
        pub games_won: u64,
        pub games_lost: u64,
        pub win_ratio: f32,
        pub rank: Option<u64>,
    }

    #[server]
    pub async fn get_username() -> Result<String, ServerFnError> {
        use crate::server::{auth, db};
        let Some(session_record) = auth::session::get_from_extension() else {
            return Err(ServerFnError::new("User is not authenticated"));
        };
        let db = db::get().await;
        let username_opt: Option<String> = db
            .query("SELECT VALUE username FROM ONLY $user_id")
            .bind(("user_id", session_record.user_id.0))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .take(0)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let username = username_opt.ok_or(ServerFnError::new(
            "Did not find the user's username".to_string(),
        ))?;
        Ok(username)
    }

    #[server]
    pub async fn get_profile() -> Result<UserProfile, ServerFnError> {
        use crate::server::{auth, db};
        let Some(session_record) = auth::session::get_from_extension() else {
            return Err(ServerFnError::new("User is not authenticated"));
        };
        let db = db::get().await;
        let mut user_profile: UserProfile = db
            .select(&session_record.user_id.0)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .ok_or(ServerFnError::new(
                "Did not find the user's profile".to_string(),
            ))?;
        // Fill the rank
        user_profile.rank = Some(db::get_elo_rank(user_profile.elo).await?);
        Ok(user_profile)
    }
}

pub mod global {

    use super::*;

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
