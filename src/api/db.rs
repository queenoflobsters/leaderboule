use dioxus::prelude::*;

pub mod current_user {
    use super::*;

    #[server]
    pub async fn get_username() -> Result<Option<String>, ServerFnError> {
        use crate::server::{auth, db};
        if let Some(session_record) = auth::get_session_record_extension() {
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
}
