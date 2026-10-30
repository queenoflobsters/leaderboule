use crate::{
    client::route::Route,
    server::{
        db, elo,
        entities::{session as session_entity, user, user_cred},
        helloasso, utils,
    },
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
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// Identifiant d'une session utilisateur dans la base de données.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(transparent)]
pub struct SessionToken(pub Uuid);

impl SessionToken {
    /// Crée un nouveau `SessionToken` avec un UUID v7.
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }
}

impl std::fmt::Display for SessionToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for SessionToken {
    type Err = uuid::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

/// Résultat d'une tentative de connexion.
pub enum UserAuthTry {
    Success(Uuid),
    NonexistentCredentials(Uuid),
    WrongPassword,
    NonexistentAccount,
}

/// Gère la session de l'utilisateur.
///
/// - L'utilisateur se connecte et un `session_token` est créé.
/// - Le token est enregistré en base, associé à l'utilisateur.
/// - Le token est stocké dans un cookie.
/// - À chaque requête le middleware vérifie le cookie et retrouve la session.
pub mod session {
    use super::*;

    /// Récupère un `SessionToken` directement depuis les headers.
    pub fn parse_token_from_cookie(headers: &HeaderMap) -> Option<SessionToken> {
        let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;

        cookie_header.split(';').find_map(|cookie| {
            let (key, value) = cookie.trim().split_once('=')?;
            (key == "session_token")
                .then(|| value.parse().ok())
                .flatten()
        })
    }

    /// Supprime la session courante de la base de données.
    pub async fn delete() -> Result<(), ServerFnError> {
        let session_record = get_from_extension()
            .ok_or_else(|| ServerFnError::new("Unable to get SessionRecord from FullstackCtx"))?;

        session_entity::Entity::delete_by_id(session_record.id)
            .exec(db::get().await)
            .await
            .map_err(db::db_err)?;

        Ok(())
    }

    /// Récupère la session depuis la base de données.
    pub async fn get_from_db(session_token: &SessionToken) -> Option<session_entity::Model> {
        let session_match = session_entity::Entity::find_by_id(session_token.0)
            .one(db::get().await)
            .await
            .ok()
            .flatten()?;

        if session_match.expires_at > utils::current_time_secs() as i64 {
            debug!("AUTH : found session_record");
            Some(session_match)
        } else {
            debug!("AUTH : session_record is expired");
            let _ = session_entity::Entity::delete_by_id(session_match.id)
                .exec(db::get().await)
                .await;
            None
        }
    }

    /// Récupère le `SessionRecord` depuis l'extension axum,
    /// qui a été populée par le middleware.
    pub fn get_from_extension() -> Option<session_entity::Model> {
        let ctx = FullstackContext::current()?;
        ctx.extension::<session_entity::Model>()
    }

    /// Génère une session et la range dans la base de données.
    pub async fn create(user_id: Uuid) -> Result<SessionToken, ServerFnError> {
        let session_token = SessionToken::new_v7();
        let expires_at = (utils::current_time_secs() + utils::THIRTY_DAYS_IN_SECS) as i64;

        session_entity::ActiveModel {
            id: Set(session_token.0),
            user_id: Set(user_id),
            expires_at: Set(expires_at),
        }
        .insert(db::get().await)
        .await
        .map_err(db::db_err)?;

        debug!("AUTH : created session record : {:?}", session_token);

        Ok(session_token)
    }
}

/// Gestion des cookies.
///
/// On y stocke uniquement le `session_token`.
pub mod cookie {
    use super::*;

    /// Génère le header HTTP du cookie et le range dans la réponse.
    pub fn create_in_response(session_token: SessionToken) -> Result<(), ServerFnError> {
        let cookie_str = format!(
            "session_token={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
            session_token.0,
            utils::THIRTY_DAYS_IN_SECS
        );

        let ctx = FullstackContext::current()
            .ok_or_else(|| ServerFnError::new("Unable to get FullstackContext"))?;
        let header_val =
            HeaderValue::from_str(&cookie_str).map_err(|e| ServerFnError::new(e.to_string()))?;
        ctx.add_response_header(SET_COOKIE, header_val);

        debug!(
            "AUTH : put session_token inside cookie : {:?}",
            session_token
        );

        Ok(())
    }

    /// Péremption un cookie en générant un nouveau vide.
    pub fn clear_from_response() -> Result<(), ServerFnError> {
        let clear_cookie = "session_token=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";

        if let Some(ctx) = FullstackContext::current() {
            if let Ok(val) = HeaderValue::from_str(clear_cookie) {
                ctx.add_response_header(SET_COOKIE, val);
            }
        }

        Ok(())
    }
}

/// Middleware d'authentification.
pub mod middleware {

    use super::*;

