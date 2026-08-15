use dioxus::prelude::*;

/// Login Server Function
#[server]
pub async fn login(email: String) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::auth;
    if !email.contains('@') || !email.contains('.') {
        return Ok(Err("Email invalide".to_string()));
    }

    debug!("AUTH : user with email `{}` is trying to connect", email);

    // 1. Search inside the DB for user with this email
    if let Some(user_id) = auth::account::exists_or_create(&email).await? {
        let token = auth::session::create(user_id).await?;
        auth::cookie::create_in_response(token)?;
        Ok(Ok(()))
    // 2. Search if they are registered in Helloasso
    } else if let Some(user_id) = auth::account::try_create(&email).await? {
        let token = auth::session::create(user_id).await?;
        auth::cookie::create_in_response(token)?;
        Ok(Ok(()))
    // 3. They are not authenticated
    } else {
        Ok(Err("Email non enregistré chez HelloAsso.".to_string()))
    }
}

/// Logout Server Function
#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    use crate::server::auth;
    auth::session::delete().await?;
    auth::cookie::clear_from_response()
}
