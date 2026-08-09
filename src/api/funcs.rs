use crate::api::UserPerformance;

use dioxus::{prelude::*};

#[server]
pub async fn get_users_performances() -> Result<Vec<UserPerformance>> {
    let josephine = UserPerformance {
        name: "Josephine".into(),
        elo: 350,
        games_played: 23,
        games_won: 13,
    };
    let edward = UserPerformance {
        name: "Edward le sacro saint destructeur de chattes".into(),
        elo: 1000,
        games_played: 100,
        games_won: 100,
    };
    let pablo = UserPerformance {
        name: "Pablo".into(),
        elo: 8,
        games_played: 1331,
        games_won: 7,
    };
    let stella = UserPerformance {
        name: "Stella".into(),
        elo: 110,
        games_played: 1,
        games_won: 0,
    };
    Ok(vec![josephine, edward, pablo, stella])
}

/// Login Server Function
#[server]
pub async fn login(email: String) -> Result<Result<(), String>, ServerFnError> {
    use dioxus::server::axum::http::{header::SET_COOKIE, HeaderValue};
    use uuid::Uuid;

    // 1. Check HelloAsso
    let is_adherent = crate::server::check_helloasso_adherent(&email)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !is_adherent {
        return Ok(Err(
            "Votre email n'est pas enregistré chez HelloAsso.".to_string()
        ));
    }

    // 2. Generate session and insert into SurrealDB without SQL (.create())
    let token = Uuid::new_v4().to_string();
    let db = crate::server::get_db().await;
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339();

    let session = crate::server::SessionRecord {
        token: token.clone(),
        email: email.clone(),
        expires_at,
    };

    let _created: Option<crate::server::SessionRecord> = db
        .create(("session", token.clone()))
        .content(session)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // 3. Set-Cookie header directly without tower_cookies
    let cookie_str =
        format!("session_token={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000");

    // if let Ok(mut res_parts) = FullstackContext::extract::<axum::http::response::Parts, _>().await {
    //     if let Ok(header_val) = axum::http::HeaderValue::from_str(&cookie_str) {
    //         res_parts.headers.append(axum::http::header::SET_COOKIE, header_val);
    //     }
    // }

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
    use dioxus::server::axum::http::{header::SET_COOKIE, HeaderValue};
    use dioxus::server::axum;
    use crate::server::parse_cookie;

    if let Ok(req_headers) = FullstackContext::extract::<axum::http::HeaderMap, _>().await {
        if let Some(token) = parse_cookie(&req_headers, "session_token") {
            let db = crate::server::get_db().await;

            // Delete record from SurrealDB without SQL (.delete())
            let _: Option<crate::server::SessionRecord> = db
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
    use crate::server::AuthUser;
    use dioxus::server::axum::Extension;

    let auth_user = FullstackContext::extract::<Extension<AuthUser>, _>().await;

    match auth_user {
        Ok(Extension(user)) => Ok(Some(user.email)),
        Err(_) => Ok(None),
    }
}
