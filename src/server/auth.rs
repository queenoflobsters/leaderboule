use crate::{
    api::auth,
    server::{db, utils},
};
use dioxus::server::axum::{self, extract, middleware, response};

/// Helper function to parse cookie string from raw headers
pub fn parse_cookie(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for cookie in cookie_header.split(';') {
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
pub async fn check_helloasso_adherent(email: &str) -> Result<bool, String> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        return Ok(false);
    }
    Ok(true)
}

/// Axum Middleware: Populates AuthUser extension if cookie is valid.
pub async fn middleware(mut req: extract::Request, next: middleware::Next) -> response::Response {
    if let Some(token) = parse_cookie(req.headers(), "session_token") {
        let db = db::get().await;

        // TODO make this prettier
        let session: Option<db::SessionRecord> =
            db.select(("session", token)).await.ok().flatten();

        if let Some(session) = session {
            if session.expires_at > utils::current_time_secs() {
                req.extensions_mut().insert(auth::AuthUser {
                    email: session.email,
                });
            }
        }
    }

    next.run(req).await
}
