//! Interface client/serveur du système de modération.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::api::types::{SanctionKind, UserRole, UserStatus};

/// Résumé d'un utilisateur dans la liste de modération.
#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ModerationUserSummary {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub elo: i64,
    pub best_elo: i64,
    pub games_played: i64,
    pub games_won: i64,
    pub games_lost: i64,
    pub win_ratio: f32,
    pub member_since: u64,
    pub suspended_until: Option<u64>,
    pub status_reason: Option<String>,
    pub is_env_admin: bool,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ModerationUserPage {
    pub users: Vec<ModerationUserSummary>,
    pub total: u64,
    pub page: u64,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ActivityEntry {
    pub kind: String,
    pub label: String,
    pub detail: String,
    pub occurred_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct AuditEntry {
    pub id: String,
    pub actor_username: String,
    pub action: String,
    pub details: String,
    pub target_username: Option<String>,
    pub target_game_id: Option<String>,
    pub created_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ModerationGamePlayer {
    pub user_id: String,
    pub username: String,
    pub elo_change: i64,
    pub won: bool,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ModerationGameSummary {
    pub id: String,
    pub won_score: i64,
    pub lost_score: i64,
    pub played_at: u64,
    pub recorded_by: String,
    pub players: Vec<ModerationGamePlayer>,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ModerationGamePage {
    pub games: Vec<ModerationGameSummary>,
    pub total: u64,
    pub page: u64,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct ModerationUserDetail {
    pub user: ModerationUserSummary,
    pub moderated_by: Option<String>,
    pub activity: Vec<ActivityEntry>,
    pub games: Vec<ModerationGameSummary>,
    /// `false` si aucun hash n'est présent (jamais défini ou réinitialisé) :
    /// le prochain mot de passe saisi à la connexion sera enregistré.
    pub has_password: bool,
    pub is_self: bool,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct UserStatsUpdate {
    pub elo: i64,
    pub best_elo: i64,
    pub games_played: i64,
    pub games_won: i64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SanctionInput {
    pub kind: SanctionKind,
    pub reason: String,
    /// `None` = suspension indéterminée.
    pub duration_hours: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct GameEditInput {
    pub game_id: String,
    pub won_score: u64,
    pub lost_score: u64,
    pub winning_team: Vec<String>,
    pub losing_team: Vec<String>,
}

/// Nettoie un nom d'utilisateur : espaces multiples, caractères de contrôle
/// et symboles exotiques sont retirés, tout en conservant les accents.
pub fn sanitize_username(raw: &str) -> String {
    let mut output = String::new();
    let mut last_was_space = false;

    for character in raw.trim().chars() {
        if character.is_control() {
            continue;
        }

        if character.is_whitespace() {
            if !last_was_space && !output.is_empty() {
                output.push(' ');
                last_was_space = true;
            }
            continue;
        }

        let allowed =
            character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | '\'' | '’' | '‘');
        if allowed {
            output.push(character);
            last_was_space = false;
        } else if !last_was_space && !output.is_empty() {
            output.push(' ');
            last_was_space = true;
        }
    }

    let output = output.trim().to_string();
    output.chars().take(32).collect()
}

/// Liste paginée des utilisateurs.
#[server]
pub async fn moderation_list_users(
    search: String,
    status: Option<UserStatus>,
    page: u64,
) -> Result<ModerationUserPage, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::list_users(&actor, search, status, page).await
}

/// Détail d'un utilisateur et de son activité.
#[server]
pub async fn moderation_get_user(user_id: String) -> Result<ModerationUserDetail, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::get_user_detail(&actor, user_id).await
}

/// Modifie l'Elo et les compteurs d'un utilisateur.
#[server]
pub async fn moderation_update_stats(
    user_id: String,
    update: UserStatsUpdate,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::update_stats(&actor, user_id, update).await
}

/// Force le changement du nom d'utilisateur.
#[server]
pub async fn moderation_change_username(
    user_id: String,
    new_username: String,
) -> Result<Result<String, String>, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::change_username(&actor, user_id, new_username).await
}

/// Réinitialise le mot de passe d'un utilisateur.
#[server]
pub async fn moderation_reset_password(
    user_id: String,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::reset_password(&actor, user_id).await
}

/// Promeut ou rétrograde un utilisateur.
#[server]
pub async fn moderation_set_role(
    user_id: String,
    role: UserRole,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::set_role(&actor, user_id, role).await
}

/// Applique une suspension ou un bannissement.
#[server]
pub async fn moderation_apply_sanction(
    user_id: String,
    sanction: SanctionInput,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::apply_sanction(&actor, user_id, sanction).await
}

/// Lève une sanction.
#[server]
pub async fn moderation_lift_sanction(
    user_id: String,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::lift_sanction(&actor, user_id).await
}

/// Liste paginée des parties.
#[server]
pub async fn moderation_list_games(
    search: String,
    page: u64,
) -> Result<ModerationGamePage, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::list_games(&actor, search, page).await
}

/// Détail d'une partie.
#[server]
pub async fn moderation_get_game(game_id: String) -> Result<ModerationGameSummary, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::get_game(&actor, game_id).await
}

/// Modifie le score et les participants d'une partie.
#[server]
pub async fn moderation_update_game(
    input: GameEditInput,
) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::update_game(&actor, input).await
}

/// Supprime une partie et annule ses effets.
#[server]
pub async fn moderation_delete_game(game_id: String) -> Result<Result<(), String>, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::delete_game(&actor, game_id).await
}

/// Dernières actions de modération.
#[server]
pub async fn moderation_recent_audit(page: u64) -> Result<Vec<AuditEntry>, ServerFnError> {
    use crate::server::{access, moderation};
    let actor = access::require_admin().await?;
    moderation::recent_audit(&actor, page).await
}
