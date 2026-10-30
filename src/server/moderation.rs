//! Logique métier de la modération.
//!
//! Toutes les fonctions publiques reçoivent l'administrateur déjà authentifié
//! par `access::require_admin`. Les modifications de partie sont transactionnelles
//! et commencent par annuler les effets de l'ancienne partie avant d'appliquer
//! les nouveaux, afin que l'Elo et les compteurs restent cohérents.

use std::collections::{HashMap, HashSet};

use dioxus::server::ServerFnError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbBackend, EntityTrait, FromQueryResult,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::{
    api::{
        db::global::GameSendItem,
        moderation::{
            ActivityEntry, AuditEntry, GameEditInput, ModerationGamePage, ModerationGamePlayer,
            ModerationGameSummary, ModerationUserDetail, ModerationUserPage, ModerationUserSummary,
            SanctionInput, UserStatsUpdate,
        },
        types::{SanctionKind, UserRole, UserStatus},
    },
    server::{
        access, audit, db, elo,
        entities::{activity_log, audit_log, game, game_player, session, user, user_cred},
        utils,
    },
};

const USER_PAGE_SIZE: u64 = 20;
const GAME_PAGE_SIZE: u64 = 12;
const TIMELINE_LIMIT: usize = 80;

fn invalid_id() -> ServerFnError {
    ServerFnError::ServerError {
        message: "Identifiant invalide".to_string(),
        code: 400,
        details: None,
    }
}

fn not_found(message: &str) -> ServerFnError {
    ServerFnError::ServerError {
        message: message.to_string(),
        code: 404,
        details: None,
    }
}

fn summary(user: &user::Model) -> ModerationUserSummary {
    ModerationUserSummary {
        id: user.id.to_string(),
        username: user.username.clone(),
        email: user.email.clone(),
        role: if access::is_admin(user) {
            UserRole::Admin
        } else {
            user.role
        },
        status: user.status,
        elo: user.elo,
        best_elo: user.best_elo,
        games_played: user.games_played,
        games_won: user.games_won,
        games_lost: user.games_lost() as i64,
        win_ratio: user.win_ratio(),
        member_since: user.member_since.max(0) as u64,
        suspended_until: user.suspended_until.map(|until| until.max(0) as u64),
        status_reason: user.status_reason.clone(),
        is_env_admin: access::is_env_admin(&user.email),
    }
}

fn activity_label(kind: &str) -> &'static str {
    match kind {
        "login" => "Connexion",
        "logout" => "Déconnexion",
        "game_recorded" => "Partie enregistrée",
        "username_changed" => "Nom modifié",
        "password_changed" => "Mot de passe modifié",
        "sanction" => "Sanction",
        _ => "Activité",
    }
}

fn audit_label(action: &str) -> &'static str {
    match action {
        "update_stats" => "Statistiques modifiées",
        "change_username" => "Nom forcé",
        "reset_password" => "Mot de passe réinitialisé",
        "set_role" => "Rôle modifié",
        "suspend" => "Suspension",
        "ban" => "Bannissement",
        "lift_sanction" => "Sanction levée",
        "update_game" => "Partie modifiée",
        "delete_game" => "Partie supprimée",
        _ => "Action de modération",
    }
}

/// Liste paginée des utilisateurs, avec recherche nom/email et filtre d'état.
pub async fn list_users(
    _actor: &user::Model,
    search: String,
    status: Option<UserStatus>,
    page: u64,
) -> Result<ModerationUserPage, ServerFnError> {
    let mut query = user::Entity::find();
    let search = search.trim();

    if !search.is_empty() {
        let pattern = format!("%{}%", db::escape_like(search));
        query = query.filter(
            Condition::any()
                .add(user::Column::Username.ilike(pattern.clone()))
                .add(user::Column::Email.ilike(pattern)),
        );
    }

    if let Some(status) = status {
        query = query.filter(user::Column::Status.eq(status));
    }

    let total = query
        .clone()
        .count(db::get().await)
        .await
        .map_err(db::db_err)?;
    let users = query
        .order_by_desc(user::Column::MemberSince)
        .order_by_asc(user::Column::Username)
        .offset(page.saturating_mul(USER_PAGE_SIZE))
        .limit(USER_PAGE_SIZE)
        .all(db::get().await)
        .await
        .map_err(db::db_err)?;

    Ok(ModerationUserPage {
        users: users.iter().map(summary).collect(),
        total,
        page,
    })
}

