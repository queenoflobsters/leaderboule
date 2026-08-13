use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

/// User identity inserted into Axum Request Extensions when authenticated
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthedUser {
    pub email: String,
}

/// Login Server Function
#[server]
pub async fn login(email: String) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::{auth, db};
    
    if let Some(user_id) = db::has_account_or_create(&email).await? {
        let token = db::save_session_record(user_id).await?;
        auth::add_cookie_to_response(token)?;
        Ok(Ok(()))
    } else {
        Ok(Err("Email non enregistré chez HelloAsso.".to_string()))
    }

}

/// Logout Server Function
#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    use crate::server::{auth, db};
    use dioxus::server::axum::{
        self,
        http::{header::SET_COOKIE, HeaderValue},
    };

    // 1. Get the session_token from the axum context and delete it from DB
    if let Ok(req_headers) = FullstackContext::extract::<axum::http::HeaderMap, _>().await {
        if let Some(session_token) = auth::parse_session_token_cookie(&req_headers) {
            let db = db::get().await;

            // Delete record from SurrealDB without SQL (.delete())
            let _: Option<db::SessionRecord> = db
                .delete(("session", session_token.0))
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
    }

    // 2. Expire cookie by generating clear one
    let clear_cookie = "session_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

    if let Some(ctx) = FullstackContext::current() {
        if let Ok(val) = HeaderValue::from_str(&clear_cookie) {
            ctx.add_response_header(SET_COOKIE, val);
        }
    }

    Ok(())
}