    /// Axum Middleware : remplit la session si le cookie est valide.
    ///
    /// - Les assets statiques sont servis sans vérification.
    /// - Si le cookie correspond à une session valide, la session est ajoutée
    ///   aux extensions de la requête.
    /// - Une navigation vers une page privée non authentifiée redirige vers le
    ///   login. Les appels d'API passent et décideront eux-mêmes quoi faire.
    pub async fn server_auth_guard(mut req: Request, next: Next) -> Response {
        let path = req.uri().path();
        debug!("GUARD : request to path : {}", path);

        if is_static_asset(path) {
            debug!("GUARD : static asset, forwarding request");
            return next.run(req).await;
        }

        debug!("GUARD : checking for auth");
        if let Some(session_token) = session::parse_token_from_cookie(req.headers()) {
            debug!(
                "GUARD : session_token found in cookie : {:?}",
                &session_token
            );
            if let Some(session_record) = session::get_from_db(&session_token).await {
                debug!("GUARD : session_record found in db, inserting axum extension and forwarding request");
                req.extensions_mut().insert(session_record);
                return next.run(req).await;
            }
        }

        let is_nav = is_page_navigation(req.headers(), req.method());
        let route = Route::from_str(path).unwrap_or(Route::PageNotFound { segments: vec![] });
        if is_nav && !route.is_public() {
            return Redirect::to(&Route::Login.to_string()).into_response();
        }

        debug!("GUARD : public route, or api call, forwarding request");
        next.run(req).await
    }

    /// Vérifie si la requête correspond à un asset public.
    fn is_static_asset(path: &str) -> bool {
        if path.starts_with("/public")
            || path.starts_with("/_dioxus")
            || path.starts_with("/assets")
            || path.starts_with("/wasm")
        {
            return true;
        }

        // Tout chemin terminé par une extension de fichier est un asset.
        path.rsplit('/')
            .next()
            .is_some_and(|filename| filename.contains('.'))
    }

    /// Vérifie si la requête correspond à une navigation.
    fn is_page_navigation(headers: &HeaderMap, method: &Method) -> bool {
        if let Some(mode) = headers.get("sec-fetch-mode") {
            if mode == "navigate" {
                return true;
            }
        }

        if let Some(dest) = headers.get("sec-fetch-dest") {
            if dest == "document" {
                return true;
            }
        }

        // Fallback pour les vieux clients : GET qui demande du HTML.
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

    /// Vérifie si l'utilisateur existe et si le mot de passe est le bon.
    pub async fn exists(email: &str, password: &str) -> Result<UserAuthTry, ServerFnError> {
        debug!("AUTH : searching inside db for user with email `{}`", email);

        let Some(user_record) = db::get_user_by_email(email).await? else {
            debug!("AUTH : user not found in db");
            return Ok(UserAuthTry::NonexistentAccount);
        };

        debug!("AUTH : user found in db");
        credentials::verify(user_record.id, password).await
    }

    /// Appelle Helloasso pour vérifier si l'utilisateur est un adhérent.
    /// Si oui, crée le `UserRecord` et son mot de passe.
    pub async fn try_create(
        email: &str,
        password: &str,
    ) -> Result<Result<Uuid, String>, ServerFnError> {
        debug!("AUTH : asking Helloasso if `{}` is an adherent", email);
        let helloasso_payer = helloasso::get_adherent(email).await?;

        match helloasso_payer {
            None => {
                debug!("AUTH : Helloasso has no adherent with this email, connexion refused");
                Ok(Err(
                    "Cet email n'est pas enregistré chez Helloasso".to_string()
                ))
            }
            Some(payer) => {
                debug!("AUTH : Helloasso has user with email `{}`", email);
                if let Err(e) = credentials::verify_password_validity(password) {
                    return Ok(Err(e));
                }

                let user_id = create_user_record_in_db(payer).await?;
                credentials::create(user_id, password)
                    .await?
                    .map_err(ServerFnError::new)?;

                Ok(Ok(user_id))
            }
        }
    }

    /// Crée le `UserRecord` grâce aux données de Helloasso.
    async fn create_user_record_in_db(payer: helloasso::PayerInfo) -> Result<Uuid, ServerFnError> {
        debug!("AUTH : creating user with : {:?}", &payer);

        let email = payer
            .email
            .ok_or_else(|| ServerFnError::new("Empty E-Mail field in helloasso answer"))?;
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

        let user_id = Uuid::now_v7();
        user::ActiveModel {
            id: Set(user_id),
            email: Set(email),
            username: Set(username),
            member_since: Set(utils::current_time_secs() as i64),
            elo: Set(elo::DEFAULT_ELO as i64),
            best_elo: Set(elo::DEFAULT_ELO as i64),
            games_played: Set(0),
            games_won: Set(0),
        }
        .insert(db::get().await)
        .await
        .map_err(db::db_err)?;

        debug!(
            "AUTH : successfully created user inside db with id {:?}",
            user_id
        );

        Ok(user_id)
    }

    /// Change le nom d'utilisateur.
    pub async fn change_username(
        user_id: Uuid,
        new_username: &str,
        password: &str,
    ) -> Result<Result<(), String>, ServerFnError> {
        let UserAuthTry::Success(_) = credentials::verify(user_id, password).await? else {
            return Ok(Err("Mot de passe incorrect".to_string()));
        };

        let username_trimmed = new_username.trim();

        if let Err(e) = check_username_validity(username_trimmed, user_id).await? {
            return Ok(Err(e));
        }

        user::ActiveModel {
            id: Set(user_id),
            username: Set(username_trimmed.to_owned()),
            ..Default::default()
        }
        .update(db::get().await)
        .await
        .map_err(|error| {
            if error.to_string().contains("users_username_unique") {
                ServerFnError::new("Ce nom d'utilisateur est déjà utilisé")
            } else {
                db::db_err(error)
            }
        })?;

        debug!(
            "AUTH : user `{:?}` changed username to `{}`",
            user_id, username_trimmed
        );

        Ok(Ok(()))
    }

    /// Vérifie si le nom d'utilisateur est conforme.
    async fn check_username_validity(
        username: &str,
        user_id: Uuid,
    ) -> Result<Result<(), String>, ServerFnError> {
        let length = username.chars().count();

        if length < 3 {
            return Ok(Err("Nom d'utilisateur trop court".to_string()));
        }

        if length > 32 {
            return Ok(Err("Nom d'utilisateur trop long".to_string()));
        }

        let existing_user = user::Entity::find()
            .select_only()
            .column(user::Column::Id)
            .filter(user::Column::Username.eq(username))
            .filter(user::Column::Id.ne(user_id))
            .into_tuple::<Uuid>()
            .one(db::get().await)
            .await
            .map_err(db::db_err)?;

        if existing_user.is_some() {
            return Ok(Err("Ce nom d'utilisateur est déjà utilisé".to_string()));
        }

        Ok(Ok(()))
    }
}

pub mod credentials {
    use super::*;

