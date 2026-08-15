use dioxus::prelude::*;

/// Login Server Function
#[server]
pub async fn login(email: String) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::auth;
    if !email.contains('@') || !email.contains('.') {
        return Ok(Err("Email invalide".to_string()));
    }

    debug!("AUTH : user with email `{}` is trying to connect", email);

    if let Some(user_id) = auth::has_account_or_create(&email).await? {
        let token = auth::create_session_record(user_id).await?;
        auth::create_cookie_in_response(token)?;
        Ok(Ok(()))
    } else {
        Ok(Err("Email non enregistré chez HelloAsso.".to_string()))
    }
}

/// Logout Server Function
#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    use crate::server::auth;
    auth::delete_session_record().await?;
    auth::clear_cookie_from_response()
}
