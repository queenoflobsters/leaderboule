use crate::{
    client::route::Route,
    server::{db, elo, helloasso, utils},
};
use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};
use dioxus::{
    fullstack::{response::IntoResponse, FullstackContext, Redirect},
    prelude::*,
    server::{
        axum::{
            self,
            extract::Request,
            http::{header::SET_COOKIE, HeaderValue},
            middleware::Next,
            response::Response,
        },
        ServerFnError,
    },
};
use reqwest::{
    header::{HeaderMap, ACCEPT},
    Method,
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use surrealdb::types::{RecordId, RecordIdKey, SurrealValue, Uuid};

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
#[serde(transparent)]
pub struct SessionToken(pub RecordId);

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
#[serde(transparent)]
pub struct UserId(pub RecordId);

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
#[serde(transparent)]
pub struct UserCredId(pub RecordId);

impl SessionToken {
    /// Create a new SessionToken with a Uuid v7
    pub fn new_v7() -> Self {
        Self(RecordId::new("session", Uuid::new_v7()))
    }

    pub fn to_uuid(&self) -> Result<Uuid, ServerFnError> {
        match self.0.key {
            RecordIdKey::Uuid(uuid) => Ok(uuid),
            RecordIdKey::String(ref s) => Ok(s.parse().map_err(|e| ServerFnError::new(e))?),
            _ => Err(ServerFnError::new("Unable to parse SessionToken into Uuid")),
        }
    }
}

impl std::str::FromStr for SessionToken {
    // this is absolutly dreadful but honestly hilarous I couldn't get the uuid crate from a surrealdb re-export so I just converted it to a ServerFnError without even having access to the type
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
    // pub fn new_v7() -> (UserId, UserCredId) { let uuid = Uuid::new_v7(); let user_id = UserId(RecordId::new("user", uuid)); let user_cred_id = UserCredId(RecordId::new("user_cred", uuid)); (user_id, user_cred_id) }
    // pub fn to_uuid(&self) -> Result<Uuid, ServerFnError> { match self.0.key { RecordIdKey::Uuid(uuid) => Ok(uuid), RecordIdKey::String(ref s) => Ok(s.parse().map_err(|e| ServerFnError::new(e))?), _ => Err(ServerFnError::new("Unable to parse UserId into Uuid")), } }
    /// Create a new UserId with a Uuid v7
    pub fn new_v7() -> Self {
        Self(RecordId::new("user", Uuid::new_v7()))
    }
}

// impl From<UserId> for UserCredId { fn from(value: UserId) -> Self { Self(RecordId::new("user_cred", value.0.key)) } }
impl UserCredId {
    /// Create a new UserId with a Uuid v7
    pub fn new_v7() -> Self {
        Self(RecordId::new("user_cred", Uuid::new_v7()))
    }
}

/// Session database model
#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct SessionRecord {
    pub id: SessionToken,
    pub user_id: UserId,
    pub expires_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct UserCredRecord {
    pub id: UserCredId,
    pub user_id: UserId,
    pub password_hash: String,
}

pub enum UserAuthTry {
    Success(UserId),
    NonexistentCredentials(UserId),
    WrongPassword,
    NonexistentAccount,
}

pub mod session {
    use super::*;

    /// Helper functiparse_session_token_from_cookieon to parse session_token cookie from raw headers
    pub fn parse_token_from_cookie(headers: &HeaderMap) -> Option<SessionToken> {
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

    /// Get the session_token from the axum context and delete it from DB
    pub async fn delete() -> Result<(), ServerFnError> {
        let session_record = get_from_extension().ok_or(ServerFnError::new(
            "Unable to get SessionRecord from FullstackCtx",
        ))?;
        let _deleted: Option<SessionRecord> = db::get()
            .await
            .delete(&session_record.id.0)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(())
    }

    pub async fn get_from_db(session_token: &SessionToken) -> Option<SessionRecord> {
        let db = db::get().await;
        let session_match: Option<SessionRecord> = db.select(&session_token.0).await.ok().flatten();
        if let Some(session) = session_match {
            debug!("AUTH : found session_record");
            if session.expires_at > utils::current_time_secs() {
                return Some(session);
            } else {
                debug!("AUTH : session_record is expired");
                let _deleted: Option<SessionRecord> =
                    db.delete(&session_token.0).await.ok().flatten();
            }
        }

        debug!("AUTH : did not found session_token in db");
        None
    }

    pub fn get_from_extension() -> Option<SessionRecord> {
        let ctx = FullstackContext::current()?;
        ctx.extension::<SessionRecord>()
    }
    /// Generate session and insert into SurrealDB
    pub async fn create(user_id: UserId) -> Result<SessionToken, ServerFnError> {
        let session_token = SessionToken::new_v7();
        let db = db::get().await;
        let expires_at = utils::current_time_secs() + utils::THIRTY_DAYS_IN_SECS;

        let session = SessionRecord {
            id: session_token.clone(),
            user_id,
            expires_at,
        };

        let _created: Option<SessionRecord> = db
            .create(&session_token.0)
            .content(session.clone())
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        debug!("AUTH : created session record : {:?}", &session);

        Ok(session_token)
    }
}

pub mod cookie {
    use super::*;

    /// Generate cookie header and modify response via context
    pub fn create_in_response(session_token: SessionToken) -> Result<(), ServerFnError> {
        // Generate cookie header directly
        let cookie_str = format!(
            "session_token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
            session_token.to_uuid()?,
            utils::THIRTY_DAYS_IN_SECS
        );

        // Modify response headers via context
        let ctx = FullstackContext::current()
            .ok_or(ServerFnError::new("Unable to get FullstackContext"))?;
        let header_val =
            HeaderValue::from_str(&cookie_str).map_err(|e| ServerFnError::new(e.to_string()))?;
        ctx.add_response_header(SET_COOKIE, header_val);

        debug!(
            "AUTH : put session_token inside cookie : {:?}",
            session_token
        );

        Ok(())
    }

    /// Expire cookie by generating clear one
    pub fn clear_from_response() -> Result<(), ServerFnError> {
        let clear_cookie = "session_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

        if let Some(ctx) = FullstackContext::current() {
            if let Ok(val) = HeaderValue::from_str(&clear_cookie) {
                ctx.add_response_header(SET_COOKIE, val);
            }
        }

        Ok(())
    }
}

pub mod middleware {

    use super::*;

    /// Axum Middleware: Populates AuthUser extension if cookie is valid.
    pub async fn server_auth_guard(mut req: Request, next: Next) -> Response {
        let path = req.uri().path();
        debug!("GUARD : request to path : {}", path);

        // 1. Ignore public paths
        if is_static_asset(path) {
            debug!("GUARD : static asset, forwarding request");
            return next.run(req).await;
        }

        // 2. Check for authentification
        debug!("GUARD : checking for auth");
        if let Some(session_token) = session::parse_token_from_cookie(req.headers()) {
            debug!(
                "GUARD : session_token found in cookie : {:?}",
                &session_token
            );
            if let Some(session_record) = session::get_from_db(&session_token).await {
                // Insert axum extension if authed
                debug!("GUARD : session_record found in db, inserting axum extension and forwarding request");
                req.extensions_mut().insert(session_record);
                return next.run(req).await;
            }
        }

        // 3. Request is unauthenticated: redirect or forward based on navigation
        let is_nav = is_page_navigation(req.headers(), req.method());
        let route = Route::from_str(path).unwrap_or(Route::PageNotFound { segments: vec![] });
        if is_nav && !route.is_public() {
            return Redirect::to(&Route::Login.to_string()).into_response();
        }
        debug!("GUARD : public route, or api call, forwarding request");
        next.run(req).await
    }

    fn is_static_asset(path: &str) -> bool {
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

        false

        // 3. Public routes
    }

    fn is_page_navigation(headers: &HeaderMap, method: &Method) -> bool {
        // 1. Sec-Fetch-Mode is "navigate" for full page loads / URL bar entries
        if let Some(mode) = headers.get("sec-fetch-mode") {
            if mode == "navigate" {
                return true;
            }
        }

        // 2. Sec-Fetch-Dest is "document" for HTML documents
        if let Some(mode) = headers.get("sec-fetch-dest") {
            if mode == "document" {
                return true;
            }
        }

        // 3. Fallback for older clients: GET requests asking for text/html
        if method == Method::GET {
            if let Some(accept) = headers.get(ACCEPT).and_then(|h| h.to_str().ok()) {
                if accept.contains("text/html") {
                    return true;
                }
            }
        }

        false
    }
}

pub mod account {
    use super::*;

    /// Returns the user id
    /// 1. Search inside the DB for user with this email
    pub async fn exists(email: &str, password: &str) -> Result<UserAuthTry, ServerFnError> {
        debug!("AUTH : searching inside db for user with email `{}`", email);

        let Some(user_record) = get_user_record(email).await? else {
            debug!("AUTH : user not found in db");
            return Ok(UserAuthTry::NonexistentAccount);
        };

        debug!("AUTH : user found in db");

        credentials::verify(user_record.id, password).await
    }

    pub async fn get_user_record(email: &str) -> Result<Option<db::UserRecord>, ServerFnError> {
        let db = db::get().await;

        let user_match: Option<db::UserRecord> = db
            .query("SELECT * FROM user WHERE email = $email")
            .bind(("email", email))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .take(0)
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(user_match)
    }

    /// 2. Search if they are registered in Helloasso
    pub async fn try_create(email: &str, password: &str) -> Result<Option<UserId>, ServerFnError> {
        debug!("AUTH : asking Helloasso if `{}` is an adherent", email);
        let helloasso_payer = helloasso::get_adherent(email).await?;

        match helloasso_payer {
            // There is no adherent with this email
            None => {
                debug!("AUTH : Helloasso has no adherent with this email, connexion refused");
                Ok(None)
            }
            // 3. There is an adherent with this email
            // Create an account and return the user id
            Some(payer) => {
                debug!("AUTH : Helloasso has user with email `{}`", email);
                let user_id = create_user_record_in_db(payer).await?;
                credentials::create(&user_id, password)
                    .await?
                    .map_err(|s| ServerFnError::new(s))?;
                Ok(Some(user_id))
            }
        }
    }

    /// Returns the user id
    async fn create_user_record_in_db(
        payer: helloasso::PayerInfo,
    ) -> Result<UserId, ServerFnError> {
        debug!("AUTH : creating user with : {:?}", &payer);

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

        debug!(
            "AUTH : successfully created user inside db with id {:?}",
            id
        );
        Ok(id)
    }

    pub async fn change_username(
        user_id: &UserId,
        new_username: &str,
        password: &str,
    ) -> Result<Result<(), String>, ServerFnError> {
        let db = db::get().await;

        let UserAuthTry::Success(_) = credentials::verify(user_id.clone(), password).await? else {
            return Ok(Err("Mot de passe incorrect".to_string()));
        };

        let username_trimmed = new_username.trim();

        if let Err(e) = check_username_validity(username_trimmed).await? {
            return Ok(Err(e));
        }

        let updated: Option<db::UserRecord> = db
            .query("UPDATE ONLY $user_id SET username = $username")
            .bind(("user_id", user_id.clone()))
            .bind(("username", username_trimmed))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .take(0)
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        if updated.is_none() {
            return Err(ServerFnError::new(
                "Utilisateur voulant changer de nom d'utilisateur introuvable",
            ));
        }

        debug!(
            "AUTH : user `{:?}` changed username to `{}`",
            user_id, username_trimmed
        );

        Ok(Ok(()))
    }

    async fn check_username_validity(username: &str) -> Result<Result<(), String>, ServerFnError> {
        let db = db::get().await;

        if username.len() <= 3 {
            return Ok(Err("Nom d'utilisateur trop court".to_string()));
        }

        if username.len() >= 32 {
            return Ok(Err("Nom d'utilisateur trop long".to_string()));
        }

        let existing_user: Option<UserId> = db
            .query("SELECT VALUE id FROM ONLY user WHERE username = $username")
            .bind(("username", username))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .take(0)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        if existing_user.is_some() {
            return Ok(Err("Ce nom d'utilisateur est déjà utilisé".to_string()));
        }

        Ok(Ok(()))
    }
}

pub mod credentials {
    use super::*;

    pub fn create_hash(password: &str) -> Result<String, ServerFnError> {
        let salt = SaltString::generate(&mut OsRng);

        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(password_hash.to_string())
    }

    pub async fn verify(user_id: UserId, password: &str) -> Result<UserAuthTry, ServerFnError> {
        let db = db::get().await;

        let user_cred_record_match: Option<UserCredRecord> = db
            .query("SELECT * FROM ONLY user_cred WHERE user_id = $user_id")
            .bind(("user_id", user_id.clone()))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .take(0)
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let Some(user_cred_record) = user_cred_record_match else {
            return Ok(UserAuthTry::NonexistentCredentials(user_id));
        };

        let Ok(parsed_hash) = PasswordHash::new(&user_cred_record.password_hash) else {
            return Err(ServerFnError::new("Couldn't create hash from string in db"));
        };

        match Argon2::default().verify_password(password.as_bytes(), &parsed_hash) {
            Ok(()) => Ok(UserAuthTry::Success(user_id)),
            Err(_) => Ok(UserAuthTry::WrongPassword),
        }
    }

    pub async fn create(
        user_id: &UserId,
        password: &str,
    ) -> Result<Result<(), String>, ServerFnError> {
        if password.len() < 8 {
            return Ok(Err("Mot de passe trop court".to_string()));
        }
        let db = db::get().await;
        let user_cred_id = UserCredId::new_v7();
        let password_hash = credentials::create_hash(password)?;
        let user_cred_record = UserCredRecord {
            id: user_cred_id.clone(),
            user_id: user_id.clone(),
            password_hash,
        };

        let _created: Option<UserCredRecord> = db
            .create(&user_cred_id.0)
            .content(user_cred_record)
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        Ok(Ok(()))
    }

    pub async fn change_password(
        user_id: &UserId,
        old_password: &str,
        new_password: &str,
    ) -> Result<Result<(), String>, ServerFnError> {
        let db = db::get().await;

        let UserAuthTry::Success(_) = credentials::verify(user_id.clone(), old_password).await?
        else {
            return Ok(Err("Mot de passe incorrect".to_string()));
        };

        if new_password.len() < 8 {
            return Ok(Err("Mot de passe trop court".to_string()));
        }

        let new_password_hash = credentials::create_hash(new_password)?;

        let updated: Option<UserCredRecord> = db
            .query("UPDATE ONLY user_cred SET password_hash = $new_password_hash WHERE user_id = $user_id")
            .bind(("new_password_hash", new_password_hash))
            .bind(("user_id", user_id.clone().0))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .take(0)
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        if updated.is_none() {
            return Err(ServerFnError::new(
                "Cet utilisateur n'a aucun mot de passe enregistré. Absolument anormal",
            ));
        }

        Ok(Ok(()))
    }
}
