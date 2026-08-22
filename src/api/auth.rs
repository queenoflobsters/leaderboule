use dioxus::prelude::*;

/// Login Server Function
#[server]
pub async fn login(email: String, password: String) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::auth;
    use crate::server::auth::UserAuthTry;
    if !email.contains('@') || !email.contains('.') {
        return Ok(Err("Email invalide".to_string()));
    }

    debug!("AUTH : user with email `{}` is trying to connect", email);

    // Search inside the DB for user with this email
    match auth::account::exists(&email, &password).await? {
        // SCENARIO 1 : The user exists and it's the good password
        UserAuthTry::Success(user_id) => {
            // Create a session_record and fill the cookie
            let token = auth::session::create(user_id).await?;
            auth::cookie::create_in_response(token)?;
            Ok(Ok(()))
        }
        // SCENARIO 2 : The user exists but wrong password
        UserAuthTry::WrongPassword => Ok(Err("Mot de passe incorrect.".to_string())),
        // SCENARIO 3 : The user doesn't exist in the DB
        UserAuthTry::NonexistentAccount => {
            // Check Helloasso to see the email is registered
            match auth::account::try_create(&email, &password).await? {
                // SCENARIO 3.1 The user exists in Helloasso
                Ok(user_id) => {
                    // Create a session_record and fill the cookie
                    let token = auth::session::create(user_id).await?;
                    auth::cookie::create_in_response(token)?;
                    Ok(Ok(()))
                }
                // SCENARIO 3.2 : The user is not registered in Helloasso
                Err(e) => Ok(Err(e)),
            }
        }
        // SCENARIO 4 : User exists in DB but has no credentials in DB
        UserAuthTry::NonexistentCredentials(user_id) => {
            match auth::credentials::create(&user_id, &password).await? {
                Ok(()) => Ok(Err("Mot de passe réinitialisé avec succès".to_string())),
                Err(e) => Ok(Err(e)),
            }
        }
    }
}

/// Logout Server Function
#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    use crate::server::auth;
    auth::session::delete().await?;
    auth::cookie::clear_from_response()
}

#[server]
pub async fn change_username(
    new_username: String,
    password: String,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::auth;

    let Some(session_record) = auth::session::get_from_extension() else {
        return Err(ServerFnError::new("User is not authenticated"));
    };

    auth::account::change_username(&session_record.user_id, &new_username, &password).await
}

#[server]
pub async fn change_password(
    old_password: String,
    new_password: String,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::auth;

    let Some(session_record) = auth::session::get_from_extension() else {
        return Err(ServerFnError::new("User is not authenticated"));
    };

    auth::credentials::change_password(&session_record.user_id, &old_password, &new_password).await
}