/// Détail complet d'un utilisateur : profil, activité et parties.
pub async fn get_user_detail(
    actor: &user::Model,
    user_id: String,
) -> Result<ModerationUserDetail, ServerFnError> {
    let user_id = Uuid::parse_str(&user_id).map_err(|_| invalid_id())?;
    let db = db::get().await;

    let user = user::Entity::find_by_id(user_id)
        .one(db)
        .await
        .map_err(db::db_err)?
        .ok_or_else(|| not_found("Utilisateur introuvable"))?;

    let moderated_by = match user.moderated_by {
        Some(actor_id) => user::Entity::find_by_id(actor_id)
            .select_only()
            .column(user::Column::Username)
            .into_tuple::<String>()
            .one(db)
            .await
            .map_err(db::db_err)?,
        None => None,
    };

    let has_password = user_cred::Entity::find()
        .filter(user_cred::Column::UserId.eq(user_id))
        .count(db)
        .await
        .map_err(db::db_err)?
        > 0;

    let games = games_for_user(user_id, 25).await?;
    let logs = activity_log::Entity::find()
        .filter(activity_log::Column::UserId.eq(user_id))
        .order_by_desc(activity_log::Column::CreatedAt)
        .limit(50)
        .all(db)
        .await
        .map_err(db::db_err)?;

    let audit_rows = AuditRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT id, actor_username, action, details, target_username, target_game_id, created_at
        FROM audit_logs
        WHERE target_user_id = $1
           OR target_game_id IN (
                SELECT game_id FROM game_players WHERE user_id = $1
           )
        ORDER BY created_at DESC
        LIMIT 50
        "#,
        [user_id.into()],
    ))
    .all(db)
    .await
    .map_err(db::db_err)?;

    let mut timeline: Vec<ActivityEntry> = logs
        .into_iter()
        .map(|log| ActivityEntry {
            kind: log.kind.clone(),
            label: activity_label(&log.kind).to_string(),
            detail: log.description,
            occurred_at: log.created_at.max(0) as u64,
        })
        .collect();

    for game in &games {
        let player = game
            .players
            .iter()
            .find(|player| player.user_id == user_id.to_string());
        let outcome = match player {
            Some(player) if player.won => "victoire",
            Some(_) => "défaite",
            None => "participation",
        };
        timeline.push(ActivityEntry {
            kind: "game".to_string(),
            label: format!("Partie ({outcome})"),
            detail: format!(
                "Score {}-{} le {}",
                game.won_score,
                game.lost_score,
                crate::api::utils::format_date_and_hour(game.played_at)
            ),
            occurred_at: game.played_at,
        });
    }

    for row in audit_rows {
        timeline.push(ActivityEntry {
            kind: "moderation".to_string(),
            label: format!("Modération : {}", audit_label(&row.action)),
            detail: format!("Par {} — {}", row.actor_username, row.details),
            occurred_at: row.created_at.max(0) as u64,
        });
    }

    timeline.sort_by_key(|entry| std::cmp::Reverse(entry.occurred_at));
    timeline.truncate(TIMELINE_LIMIT);

    Ok(ModerationUserDetail {
        user: summary(&user),
        moderated_by,
        activity: timeline,
        games,
        has_password,
        is_self: actor.id == user_id,
    })
}

/// Modifie manuellement l'Elo et les compteurs d'un utilisateur.
pub async fn update_stats(
    actor: &user::Model,
    user_id: String,
    update: UserStatsUpdate,
) -> Result<Result<(), String>, ServerFnError> {
    let Ok(user_id) = Uuid::parse_str(&user_id) else {
        return Ok(Err("Identifiant utilisateur invalide".to_string()));
    };

    if update.elo < 0 {
        return Ok(Err("L'Elo ne peut pas être négatif".to_string()));
    }
    if update.games_played < 0 {
        return Ok(Err(
            "Le nombre de parties jouées ne peut pas être négatif".to_string()
        ));
    }
    if update.games_won < 0 || update.games_won > update.games_played {
        return Ok(Err(
            "Le nombre de victoires doit être compris entre 0 et le nombre de parties jouées"
                .to_string(),
        ));
    }
    if update.best_elo < update.elo {
        return Ok(Err(
            "Le record d'Elo doit être supérieur ou égal à l'Elo actuel".to_string(),
        ));
    }

    let db = db::get().await;
    let transaction = db.begin().await.map_err(db::db_err)?;

    let Some(target) = user::Entity::find_by_id(user_id)
        .one(&transaction)
        .await
        .map_err(db::db_err)?
    else {
        return Ok(Err("Utilisateur introuvable".to_string()));
    };

    let details = format!(
        "Elo {} → {} ; record {} → {} ; parties {} → {} ; victoires {} → {}",
        target.elo,
        update.elo,
        target.best_elo,
        update.best_elo,
        target.games_played,
        update.games_played,
        target.games_won,
        update.games_won
    );

    user::ActiveModel {
        id: Set(user_id),
        elo: Set(update.elo),
        best_elo: Set(update.best_elo),
        games_played: Set(update.games_played),
        games_won: Set(update.games_won),
        ..Default::default()
    }
    .update(&transaction)
    .await
    .map_err(db::db_err)?;

    audit::record_audit(
        &transaction,
        actor,
        "update_stats",
        Some(target.id),
        Some(target.username),
        None,
        details,
    )
    .await?;

    transaction.commit().await.map_err(db::db_err)?;
    Ok(Ok(()))
}