    /// Crée un hash à partir d'un mot de passe.
    ///
    /// Argon2 est volontairement exécuté sur un thread bloquant pour ne pas
    /// ralentir le runtime asynchrone.
    pub async fn create_hash(password: &str) -> Result<String, ServerFnError> {
        let password = password.to_owned();

        tokio::task::spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            Argon2::default()
                .hash_password(password.as_bytes(), &salt)
                .map(|hash| hash.to_string())
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .map_err(ServerFnError::new)
    }

    /// Vérifie si le mot de passe est bien celui de l'utilisateur.
    pub async fn verify(user_id: Uuid, password: &str) -> Result<UserAuthTry, ServerFnError> {
        let user_cred_record = user_cred::Entity::find()
            .filter(user_cred::Column::UserId.eq(user_id))
            .one(db::get().await)
            .await
            .map_err(db::db_err)?;

        let Some(user_cred_record) = user_cred_record else {
            return Ok(UserAuthTry::NonexistentCredentials(user_id));
        };

        let password_hash = user_cred_record.password_hash;
        let password = password.to_owned();

        let is_valid = tokio::task::spawn_blocking(move || {
            let parsed_hash = PasswordHash::new(&password_hash).map_err(|e| e.to_string())?;
            Ok::<bool, String>(
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed_hash)
                    .is_ok(),
            )
        })
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .map_err(|e| ServerFnError::new(format!("Hash de mot de passe invalide : {e}")))?;

        if is_valid {
            Ok(UserAuthTry::Success(user_id))
        } else {
            Ok(UserAuthTry::WrongPassword)
        }
    }

    /// Vérifie la conformité d'un mot de passe.
    pub fn verify_password_validity(password: &str) -> Result<(), String> {
        if password.chars().count() < 8 {
            return Err("Mot de passe trop court".to_string());
        }
        Ok(())
    }

    /// Crée un mot de passe et le range dans la base de données.
    pub async fn create(
        user_id: Uuid,
        password: &str,
    ) -> Result<Result<(), String>, ServerFnError> {
        if let Err(e) = verify_password_validity(password) {
            return Ok(Err(e));
        }

        let password_hash = create_hash(password).await?;

        user_cred::ActiveModel {
            id: Set(Uuid::now_v7()),
            user_id: Set(user_id),
            password_hash: Set(password_hash),
        }
        .insert(db::get().await)
        .await
        .map_err(db::db_err)?;

        Ok(Ok(()))
    }

    /// Change le mot de passe.
    pub async fn change_password(
        user_id: Uuid,
        old_password: &str,
        new_password: &str,
    ) -> Result<Result<(), String>, ServerFnError> {
        let UserAuthTry::Success(_) = verify(user_id, old_password).await? else {
            return Ok(Err("Mot de passe incorrect".to_string()));
        };

        if let Err(e) = verify_password_validity(new_password) {
            return Ok(Err(e));
        }

        let new_password_hash = create_hash(new_password).await?;

        let result = user_cred::Entity::update_many()
            .col_expr(
                user_cred::Column::PasswordHash,
                sea_orm::sea_query::Expr::value(new_password_hash),
            )
            .filter(user_cred::Column::UserId.eq(user_id))
            .exec(db::get().await)
            .await
            .map_err(db::db_err)?;

        if result.rows_affected == 0 {
            return Err(ServerFnError::new(
                "Cet utilisateur n'a aucun mot de passe enregistré. Absolument anormal",
            ));
        }

        Ok(Ok(()))
    }
}
