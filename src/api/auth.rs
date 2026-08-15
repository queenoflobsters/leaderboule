use dioxus::prelude::*;
#[cfg(feature = "server")]
use {crate::server::auth::SessionToken, dioxus::server::axum::extract::Extension};

/// Login Server Function
#[server]
pub async fn login(email: String) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::auth;
    if !email.contains('@') || !email.contains('.') {
        return Ok(Err("Email invalide".to_string()));
    }

    info!("AUTH : user with email `{}` is trying to connect", email);

    if let Some(user_id) = auth::has_account_or_create(&email).await? {
        let token = auth::create_session_record(user_id).await?;
        auth::add_cookie_to_response(token)?;
        Ok(Ok(()))
    } else {
        Ok(Err("Email non enregistré chez HelloAsso.".to_string()))
    }
}

/// Logout Server Function
#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    let ctx =
        FullstackContext::current().ok_or(ServerFnError::new("Failed to get FullstackContext"))?;
    let session_token = ctx
        .extension::<SessionToken>()
        .ok_or(ServerFnError::new("Failed to get extension"))?;
    use crate::server::auth;
    auth::delete_session_record().await?;
    auth::clear_cookie_from_response()
}