/// Force le nom d'utilisateur, sans demander son mot de passe à la cible.
pub async fn change_username(
    actor: &user::Model,
    user_id: String,
    new_username: String,
) -> Result<Result<String, String>, ServerFnError> {
    let Ok(user_id) = Uuid::parse_str(&user_id) else {
        return Ok(Err("Identifiant utilisateur invalide".to_string()));
    };
    let trimmed = new_username.trim().to_string();

    if let Err(e) = crate::server::auth::account::check_username_validity(&trimmed, user_id).await?
    {
        return Ok(Err(e));
    }

    if !trimmed.chars().all(|c| !c.is_control()) {
        return Ok(Err(
            "Le nom d'utilisateur contient des caractères de contrôle".to_string(),
        ));
    }

    let db = db::get().await;
    let transaction = db.begin().await.map_err(db::db_err)?;

    let Some(target) = user::Entity::find_by_id(user_id)
        .one(&transaction)
        .await
        .map_err(db::db_err)?
    else {
        return Ok(Err("Utilisateur introuvable".to_string()));
    };

    let old_username = target.username.clone();
    let updated = user::ActiveModel {
        id: Set(user_id),
        username: Set(trimmed.clone()),
        ..Default::default()
    }
    .update(&transaction)
    .await;

    if let Err(error) = updated {
        if error.to_string().contains("users_username_unique") {
            return Ok(Err("Ce nom d'utilisateur est déjà utilisé".to_string()));
        }
        return Err(db::db_err(error));
    }

    audit::record_audit(
        &transaction,
        actor,
        "change_username",
        Some(user_id),
        Some(trimmed.clone()),
        None,
        format!("Nom « {old_username} » → « {trimmed} »"),
    )
    .await?;

    transaction.commit().await.map_err(db::db_err)?;
    Ok(Ok(trimmed))
}

/// Réinitialise le mot de passe d'un utilisateur.
///
/// Le hash existant est supprimé : à la prochaine connexion, le mot de passe
/// saisi par l'utilisateur devient son nouveau mot de passe. Les sessions
/// ouvertes sont fermées pour que l'ancien mot de passe ne donne plus accès à
/// un compte déjà connecté.
pub async fn reset_password(
    actor: &user::Model,
    user_id: String,
) -> Result<Result<(), String>, ServerFnError> {
    let Ok(user_id) = Uuid::parse_str(&user_id) else {
        return Ok(Err("Identifiant utilisateur invalide".to_string()));
    };

    let db = db::get().await;
    let transaction = db.begin().await.map_err(db::db_err)?;

    let Some(target) = user::Entity::find_by_id(user_id)
        .one(&transaction)
        .await
        .map_err(db::db_err)?
    else {
        return Ok(Err("Utilisateur introuvable".to_string()));
    };

    user_cred::Entity::delete_many()
        .filter(user_cred::Column::UserId.eq(user_id))
        .exec(&transaction)
        .await
        .map_err(db::db_err)?;

    session::Entity::delete_many()
        .filter(session::Column::UserId.eq(user_id))
        .exec(&transaction)
        .await
        .map_err(db::db_err)?;

    audit::record_audit(
        &transaction,
        actor,
        "reset_password",
        Some(user_id),
        Some(target.username),
        None,
        "Mot de passe réinitialisé. Le prochain mot de passe saisi à la connexion deviendra le nouveau mot de passe ; toutes les sessions ont été fermées.".to_string(),
    )
    .await?;

    transaction.commit().await.map_err(db::db_err)?;
    Ok(Ok(()))
}

