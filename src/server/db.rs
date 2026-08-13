use dioxus::server::ServerFnError;
use serde::{Deserialize, Serialize};
use surrealdb::{
    engine::remote::ws::{Client, Ws},
    opt::auth::Root,
    types::{SurrealValue, Uuid},
    Surreal,
};
use tokio::sync::OnceCell;

use crate::server::{
    elo,
    helloasso::{self, PayerInfo},
    utils,
};

pub static DB: OnceCell<Surreal<Client>> = OnceCell::const_new();

#[derive(Debug, Serialize, Deserialize, Clone, Copy, SurrealValue)]
pub struct SessionToken(pub Uuid);

#[derive(Debug, Serialize, Deserialize, Clone, Copy, SurrealValue)]
pub struct UserId(pub Uuid);

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
        db
    })
    .await
}

/// Generate session and insert into SurrealDB
pub async fn save_session_record(user_id: UserId) -> Result<SessionToken, ServerFnError> {
    // TODO MAKE THIS WITH UUID NOT EMAIL
    let session_token = SessionToken(Uuid::new_v7());
    let db = get().await;
    let expires_at = utils::current_time_secs() + utils::THIRTY_DAYS_IN_SECS;

    let session = SessionRecord {
        session_token,
        user_id,
        expires_at,
    };

    let _created: Option<SessionRecord> = db
        .create(("session", session_token.0.to_string()))
        .content(session)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(session_token)
}

/// Returns the user id
pub async fn has_account_or_create(email: &str) -> Result<Option<UserId>, ServerFnError> {
    // 1. Search inside the DB for user with this email
    let db = get().await;
    let user_match: Option<UserRecord> = db
        .query("SELECT * FROM users WHERE email = $email")
        .bind(("email", email))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .take(0)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if let Some(user) = user_match {
        return Ok(Some(user.id));
    }

    // 2. Search if they are registered in Helloasso
    let helloasso_payer = helloasso::get_adherent(email).await?;

    match helloasso_payer {
        // There is no adherent with this email
        None => Ok(None),
        // 3. There is an adherent with this email
        // Create an account and return the user id
        Some(payer) => Ok(Some(create_user_from_payer(payer).await?)),
    }
}

/// Returns the user id
async fn create_user_from_payer(payer: PayerInfo) -> Result<UserId, ServerFnError> {
    let db = get().await;

    let id = UserId(Uuid::new_v7());
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
        id,
        email,
        username,
        elo: elo::DEFAULT_ELO,
        games_played: 0,
        games_won: 0,
    };

    let _created: Option<UserRecord> = db
        .create(("users", id.0))
        .content(user_record)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(id)
}
