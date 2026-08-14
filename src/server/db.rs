use std::str::FromStr;

use dioxus::{fullstack::FullstackContext, logger::tracing::info, server::ServerFnError};
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
    types::{RecordId, RecordIdKey, SurrealValue, Uuid},
    Surreal,
};
use tokio::sync::OnceCell;

use crate::server::{
    auth, elo,
    helloasso::{self, PayerInfo},
    utils,
};

pub static DB: OnceCell<Surreal<Client>> = OnceCell::const_new();

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

/// User database model
#[derive(Serialize, Deserialize, Clone, SurrealValue)]
pub struct UserRecord {
    pub id: UserId,
    pub email: String,
    pub username: String,
    pub elo: u64,
    pub games_played: u64,
    pub games_won: u64,
}

/// Initialize SurrealDB connection singleton
pub async fn get() -> &'static Surreal<Client> {
    DB.get_or_init(|| async {
        let db_url = std::env::var("DATABASE_URL").unwrap_or("127.0.0.1:8000".to_string());
        let db_user = std::env::var("DATABASE_USER").unwrap_or("root".to_string());
        let db_pass = std::env::var("DATABASE_PASS").unwrap_or("root".to_string());

        let db = Surreal::new::<Ws>(db_url)
            .await
            .expect("Failed to connect to SurrealDB");

        db.signin(Root {
            username: db_user,
            password: db_pass,
        })
        .await
        .expect("Failed to sign in to SurrealDB");

        db.use_ns("leaderboule")
            .use_db("leaderboule")
            .await
            .expect("Failed to select namespace/db");

        db.query("DEFINE TABLE IF NOT EXISTS users")
            .await
            .expect("Failed to create table \"users\"");
        db
    })
    .await
}

/// Generate session and insert into SurrealDB
pub async fn create_session_record(user_id: UserId) -> Result<SessionToken, ServerFnError> {
    // TODO MAKE THIS WITH UUID NOT EMAIL
    let session_token = SessionToken::new_v7();
    let db = get().await;
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
        if let Some(session_token) = auth::parse_session_token_cookie(&req_headers) {
            let _: Option<SessionRecord> = get()
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
    let db = get().await;
    let user_match: Option<UserRecord> = db
        .query("SELECT * FROM users WHERE email = $email")
        .bind(("email", email))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    // let user_match = Some(UserRecord {
    //     id: UserId(Uuid::max()),
    //     email: "max@maximum.com".to_string(),
    //     username: "Maxime Maximum".to_string(),
    //     elo: 9999,
    //     games_played: 1,
    //     games_won: 1
    // });

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
async fn create_user_from_payer(payer: PayerInfo) -> Result<UserId, ServerFnError> {
    info!("AUTH : creating user with : {:?}", &payer);

    let db = get().await;

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

    let user_record = UserRecord {
        id: id.clone(),
        email,
        username,
        elo: elo::DEFAULT_ELO,
        games_played: 0,
        games_won: 0,
    };

    info!("DAB DAB DAB");
    let _created: Option<UserRecord> = db
        .create(&id.0)
        .content(user_record)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    info!("DAB DAB DAB DAB");

    info!(
        "AUTH : successfully created user inside db with id {:?}",
        id
    );
    Ok(id)
}
