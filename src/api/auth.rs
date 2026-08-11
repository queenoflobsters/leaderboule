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
    use crate::server::{self, auth};

    // 1. Check HelloAsso
    let is_adherent = server::auth::is_helloasso_adherent(&email)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !is_adherent {
        return Ok(Err("Email non enregistré chez HelloAsso.".to_string()));
    }

    // Create Session Record
    let token = auth::save_session_record(email).await?;

    auth::add_cookie_to_response(token)?;

    Ok(Ok(()))
}

/// Logout Server Function
#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    use crate::server::{auth, db};
    use dioxus::server::axum::{
        self,
        http::{header::SET_COOKIE, HeaderValue},
    };

    if let Ok(req_headers) = FullstackContext::extract::<axum::http::HeaderMap, _>().await {
        if let Some(token) = auth::parse_cookie(&req_headers, "session_token") {
            let db = db::get().await;

            // Delete record from SurrealDB without SQL (.delete())
            let _: Option<db::SessionRecord> = db
                .delete(("session", token))
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
    }

    // Expire cookie
    let clear_cookie = "session_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

    if let Some(ctx) = FullstackContext::current() {
        if let Ok(val) = HeaderValue::from_str(&clear_cookie) {
            ctx.add_response_header(SET_COOKIE, val);
        }
    }

    Ok(())
}

/// Get currently authenticated user email
#[server]
pub async fn get_current_user() -> Result<Option<AuthedUser>, ServerFnError> {
    use dioxus::server::axum;

    // Extract the Extension from the context
    let authed_user = FullstackContext::extract::<axum::Extension<AuthedUser>, _>().await;

    match authed_user {
        Ok(axum::Extension(user)) => Ok(Some(user)),
        Err(_) => Ok(None),
    }
}
