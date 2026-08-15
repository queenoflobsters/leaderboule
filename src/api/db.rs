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
            let record: Option<db::UserRecord> = db
                .select(&session_record.user_id.0)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            Ok(record.map(UserProfile::from))
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
    }

    #[server]
    pub async fn get_leaderboard_cards(
        page: u64,
        page_size: u64,
    ) -> Result<Vec<LeaderboardUserCard>, ServerFnError> {
        use crate::server::{auth, db};
        if auth::session::get_from_extension().is_none() {
            return Ok(vec![]);
        }
        let db = db::get().await;
        let records: Vec<db::UserRecord> = db
            .query("SELECT * FROM user ORDER BY elo DESC LIMIT $limit START $start")
            .bind(("limit", page_size))
            .bind(("start", page * page_size))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .take(0)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        debug!("{}", records.len());
        let cards = records.into_iter().map(LeaderboardUserCard::from).collect();
        Ok(cards)
    }
}
