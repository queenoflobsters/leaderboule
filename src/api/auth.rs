use dioxus::prelude::*;

const THIRTY_DAYS_IN_SECS: u64 = 30 * 24 * 3600;


/// User identity inserted into Axum Request Extensions when authenticated
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub email: String,
}

/// Login Server Function
#[server]
pub async fn login(email: String) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::{self, db, utils};
    use dioxus::server::axum::http::{header::SET_COOKIE, HeaderValue};
    use uuid::Uuid;

    // 1. Check HelloAsso
    let is_adherent = server::auth::check_helloasso_adherent(&email)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !is_adherent {
        return Ok(Err("Email non enregistré chez HelloAsso.".to_string()));
    }

    // 2. Generate session and insert into SurrealDB without SQL (.create())
    let token = Uuid::new_v4().to_string();
    let db = db::get().await;
    let expires_at = utils::current_time_secs() + THIRTY_DAYS_IN_SECS;

    let session = db::SessionRecord {
        token: token.clone(),
        email: email.clone(),
        expires_at,
    };

    let _created: Option<db::SessionRecord> = db
        .create(("session", token.clone()))
        .content(session)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // 3. Set-Cookie header directly without tower_cookies
    let cookie_str =
        format!("session_token={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000");


    // Modify response headers via server_context()
    if let Some(ctx) = FullstackContext::current() {
        if let Ok(val) = HeaderValue::from_str(&cookie_str) {
            ctx.add_response_header(SET_COOKIE, val);
        }
    }
    Ok(Ok(()))
}

/// Logout Server Function
#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    use crate::server::{auth, db};
    use dioxus::server::axum;
    use dioxus::server::axum::http::{header::SET_COOKIE, HeaderValue};

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
    // if let Ok(mut res_parts) = FullstackContext::extract::<axum::http::response::Parts, _>().await {
    //     if let Ok(header_val) = axum::http::HeaderValue::from_str(clear_cookie) {
    //         res_parts
    //             .headers
    //             .append(axum::http::header::SET_COOKIE, header_val);
    //     }
    // }
    if let Some(ctx) = FullstackContext::current() {
        if let Ok(val) = HeaderValue::from_str(&clear_cookie) {
            ctx.add_response_header(SET_COOKIE, val);
        }
    }

    Ok(())
}

/// Get currently authenticated user email
#[server]
pub async fn get_current_user() -> Result<Option<String>, ServerFnError> {
    use dioxus::server::axum;

    let auth_user = FullstackContext::extract::<axum::Extension<AuthUser>, _>().await;

    match auth_user {
        Ok(axum::Extension(user)) => Ok(Some(user.email)),
        Err(_) => Ok(None),
    }
}