/// Promeut ou rétrograde un utilisateur.
pub async fn set_role(
    actor: &user::Model,
    user_id: String,
    role: UserRole,
) -> Result<Result<(), String>, ServerFnError> {
    let Ok(user_id) = Uuid::parse_str(&user_id) else {
        return Ok(Err("Identifiant utilisateur invalide".to_string()));
    };

    let db = db::get().await;
    let transaction = db.begin().await.map_err(db::db_err)?;

    let Some(target) = user::Entity::find_by_id(user_id)
        .one(&transaction)
        .await
        .map_err(db::db_err)?
    else {
        return Ok(Err("Utilisateur introuvable".to_string()));
    };

    if target.id == actor.id && role == UserRole::User {
        return Ok(Err(
            "Vous ne pouvez pas retirer votre propre rôle administrateur".to_string(),
        ));
    }

    if access::is_env_admin(&target.email) && role != UserRole::Admin {
        return Ok(Err(
            "Cet utilisateur est administrateur via ADMIN_EMAILS et ne peut pas être rétrogradé"
                .to_string(),
        ));
    }

    if target.role == role {
        return Ok(Ok(()));
    }

    user::ActiveModel {
        id: Set(user_id),
        role: Set(role),
        ..Default::default()
    }
    .update(&transaction)
    .await
    .map_err(db::db_err)?;

    audit::record_audit(
        &transaction,
        actor,
        "set_role",
        Some(user_id),
        Some(target.username),
        None,
        format!("Rôle {} → {}", target.role.label_fr(), role.label_fr()),
    )
    .await?;

    transaction.commit().await.map_err(db::db_err)?;
    Ok(Ok(()))
}

/// Applique une suspension ou un bannissement.
pub async fn apply_sanction(
    actor: &user::Model,
    user_id: String,
    sanction: SanctionInput,
) -> Result<Result<(), String>, ServerFnError> {
    let Ok(user_id) = Uuid::parse_str(&user_id) else {
        return Ok(Err("Identifiant utilisateur invalide".to_string()));
    };
    let kind = sanction.kind;
    let reason = sanction.reason.trim().to_string();

    if reason.chars().count() < 3 {
        return Ok(Err(
            "Le motif doit contenir au moins 3 caractères".to_string()
        ));
    }
    if reason.chars().count() > 500 {
        return Ok(Err("Le motif est trop long".to_string()));
    }

    let db = db::get().await;
    let transaction = db.begin().await.map_err(db::db_err)?;

    let Some(target) = user::Entity::find_by_id(user_id)
        .one(&transaction)
        .await
        .map_err(db::db_err)?
    else {
        return Ok(Err("Utilisateur introuvable".to_string()));
    };

    if target.id == actor.id {
        return Ok(Err(
            "Vous ne pouvez pas vous sanctionner vous-même".to_string()
        ));
    }
    if access::is_env_admin(&target.email) || target.role == UserRole::Admin {
        return Ok(Err(
            "Retirez d'abord le rôle administrateur avant de sanctionner ce compte".to_string(),
        ));
    }

    let now = utils::current_time_secs() as i64;
    let (status, suspended_until, details) = match kind {
        SanctionKind::Ban => (
            UserStatus::Banned,
            None,
            format!("Bannissement définitif. Motif : {reason}"),
        ),
        SanctionKind::Suspend => {
            let duration = match sanction.duration_hours {
                Some(hours) if hours > 0 && hours <= 24 * 365 * 10 => Some(hours),
                Some(_) => return Ok(Err("Durée de suspension invalide".to_string())),
                None => None,
            };
            let until = duration.map(|hours| now + hours as i64 * 3600);
            let human_duration = duration
                .map(|hours| format!("{hours} h"))
                .unwrap_or_else(|| "durée indéterminée".to_string());
            (
                UserStatus::Suspended,
                until,
                format!("Suspension ({human_duration}). Motif : {reason}"),
            )
        }
    };

    user::ActiveModel {
        id: Set(user_id),
        status: Set(status),
        suspended_until: Set(suspended_until),
        status_reason: Set(Some(reason)),
        moderated_at: Set(Some(now)),
        moderated_by: Set(Some(actor.id)),
        ..Default::default()
    }
    .update(&transaction)
    .await
    .map_err(db::db_err)?;

    // Une sanction doit couper les sessions ouvertes immédiatement.
    session::Entity::delete_many()
        .filter(session::Column::UserId.eq(user_id))
        .exec(&transaction)
        .await
        .map_err(db::db_err)?;

    audit::record_audit(
        &transaction,
        actor,
        match kind {
            SanctionKind::Suspend => "suspend",
            SanctionKind::Ban => "ban",
        },
        Some(user_id),
        Some(target.username),
        None,
        details,
    )
    .await?;

    transaction.commit().await.map_err(db::db_err)?;
    Ok(Ok(()))
}

