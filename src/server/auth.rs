use crate::{
    api::{auth},
    server::{
        db,
        utils::{self, THIRTY_DAYS_IN_SECS},
    },
};
use dioxus::{
    prelude::*,
    fullstack::FullstackContext,
    server::{
        axum::{
            self, extract,
            http::{header::SET_COOKIE, HeaderValue},
            middleware, response,
        },
        ServerFnError,
    },
};
use uuid::Uuid;

/// Helper function to parse cookie string from raw headers
pub fn parse_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    // Split by ';'
    for cookie in cookie_header.split(';') {
        // Split by '=' in two parts
        let mut parts = cookie.trim().splitn(2, '=');
        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
            if k == name {
                return Some(v.to_string());
            }
        }
    }
    None
}

// TODO Implement this
pub async fn is_helloasso_adherent(email: &str) -> Result<bool, String> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        return Ok(false);
    }
    Ok(true)
}

/// Generate session and insert into SurrealDB
pub async fn save_session_record(email: String) -> Result<Uuid, ServerFnError> {
    let token = Uuid::new_v4();
    let db = db::get().await;
    let expires_at = utils::current_time_secs() + utils::THIRTY_DAYS_IN_SECS;

    let session = db::SessionRecord {
        token: token.clone(),
        email,
        expires_at,
    };

    let _created: Option<db::SessionRecord> = db
        .create(("session", token.to_string()))
        .content(session)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(token)
}

/// Gemerate cookie header and modify response via context
pub fn add_cookie_to_response(token: uuid::Uuid) -> Result<(), ServerFnError> {
    // Generate cookie header directly
    let cookie_str = format!(
        "session_token={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={THIRTY_DAYS_IN_SECS}"
    );

    // Modify response headers via context
    let fullstack_ctx =
        FullstackContext::current().ok_or(ServerFnError::new("Unable to get FullstackContext"))?;
    let header_val =
        HeaderValue::from_str(&cookie_str).map_err(|e| ServerFnError::new(e.to_string()))?;
    fullstack_ctx.add_response_header(SET_COOKIE, header_val);

    Ok(())
}

/// Axum Middleware: Populates AuthUser extension if cookie is valid.
pub async fn middleware(mut req: extract::Request, next: middleware::Next) -> response::Response {
    if let Some(token) = parse_cookie(req.headers(), "session_token") {
        let db = db::get().await;

        let session: Option<db::SessionRecord> = db.select(("session", token)).await.ok().flatten();

        if let Some(session) = session {
            if session.expires_at > utils::current_time_secs() {
                req.extensions_mut().insert(auth::AuthedUser {
                    email: session.email,
                });
            }
            // TODO delete entry when invalid
            // } else {
            //     db.delete(("session", token)).
            // }
        }
    }

    next.run(req).await
}

