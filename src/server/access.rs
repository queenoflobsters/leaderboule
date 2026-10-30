//! Contrôle d'accès, rôles et état de modération.
//!
//! Le rôle effectif d'un utilisateur est toujours recalculé depuis la base à
//! chaque requête : une rétrogradation prend donc effet immédiatement, même
//! pour une session déjà ouverte. Les emails listés dans `ADMIN_EMAILS` sont
//! en plus forcés au rôle `admin` et ne peuvent jamais être rétrogradés.

use dioxus::server::ServerFnError;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};
use uuid::Uuid;

use crate::{
    api::{
        types::{UserRole, UserStatus},
        utils::format_date_and_hour,
    },
    server::{auth::session, db, entities::user, utils},
};

/// Erreur HTTP 403 utilisée par toutes les server functions de modération.
pub fn forbidden(message: impl ToString) -> ServerFnError {
    ServerFnError::ServerError {
        message: message.to_string(),
        code: 403,
        details: None,
    }
}

/// Emails administrateurs définis dans `.env`, séparés par des virgules.
pub fn env_admin_emails() -> Vec<String> {
    std::env::var("ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(|email| email.trim().to_lowercase())
        .filter(|email| !email.is_empty())
        .collect()
}

/// Vrai si l'email fait partie de la liste d'administrateurs du `.env`.
pub fn is_env_admin(email: &str) -> bool {
    let email = email.trim().to_lowercase();
    env_admin_emails().iter().any(|admin| admin == &email)
}

/// Vrai si l'utilisateur possède le rôle admin en base ou via `.env`.
pub fn is_admin(user: &user::Model) -> bool {
    user.role.is_admin() || is_env_admin(&user.email)
}

/// Vrai si le compte peut se connecter et utiliser l'application.
pub fn is_active(user: &user::Model) -> bool {
    user.status.is_active()
}

/// Message affiché à la connexion lorsqu'un compte est sanctionné.
pub fn sanction_message(user: &user::Model) -> String {
    match user.status {
        UserStatus::Banned => format!(
            "Ce compte est banni.{}",
            user.status_reason
                .as_deref()
                .map(|reason| format!(" Motif : {reason}"))
                .unwrap_or_default()
        ),
        UserStatus::Suspended => {
            let duration = match user.suspended_until {
                Some(until) => format!(" jusqu'au {}", format_date_and_hour(until.max(0) as u64)),
                None => " indéfiniment".to_string(),
            };
            format!(
                "Ce compte est suspendu{duration}.{}",
                user.status_reason
                    .as_deref()
                    .map(|reason| format!(" Motif : {reason}"))
                    .unwrap_or_default()
            )
        }
        UserStatus::Active => "Ce compte n'est pas autorisé à se connecter.".to_string(),
    }
}

/// Force le rôle admin pour les emails de `ADMIN_EMAILS` déjà présents en base.
///
/// Appelé une fois après la création du schéma, puis à chaque connexion : les
/// administrateurs du `.env` le restent même si leur rôle a été modifié.
/// Ils sont également réactivés s'ils avaient été suspendus avant d'être
/// ajoutés à la liste.
pub async fn sync_env_admins(conn: &impl ConnectionTrait) -> Result<(), ServerFnError> {
    let admins = env_admin_emails();
    if admins.is_empty() {
        return Ok(());
    }

    let users = user::Entity::find().all(conn).await.map_err(db::db_err)?;

    for user in users {
        if is_env_admin(&user.email)
            && (user.role != UserRole::Admin
                || user.status != UserStatus::Active
                || user.suspended_until.is_some()
                || user.status_reason.is_some())
        {
            user::ActiveModel {
                id: Set(user.id),
                role: Set(UserRole::Admin),
                status: Set(UserStatus::Active),
                suspended_until: Set(None),
                status_reason: Set(None),
                ..Default::default()
            }
            .update(conn)
            .await
            .map_err(db::db_err)?;
        }
    }

    Ok(())
}

/// Ré-applique le rôle admin et l'état actif si l'email vient de `ADMIN_EMAILS`.
///
/// Un administrateur du `.env` ne doit jamais rester rétrogradé, suspendu ou
/// banni, même si son rôle ou son statut a été modifié directement en base.
pub async fn ensure_env_admin_role(user: user::Model) -> Result<user::Model, ServerFnError> {
    let needs_update = is_env_admin(&user.email)
        && (user.role != UserRole::Admin
            || user.status != UserStatus::Active
            || user.suspended_until.is_some()
            || user.status_reason.is_some());

    if needs_update {
        user::ActiveModel {
            id: Set(user.id),
            role: Set(UserRole::Admin),
            status: Set(UserStatus::Active),
            suspended_until: Set(None),
            status_reason: Set(None),
            ..Default::default()
        }
        .update(db::get().await)
        .await
        .map_err(db::db_err)
    } else {
        Ok(user)
    }
}

/// Réactive automatiquement une suspension arrivée à échéance.
///
/// Retourne `None` si le compte est banni ou encore suspendu.
pub async fn refresh_sanction_state(
    user: user::Model,
) -> Result<Option<user::Model>, ServerFnError> {
    match user.status {
        UserStatus::Active => Ok(Some(user)),
        UserStatus::Suspended => {
            let now = utils::current_time_secs() as i64;
            let expired = user.suspended_until.is_some_and(|until| until <= now);
            if expired {
                let updated = user::ActiveModel {
                    id: Set(user.id),
                    status: Set(UserStatus::Active),
                    suspended_until: Set(None),
                    status_reason: Set(None),
                    ..Default::default()
                }
                .update(db::get().await)
                .await
                .map_err(db::db_err)?;
                Ok(Some(updated))
            } else {
                Ok(None)
            }
        }
        UserStatus::Banned => Ok(None),
    }
}

/// Vérifie qu'un compte peut se connecter et synchronise son rôle admin.
pub async fn prepare_login(user_id: Uuid) -> Result<Result<(), String>, ServerFnError> {
    let Some(user) = db::get_user(user_id).await? else {
        return Ok(Err("Compte introuvable".to_string()));
    };

    // Les administrateurs du `.env` sont réactivés avant le contrôle de
    // sanction : ils ne doivent jamais rester bloqués.
    let user = ensure_env_admin_role(user).await?;
    let original = user.clone();
    let Some(_user) = refresh_sanction_state(user).await? else {
        return Ok(Err(sanction_message(&original)));
    };

    Ok(Ok(()))
}

/// Récupère l'utilisateur courant et exige le rôle admin.
///
/// Toute server function de modération commence par cet appel. En cas
/// d'absence de session, de compte sanctionné ou de rôle insuffisant, une
/// erreur 403 est renvoyée au client.
pub async fn require_admin() -> Result<user::Model, ServerFnError> {
    let Some(session_record) = session::get_from_extension() else {
        return Err(forbidden("Authentification requise"));
    };

    let Some(user) = db::get_user(session_record.user_id).await? else {
        return Err(forbidden("Compte introuvable"));
    };

    let user = ensure_env_admin_role(user).await?;

    let Some(user) = refresh_sanction_state(user).await? else {
        return Err(forbidden("Compte suspendu ou banni"));
    };

    if !is_admin(&user) {
        return Err(forbidden("Accès réservé aux administrateurs"));
    }

    Ok(user)
}

/// Version booléenne utilisée par le middleware HTTP.
pub async fn is_admin_user_id(user_id: Uuid) -> Result<bool, ServerFnError> {
    let Some(user) = db::get_user(user_id).await? else {
        return Ok(false);
    };

    let user = ensure_env_admin_role(user).await?;

    if !is_active(&user) {
        return Ok(false);
    }

    Ok(is_admin(&user))
}
