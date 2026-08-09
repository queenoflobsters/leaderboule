use dioxus::server::axum;
use dioxus::server::axum::{extract::Request, middleware::Next, response::Response};
use serde::{Deserialize, Serialize};
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use surrealdb::types::SurrealValue;
use surrealdb::Surreal;
use tokio::sync::OnceCell;

pub static DB: OnceCell<Surreal<Client>> = OnceCell::const_new();

/// Session database model
#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct SessionRecord {
    pub token: String,
    pub email: String,
    pub expires_at: String,
}

/// User identity inserted into Axum Request Extensions when authenticated
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub email: String,
}

/// Initialize SurrealDB connection singleton
pub async fn get_db() -> &'static Surreal<Client> {
    DB.get_or_init(|| async {
        let db_user = std::env::var("DATABASE_USER").unwrap_or_else(|_| "root".to_string());
        let db_pass = std::env::var("DATABASE_PASS").unwrap_or_else(|_| "root".to_string());
        let db = Surreal::new::<Ws>("127.0.0.1:8000")
            .await
            .expect("Failed to connect to SurrealDB");
        // WARN TODO MODIFY THIS LATER
        db.signin(Root {
            username: db_user,
            password: db_pass,
        })
        .await
        .expect("Failed to sign in to SurrealDB");
        db.use_ns("myapp")
            .use_db("myapp")
            .await
            .expect("Failed to select namespace/db");
        db
    })
    .await
}

/// Helper function to parse cookie string from raw headers without tower_cookies
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

/// HelloAsso member check stub
pub async fn check_helloasso_adherent(email: &str) -> Result<bool, String> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        return Ok(false);
    }
    Ok(true)
}

/// Axum Middleware: Populates AuthUser extension if cookie is valid.
/// Does NOT check paths (handled declaratively by Dioxus #[layout]).
pub async fn auth_middleware(mut req: Request, next: Next) -> Response {
    if let Some(token) = parse_cookie(req.headers(), "session_token") {
        let db = get_db().await;

        // Query SurrealDB using typed SDK method .select() without raw SQL
        let session: Option<SessionRecord> = db.select(("session", token)).await.ok().flatten();

        if let Some(session) = session {
            if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&session.expires_at) {
                if expires > chrono::Utc::now() {
                    req.extensions_mut().insert(AuthUser {
                        email: session.email,
                    });
                }
            }
        }
    }

    next.run(req).await
}
