//! Écriture des journaux d'activité et d'audit.

use dioxus::{logger::tracing, server::ServerFnError};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set};
use uuid::Uuid;

use crate::server::{
    db,
    entities::{activity_log, audit_log, user},
    utils,
};

/// Enregistre une action de modération dans le journal d'audit.
///
/// La fonction accepte n'importe quelle connexion SeaORM : elle peut donc être
/// appelée dans la même transaction que l'action modérée, ce qui garantit
/// qu'aucune modification n'est appliquée sans trace.
#[allow(clippy::too_many_arguments)]
pub async fn record_audit<C: ConnectionTrait>(
    conn: &C,
    actor: &user::Model,
    action: &str,
    target_user_id: Option<Uuid>,
    target_username: Option<String>,
    target_game_id: Option<Uuid>,
    details: String,
) -> Result<(), ServerFnError> {
    audit_log::ActiveModel {
        id: Set(Uuid::now_v7()),
        actor_id: Set(Some(actor.id)),
        actor_username: Set(actor.username.clone()),
        target_user_id: Set(target_user_id),
        target_username: Set(target_username),
        target_game_id: Set(target_game_id),
        action: Set(action.to_string()),
        details: Set(details),
        created_at: Set(utils::current_time_secs() as i64),
    }
    .insert(conn)
    .await
    .map_err(db::db_err)?;

    Ok(())
}

/// Enregistre un événement d'activité pour un utilisateur.
pub async fn log_activity<C: ConnectionTrait>(
    conn: &C,
    user_id: Uuid,
    kind: &str,
    description: String,
) -> Result<(), ServerFnError> {
    activity_log::ActiveModel {
        id: Set(Uuid::now_v7()),
        user_id: Set(user_id),
        kind: Set(kind.to_string()),
        description: Set(description),
        created_at: Set(utils::current_time_secs() as i64),
    }
    .insert(conn)
    .await
    .map_err(db::db_err)?;

    Ok(())
}

/// Variante « best effort » : une panne du journal ne doit pas bloquer une
/// connexion ou l'enregistrement d'une partie.
pub async fn log_activity_best_effort(user_id: Uuid, kind: &str, description: String) {
    if let Err(error) = log_activity(db::get().await, user_id, kind, description).await {
        tracing::warn!("Impossible d'écrire dans le journal d'activité : {error}");
    }
}
