use dioxus::prelude::*;

/// Demande de login
/// (j'ai précédemment fait la doc interne en anglais et j'ai la flemme de la traduire)
#[server]
pub async fn login(email: String, password: String) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::auth::UserAuthTry;
    use crate::server::{access, audit, auth};
    if !email.contains('@') || !email.contains('.') {
        return Ok(Err("Email invalide".to_string()));
    }

    debug!("AUTH : user with email `{}` is trying to connect", email);

    // Search inside the DB for user with this email
    match auth::account::exists(&email, &password).await? {
        // SCENARIO 1 : The user exists and it's the good password
        UserAuthTry::Success(user_id) => {
            if let Err(e) = access::prepare_login(user_id).await? {
                return Ok(Err(e));
            }
            // Create a session_record and fill the cookie
            let token = auth::session::create(user_id).await?;
            auth::cookie::create_in_response(token)?;
            audit::log_activity_best_effort(user_id, "login", "Connexion réussie".to_string())
                .await;
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
                    if let Err(e) = access::prepare_login(user_id).await? {
                        return Ok(Err(e));
                    }
                    // Create a session_record and fill the cookie
                    let token = auth::session::create(user_id).await?;
                    auth::cookie::create_in_response(token)?;
                    audit::log_activity_best_effort(
                        user_id,
                        "login",
                        "Création du compte et connexion".to_string(),
                    )
                    .await;
                    Ok(Ok(()))
                }
                // SCENARIO 3.2 : The user is not registered in Helloasso
                Err(e) => Ok(Err(e)),
            }
        }
        // SCENARIO 4 : User exists in DB but has no credentials. Cela couvre
        // aussi le cas d'un mot de passe réinitialisé par un administrateur :
        // le mot de passe saisi devient immédiatement le nouveau.
        UserAuthTry::NonexistentCredentials(user_id) => {
            if let Err(e) = access::prepare_login(user_id).await? {
                return Ok(Err(e));
            }

            match auth::credentials::create(user_id, &password).await? {
                Ok(()) => {
                    let token = auth::session::create(user_id).await?;
                    auth::cookie::create_in_response(token)?;
                    audit::log_activity_best_effort(
                        user_id,
                        "password_changed",
                        "Nouveau mot de passe défini à la connexion après réinitialisation"
                            .to_string(),
                    )
                    .await;
                    audit::log_activity_best_effort(
                        user_id,
                        "login",
                        "Connexion réussie".to_string(),
                    )
                    .await;
                    Ok(Ok(()))
                }
                Err(e) => Ok(Err(e)),
            }
        }
    }
}

/// Demande de logout
#[server]
pub async fn logout() -> Result<(), ServerFnError> {
    use crate::server::{audit, auth};
    let user_id = auth::session::get_from_extension().map(|session| session.user_id);
    auth::session::delete().await?;
    if let Some(user_id) = user_id {
        audit::log_activity_best_effort(user_id, "logout", "Déconnexion".to_string()).await;
    }
    auth::cookie::clear_from_response()
}

/// Demande pour changer le nom d'utilisateur
#[server]
pub async fn change_username(
    new_username: String,
    password: String,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::auth;

    let Some(session_record) = auth::session::get_from_extension() else {
        return Err(ServerFnError::new("User is not authenticated"));
    };

    auth::account::change_username(session_record.user_id, &new_username, &password).await
}

/// Demande pour changer de mot de passe
#[server]
pub async fn change_password(
    old_password: String,
    new_password: String,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::auth;

    let Some(session_record) = auth::session::get_from_extension() else {
        return Err(ServerFnError::new("User is not authenticated"));
    };

    auth::credentials::change_password(session_record.user_id, &old_password, &new_password).await
}
