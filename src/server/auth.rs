use crate::{
    client::route::Route,
    server::{
        db::{self, SessionToken},
        utils::{self, THIRTY_DAYS_IN_SECS},
    },
};
use dioxus::{
    fullstack::{response::IntoResponse, FullstackContext, Redirect},
    prelude::*,
    server::{
        axum::{
            self, extract,
            http::{header::SET_COOKIE, HeaderValue},
            middleware, response,
        },
        ServerFnError,
    },
};
use reqwest::header::HeaderMap;
use std::str::FromStr;

// // may be useful sometimes
// fn parse_cookie(headers: &HeaderMap, key: &str) -> Option<String> {
//     let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
//     // Split by ';'
//     for cookie in cookie_header.split(';') {
//         // Split by '=' in two parts
//         let mut parts = cookie.trim().splitn(2, '=');
//         if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
//             if k == key {
//                 return Some(SessionToken(v.parse().ok()?));
//             }
//         }
//     }
//     None
// }

/// Helper function to parse session_token cookie from raw headers
pub fn parse_session_token_cookie(headers: &HeaderMap) -> Option<SessionToken> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    // Split by ';'
    for cookie in cookie_header.split(';') {
        // Split by '=' in two parts
        let mut parts = cookie.trim().splitn(2, '=');
        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
            if k == "session_token" {
                return Some(SessionToken(v.parse().ok()?));
            }
        }
    }
    None
}

/// Generate cookie header and modify response via context
pub fn add_cookie_to_response(session_token: SessionToken) -> Result<(), ServerFnError> {
    // Generate cookie header directly
    let cookie_str = format!(
        "session_token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        session_token.0, THIRTY_DAYS_IN_SECS
    );

    // Modify response headers via context
    let fullstack_ctx =
        FullstackContext::current().ok_or(ServerFnError::new("Unable to get FullstackContext"))?;
    let header_val =
        HeaderValue::from_str(&cookie_str).map_err(|e| ServerFnError::new(e.to_string()))?;
    fullstack_ctx.add_response_header(SET_COOKIE, header_val);

    Ok(())
}

/// Expire cookie by generating clear one
pub fn clear_cookie_from_response() -> Result<(), ServerFnError> {
    let clear_cookie = "session_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

    if let Some(ctx) = FullstackContext::current() {
        if let Ok(val) = HeaderValue::from_str(&clear_cookie) {
            ctx.add_response_header(SET_COOKIE, val);
        }
    }

    Ok(())
}

/// Axum Middleware: Populates AuthUser extension if cookie is valid.
pub async fn server_auth_guard(
    mut req: extract::Request,
    next: middleware::Next,
) -> response::Response {
    let path = req.uri().path();

    // 1. Ignore public paths
    if is_path_public(path) {
        return next.run(req).await;
    }

    // 2. Check for authentification
    if let Some(session_token) = is_cookie_authenticated(req.headers()).await {
        // Insert axum extension if authed
        req.extensions_mut().insert(session_token);
        return next.run(req).await;
    }

    // 3. Redirect if nor authed nor public
    return Redirect::to(&Route::Login.to_string()).into_response();
}

async fn is_cookie_authenticated(req: &HeaderMap) -> Option<SessionToken> {
    if let Some(session_token) = parse_session_token_cookie(req) {
        let db = db::get().await;
        let session_match: Option<db::SessionRecord> =
            db.select(("sessions", session_token.0)).await.ok().flatten();
        if let Some(session) = session_match {
            if session.expires_at > utils::current_time_secs() {
                return Some(session.session_token);
            }
        } else {
            let _deleted: Option<db::SessionRecord> =
                db.delete(("sessions", session_token.0)).await.ok().flatten();
        }
    }
    None
}

fn is_path_public(path: &str) -> bool {
    // 1. public assets and folders
    if path.starts_with("/public")
        || path.starts_with("/_dioxus")
        || path.starts_with("/assets")
        || path.starts_with("/wasm")
    {
        return true;
    }

    // 2. Any path ending in a file extension
    // (This seem to be a horrible hack but it is what internet recommends...)
    if let Some(filename) = path.split('/').last() {
        if filename.contains('.') {
            return true;
        }
    }

    // 3. Public routes
    let route = Route::from_str(path).unwrap_or(Route::PageNotFound { segments: vec![] });
    if route.is_public() {
        return true;
    }

    false
}