/// Lève une suspension ou un bannissement.
pub async fn lift_sanction(
    actor: &user::Model,
    user_id: String,
) -> Result<Result<(), String>, ServerFnError> {
    let Ok(user_id) = Uuid::parse_str(&user_id) else {
        return Ok(Err("Identifiant utilisateur invalide".to_string()));
    };

    let db = db::get().await;
    let transaction = db.begin().await.map_err(db::db_err)?;

    let Some(target) = user::Entity::find_by_id(user_id)
        .one(&transaction)
        .await
        .map_err(db::db_err)?
    else {
        return Ok(Err("Utilisateur introuvable".to_string()));
    };

    if target.status == UserStatus::Active {
        return Ok(Ok(()));
    }

    let now = utils::current_time_secs() as i64;
    user::ActiveModel {
        id: Set(user_id),
        status: Set(UserStatus::Active),
        suspended_until: Set(None),
        status_reason: Set(None),
        moderated_at: Set(Some(now)),
        moderated_by: Set(Some(actor.id)),
        ..Default::default()
    }
    .update(&transaction)
    .await
    .map_err(db::db_err)?;

    audit::record_audit(
        &transaction,
        actor,
        "lift_sanction",
        Some(user_id),
        Some(target.username),
        None,
        "Sanction levée, compte réactivé".to_string(),
    )
    .await?;

    transaction.commit().await.map_err(db::db_err)?;
    Ok(Ok(()))
}

/// Liste paginée des parties, recherche par joueur ou identifiant.
pub async fn list_games(
    _actor: &user::Model,
    search: String,
    page: u64,
) -> Result<ModerationGamePage, ServerFnError> {
    let search = search.trim();
    let pattern = format!("%{}%", db::escape_like(search));
    let offset = page.saturating_mul(GAME_PAGE_SIZE) as i64;

    let rows = GameListRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT
            g.id,
            g.won_score,
            g.lost_score,
            g.played_at,
            u.username AS recorded_by,
            COUNT(*) OVER() AS total
        FROM games g
        JOIN users u ON u.id = g.recorded_by
        WHERE (
            $1 = ''
            OR u.username ILIKE $2
            OR g.id::text ILIKE $2
            OR EXISTS (
                SELECT 1
                FROM game_players gp
                JOIN users pu ON pu.id = gp.user_id
                WHERE gp.game_id = g.id AND pu.username ILIKE $2
            )
        )
        ORDER BY g.played_at DESC, g.id DESC
        LIMIT $3 OFFSET $4
        "#,
        [
            search.to_string().into(),
            pattern.into(),
            (GAME_PAGE_SIZE as i64).into(),
            offset.into(),
        ],
    ))
    .all(db::get().await)
    .await
    .map_err(db::db_err)?;

    let total = rows.first().map(|row| row.total.max(0) as u64).unwrap_or(0);
    let games =
        assemble_game_summaries(rows.into_iter().map(GameSummaryRow::from).collect()).await?;

    Ok(ModerationGamePage { games, total, page })
}

/// Récupère une partie avec ses participants pour l'éditeur.
pub async fn get_game(
    _actor: &user::Model,
    game_id: String,
) -> Result<ModerationGameSummary, ServerFnError> {
    let game_id = Uuid::parse_str(&game_id).map_err(|_| invalid_id())?;

    let row = GameSummaryRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT g.id, g.won_score, g.lost_score, g.played_at, u.username AS recorded_by
        FROM games g
        JOIN users u ON u.id = g.recorded_by
        WHERE g.id = $1
        "#,
        [game_id.into()],
    ))
    .one(db::get().await)
    .await
    .map_err(db::db_err)?
    .ok_or_else(|| not_found("Partie introuvable"))?;

    assemble_game_summaries(vec![row])
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| not_found("Partie introuvable"))
}

