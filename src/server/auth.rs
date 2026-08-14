use crate::{
    client::route::Route,
    server::{
        db, elo, helloasso, utils::{self, THIRTY_DAYS_IN_SECS}
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
use serde::{Deserialize, Serialize};
use surrealdb::types::{RecordId, RecordIdKey, SurrealValue, Uuid};
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

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct SessionToken(pub RecordId);

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct UserId(pub RecordId);

impl SessionToken {
    /// Create a new SessionToken with a Uuid v7
    pub fn new_v7() -> Self {
        Self(RecordId::new("sessions", Uuid::new_v7()))
    }

    pub fn to_uuid(&self) -> Result<Uuid, ServerFnError> {
        match self.0.key {
            RecordIdKey::Uuid(uuid) => Ok(uuid),
            RecordIdKey::String(ref s) => Ok(s.parse().map_err(|e| ServerFnError::new(e))?),
            _ => Err(ServerFnError::new("Unable to parse SessionToken into Uuid")),
        }
    }
}

impl FromStr for SessionToken {
    // this is absolutly dreadful but honestly hilarous
    // I couldn't get the uuid crate from a surrealdb re-export
    // so I just converted it to a ServerFnError without even
    // having access to the type
    type Err = ServerFnError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SessionToken(RecordId::new(
            "session",
            s.parse::<Uuid>()
                .map_err(|e| ServerFnError::new(e.to_string()))?,
        )))
    }
}

impl UserId {
    /// Create a new UserId with a Uuid v7
    pub fn new_v7() -> Self {
        Self(RecordId::new("users", Uuid::new_v7()))
    }

    // pub fn to_uuid(&self) -> Result<Uuid, ServerFnError> {
    //     match self.0.key {
    //         RecordIdKey::Uuid(uuid) => Ok(uuid),
    //         RecordIdKey::String(ref s) => Ok(s.parse().map_err(|e| ServerFnError::new(e))?),
    //         _ => Err(ServerFnError::new("Unable to parse UserId into Uuid")),
    //     }
    // }
}

/// Session database model
#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct SessionRecord {
    pub session_token: SessionToken,
    pub user_id: UserId,
    pub expires_at: u64,
}

/// Helper function to parse session_token cookie from raw headers
pub fn parse_session_token_cookie(headers: &HeaderMap) -> Option<SessionToken> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    // Split by ';'
    for cookie in cookie_header.split(';') {
        // Split by '=' in two parts
        let mut parts = cookie.trim().splitn(2, '=');
        if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
            if k == "session_token" {
                return Some(v.parse().ok()?);
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
        session_token.to_uuid()?, THIRTY_DAYS_IN_SECS
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
        let session_match: Option<SessionRecord> =
            db.select(&session_token.0).await.ok().flatten();
        if let Some(session) = session_match {
            if session.expires_at > utils::current_time_secs() {
                return Some(session.session_token);
            }
        } else {
            let _deleted: Option<SessionRecord> =
                db.delete(&session_token.0).await.ok().flatten();
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
/// Generate session and insert into SurrealDB
pub async fn create_session_record(user_id: UserId) -> Result<SessionToken, ServerFnError> {
    // TODO MAKE THIS WITH UUID NOT EMAIL
    let session_token = SessionToken::new_v7();
    let db = db::get().await;
    let expires_at = utils::current_time_secs() + utils::THIRTY_DAYS_IN_SECS;

    let session = SessionRecord {
        session_token: session_token.clone(),
        user_id,
        expires_at,
    };

    let _created: Option<SessionRecord> = db
        .create(&session_token.0)
        .content(session)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(session_token)
}

/// Get the session_token from the axum context and delete it from DB
pub async fn delete_session_record() -> Result<(), ServerFnError> {
    if let Ok(req_headers) = FullstackContext::extract::<HeaderMap, _>().await {
        if let Some(session_token) = parse_session_token_cookie(&req_headers) {
            let _: Option<SessionRecord> = db::get()
                .await
                .delete(&session_token.0)
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?;
        }
    }

    Ok(())
}

/// Returns the user id
pub async fn has_account_or_create(email: &str) -> Result<Option<UserId>, ServerFnError> {
    // 1. Search inside the DB for user with this email
    info!("AUTH : searching inside db for user with email `{}`", email);
    let db = db::get().await;
    let user_match: Option<db::UserRecord> = db
        .query("SELECT * FROM users WHERE email = $email")
        .bind(("email", email))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if let Some(user) = user_match {
        info!("AUTH : user found in db, returning user.id");
        return Ok(Some(user.id));
    }

    // 2. Search if they are registered in Helloasso
    info!("AUTH : no user found in db");
    info!("AUTH : asking Helloasso if this user is an adherent");
    let helloasso_payer = helloasso::get_adherent(email).await?;

    match helloasso_payer {
        // There is no adherent with this email
        None => {
            info!("AUTH : Helloasso has no adherent with email `{}`", email);
            info!("AUTH : connexion refused");
            Ok(None)
        }
        // 3. There is an adherent with this email
        // Create an account and return the user id
        Some(payer) => {
            info!("AUTH : Helloasso has user with email `{}`", email);
            Ok(Some(create_user_from_payer(payer).await?))
        }
    }
}

/// Returns the user id
async fn create_user_from_payer(payer: helloasso::PayerInfo) -> Result<UserId, ServerFnError> {
    info!("AUTH : creating user with : {:?}", &payer);

    let db = db::get().await;

    let id = UserId::new_v7();
    let email = payer
        .email
        .ok_or(ServerFnError::new("Empty E-Mail field in helloasso answer"))?;
    let username = {
        match (payer.first_name, payer.last_name) {
            (None, None) => {
                let mut name_id = Uuid::new_v4().to_string();
                name_id.truncate(8);
                format!("Anonyme {name_id}")
            }
            (Some(first), None) => first,
            (None, Some(last)) => format!("Humain {last}"),
            (Some(first), Some(last)) => format!("{first} {last}"),
        }
    };

    let user_record = db::UserRecord {
        id: id.clone(),
        email,
        username,
        elo: elo::DEFAULT_ELO,
        games_played: 0,
        games_won: 0,
    };

    let _created: Option<db::UserRecord> = db
        .create(&id.0)
        .content(user_record)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    info!(
        "AUTH : successfully created user inside db with id {:?}",
        id
    );
    Ok(id)
}