/// Modifie le score et les participants d'une partie.
///
/// Les effets de l'ancienne partie sont d'abord annulés pour tous les joueurs,
/// puis la nouvelle version est calculée et appliquée. Le tout dans une seule
/// transaction avec verrouillage des lignes utilisateurs concernées.
pub async fn update_game(
    actor: &user::Model,
    input: GameEditInput,
) -> Result<Result<(), String>, ServerFnError> {
    let Ok(game_id) = Uuid::parse_str(&input.game_id) else {
        return Ok(Err("Identifiant de partie invalide".to_string()));
    };

    if input.won_score > 13 || input.lost_score > 13 {
        return Ok(Err("Le score maximum est de 13.".to_string()));
    }
    if input.won_score <= input.lost_score {
        return Ok(Err(
            "Le score de l'équipe gagnante doit être supérieur à celui de l'équipe perdante"
                .to_string(),
        ));
    }
    if input.won_score < 3 {
        return Ok(Err("Le score minimum pour gagner est de 3".to_string()));
    }

    let mut seen = HashSet::new();
    if input
        .winning_team
        .iter()
        .chain(input.losing_team.iter())
        .any(|username| !seen.insert(username.clone()))
    {
        return Ok(Err(
            "Un joueur ne peut pas apparaître deux fois dans une partie".to_string(),
        ));
    }

    let sent_game = GameSendItem {
        left_score: input.won_score,
        right_score: input.lost_score,
        left_team: input.winning_team.clone(),
        right_team: input.losing_team.clone(),
    };
    if let Err(e) = elo::verify_game(&sent_game) {
        return Ok(Err(e));
    }

    let [winning_users, losing_users] =
        match db::map_usernames(input.winning_team.clone(), input.losing_team.clone()).await? {
            Ok(teams) => teams,
            Err(e) => return Ok(Err(e)),
        };

    let db = db::get().await;
    let transaction = db.begin().await.map_err(db::db_err)?;

    let Some(old_game) = game::Entity::find_by_id(game_id)
        .one(&transaction)
        .await
        .map_err(db::db_err)?
    else {
        return Ok(Err("Partie introuvable".to_string()));
    };

    let old_players = game_player::Entity::find()
        .filter(game_player::Column::GameId.eq(game_id))
        .all(&transaction)
        .await
        .map_err(db::db_err)?;

    let mut affected_ids: Vec<Uuid> = old_players.iter().map(|player| player.user_id).collect();
    for player in winning_users.iter().chain(losing_users.iter()) {
        if !affected_ids.contains(&player.id) {
            affected_ids.push(player.id);
        }
    }

    if affected_ids.is_empty() {
        return Ok(Err("La partie doit contenir au moins un joueur".to_string()));
    }

    // Verrouille toutes les lignes utilisateurs pour éviter qu'une partie
    // simultanée ne lise un Elo en cours de modification.
    let locked_users = user::Entity::find()
        .filter(user::Column::Id.is_in(affected_ids.clone()))
        .lock_exclusive()
        .all(&transaction)
        .await
        .map_err(db::db_err)?;

    if locked_users.len() != affected_ids.len() {
        return Ok(Err(
            "Un ou plusieurs joueurs n'existent plus dans la base de données".to_string(),
        ));
    }

    // 1. Annuler l'ancienne partie.
    for player in &old_players {
        reverse_game_effect(&transaction, player.user_id, player.elo_change, player.won).await?;
    }

    // 2. Relire les Elo après annulation pour calculer les nouveaux changes.
    let refreshed_users = user::Entity::find()
        .filter(user::Column::Id.is_in(affected_ids))
        .all(&transaction)
        .await
        .map_err(db::db_err)?;
    let elo_by_id: HashMap<Uuid, u64> = refreshed_users
        .into_iter()
        .map(|user| (user.id, user.elo.max(0) as u64))
        .collect();

    let winning_with_elo: Vec<db::UserWithElo> = winning_users
        .iter()
        .map(|user| db::UserWithElo {
            id: user.id,
            elo: *elo_by_id.get(&user.id).unwrap_or(&0),
        })
        .collect();
    let losing_with_elo: Vec<db::UserWithElo> = losing_users
        .iter()
        .map(|user| db::UserWithElo {
            id: user.id,
            elo: *elo_by_id.get(&user.id).unwrap_or(&0),
        })
        .collect();

    let updates = elo::compute_changes(
        input.won_score,
        input.lost_score,
        winning_with_elo,
        losing_with_elo,
    );

    // 3. Remplacer les participants et le score.
    game_player::Entity::delete_many()
        .filter(game_player::Column::GameId.eq(game_id))
        .exec(&transaction)
        .await
        .map_err(db::db_err)?;

    game::ActiveModel {
        id: Set(game_id),
        won_score: Set(input.won_score as i64),
        lost_score: Set(input.lost_score as i64),
        recorded_by: Set(old_game.recorded_by),
        played_at: Set(old_game.played_at),
    }
    .update(&transaction)
    .await
    .map_err(db::db_err)?;

    game_player::Entity::insert_many(updates.iter().map(|update| game_player::ActiveModel {
        game_id: Set(game_id),
        user_id: Set(update.id),
        elo_change: Set(update.elo_change),
        won: Set(update.won),
    }))
    .exec(&transaction)
    .await
    .map_err(db::db_err)?;

    // 4. Appliquer les nouveaux effets.
    for update in &updates {
        apply_game_effect(&transaction, update.id, update.elo_change, update.won).await?;
    }

    audit::record_audit(
        &transaction,
        actor,
        "update_game",
        None,
        None,
        Some(game_id),
        format!(
            "Partie {} : score {}-{} → {}-{} ; gagnants [{}] ; perdants [{}]",
            game_id,
            old_game.won_score,
            old_game.lost_score,
            input.won_score,
            input.lost_score,
            input.winning_team.join(", "),
            input.losing_team.join(", ")
        ),
    )
    .await?;

    transaction.commit().await.map_err(db::db_err)?;
    Ok(Ok(()))
}

/// Supprime une partie et annule ses effets sur l'Elo et les compteurs.
pub async fn delete_game(
    actor: &user::Model,
    game_id: String,
) -> Result<Result<(), String>, ServerFnError> {
    let Ok(game_id) = Uuid::parse_str(&game_id) else {
        return Ok(Err("Identifiant de partie invalide".to_string()));
    };

    let db = db::get().await;
    let transaction = db.begin().await.map_err(db::db_err)?;

    let Some(game_record) = game::Entity::find_by_id(game_id)
        .one(&transaction)
        .await
        .map_err(db::db_err)?
    else {
        return Ok(Err("Partie introuvable".to_string()));
    };

    let players = game_player::Entity::find()
        .filter(game_player::Column::GameId.eq(game_id))
        .all(&transaction)
        .await
        .map_err(db::db_err)?;

    let user_ids: Vec<Uuid> = players.iter().map(|player| player.user_id).collect();
    let locked_users = user::Entity::find()
        .filter(user::Column::Id.is_in(user_ids.clone()))
        .lock_exclusive()
        .all(&transaction)
        .await
        .map_err(db::db_err)?;

    if locked_users.len() != user_ids.len() {
        return Ok(Err(
            "Un ou plusieurs joueurs n'existent plus dans la base de données".to_string(),
        ));
    }

    for player in &players {
        reverse_game_effect(&transaction, player.user_id, player.elo_change, player.won).await?;
    }

    // Les participations sont supprimées en cascade par PostgreSQL.
    game::Entity::delete_by_id(game_id)
        .exec(&transaction)
        .await
        .map_err(db::db_err)?;

    audit::record_audit(
        &transaction,
        actor,
        "delete_game",
        None,
        None,
        Some(game_id),
        format!(
            "Partie {} supprimée (score {}-{}, {} joueur(s))",
            game_id,
            game_record.won_score,
            game_record.lost_score,
            players.len()
        ),
    )
    .await?;

    transaction.commit().await.map_err(db::db_err)?;
    Ok(Ok(()))
}

/// Dernières actions de modération, pour le tableau de bord.
pub async fn recent_audit(
    _actor: &user::Model,
    page: u64,
) -> Result<Vec<AuditEntry>, ServerFnError> {
    let logs = audit_log::Entity::find()
        .order_by_desc(audit_log::Column::CreatedAt)
        .offset(page.saturating_mul(20))
        .limit(20)
        .all(db::get().await)
        .await
        .map_err(db::db_err)?;

    Ok(logs
        .into_iter()
        .map(|log| AuditEntry {
            id: log.id.to_string(),
            actor_username: log.actor_username,
            action: audit_label(&log.action).to_string(),
            details: log.details,
            target_username: log.target_username,
            target_game_id: log.target_game_id.map(|id| id.to_string()),
            created_at: log.created_at.max(0) as u64,
        })
        .collect())
}

/// Annule les effets d'une participation : Elo, parties jouées et victoires.
///
/// `games_won` est borné par le nouveau `games_played` pour rester valide même
/// si un administrateur a édité les statistiques à la main entre-temps.
async fn reverse_game_effect(
    conn: &impl sea_orm::ConnectionTrait,
    user_id: Uuid,
    elo_change: i64,
    won: bool,
) -> Result<(), ServerFnError> {
    conn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        UPDATE users
        SET elo = GREATEST(0, elo - $1),
            games_played = GREATEST(0, games_played - 1),
            games_won = LEAST(
                GREATEST(0, games_won - $2),
                GREATEST(0, games_played - 1)
            )
        WHERE id = $3
        "#,
        [elo_change.into(), i64::from(won).into(), user_id.into()],
    ))
    .await
    .map_err(db::db_err)?;

    Ok(())
}

/// Applique les effets d'une nouvelle participation.
async fn apply_game_effect(
    conn: &impl sea_orm::ConnectionTrait,
    user_id: Uuid,
    elo_change: i64,
    won: bool,
) -> Result<(), ServerFnError> {
    conn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        UPDATE users
        SET elo = GREATEST(0, elo + $1),
            games_played = games_played + 1,
            games_won = games_won + $2
        WHERE id = $3
        "#,
        [elo_change.into(), i64::from(won).into(), user_id.into()],
    ))
    .await
    .map_err(db::db_err)?;

    Ok(())
}

/// Récupère les parties d'un utilisateur pour l'inspecteur.
async fn games_for_user(
    user_id: Uuid,
    limit: u64,
) -> Result<Vec<ModerationGameSummary>, ServerFnError> {
    let rows = GameSummaryRow::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        SELECT g.id, g.won_score, g.lost_score, g.played_at, u.username AS recorded_by
        FROM games g
        JOIN users u ON u.id = g.recorded_by
        JOIN game_players gp ON gp.game_id = g.id
        WHERE gp.user_id = $1
        ORDER BY g.played_at DESC, g.id DESC
        LIMIT $2
        "#,
        [user_id.into(), (limit as i64).into()],
    ))
    .all(db::get().await)
    .await
    .map_err(db::db_err)?;

    assemble_game_summaries(rows).await
}

/// Complète les lignes de partie avec la liste de leurs participants.
async fn assemble_game_summaries(
    rows: Vec<GameSummaryRow>,
) -> Result<Vec<ModerationGameSummary>, ServerFnError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let game_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let participants = game_player::Entity::find()
        .find_also_related(user::Entity)
        .filter(game_player::Column::GameId.is_in(game_ids))
        .all(db::get().await)
        .await
        .map_err(db::db_err)?;

    let mut players_by_game: HashMap<Uuid, Vec<ModerationGamePlayer>> = HashMap::new();
    for (participation, user) in participants {
        let Some(user) = user else {
            continue;
        };
        players_by_game
            .entry(participation.game_id)
            .or_default()
            .push(ModerationGamePlayer {
                user_id: user.id.to_string(),
                username: user.username,
                elo_change: participation.elo_change,
                won: participation.won,
            });
    }

    for players in players_by_game.values_mut() {
        players.sort_by(|a, b| b.won.cmp(&a.won).then_with(|| a.username.cmp(&b.username)));
    }

    Ok(rows
        .into_iter()
        .map(|row| ModerationGameSummary {
            id: row.id.to_string(),
            won_score: row.won_score,
            lost_score: row.lost_score,
            played_at: row.played_at.max(0) as u64,
            recorded_by: row.recorded_by,
            players: players_by_game.remove(&row.id).unwrap_or_default(),
        })
        .collect())
}

#[derive(FromQueryResult)]
struct GameSummaryRow {
    id: Uuid,
    won_score: i64,
    lost_score: i64,
    played_at: i64,
    recorded_by: String,
}

#[derive(FromQueryResult)]
struct GameListRow {
    id: Uuid,
    won_score: i64,
    lost_score: i64,
    played_at: i64,
    recorded_by: String,
    total: i64,
}

impl From<GameListRow> for GameSummaryRow {
    fn from(row: GameListRow) -> Self {
        Self {
            id: row.id,
            won_score: row.won_score,
            lost_score: row.lost_score,
            played_at: row.played_at,
            recorded_by: row.recorded_by,
        }
    }
}

#[derive(FromQueryResult)]
struct AuditRow {
    actor_username: String,
    action: String,
    details: String,
    created_at: i64,
}
