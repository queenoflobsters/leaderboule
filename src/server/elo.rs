use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;

use crate::{
    api::db::global::GameSendItem,
    server::{auth::UserId, db::UserWithElo},
};

pub const DEFAULT_ELO: u64 = 400;

/// Le résultat des performances d'un utilisateur pendant une partie
#[derive(Serialize, Deserialize, Clone, SurrealValue)]
pub struct UserGameLog {
    pub id: UserId,
    pub elo_change: i64,
    pub won: bool,
}

impl UserGameLog {
    /// Construit à partir des données de la game
    pub fn construct(
        this: &UserWithElo,
        this_score: u64,
        other_score: u64,
        this_team_users: &[UserWithElo],
        other_team_users: &[UserWithElo],
    ) -> Self {
        let this_team = this_team_users
            .iter()
            .filter(|mates| this.id != mates.id)
            .map(|u| u.elo)
            .collect();
        let other_team = other_team_users.iter().map(|u| u.elo).collect();
        let this_change =
            compute_user_new_elo(this.elo, this_score, other_score, this_team, other_team);
        UserGameLog {
            id: this.id.clone(),
            elo_change: this_change,
            won: this_score > other_score,
        }
    }
}

/// La partie est-elle valide ????
pub fn verify_game(game_item: &GameSendItem) -> Result<(), String> {
    if game_item.left_score > 13 || game_item.right_score > 13 {
        return Err("Le score maximum est de 13.".to_string());
    }

    if game_item.left_score == game_item.right_score {
        return Err("Les deux équipes doivent avoir un score différent".to_string());
    }

    if std::cmp::max(game_item.left_score, game_item.right_score) < 3 {
        return Err("Le score minimum pour gagner est de 3".to_string());
    }

    let mut seen = HashSet::new();
    let has_duplicates = game_item
        .left_team
        .iter()
        .chain(game_item.right_team.iter())
        .any(|item| !seen.insert(item));
    if has_duplicates {
        return Err("Un joueur ne peut pas apparaître deux fois dans une partie".to_string());
    }

    if game_item.left_team.is_empty() || game_item.right_team.is_empty() {
        return Err("Les deux équipes doivent contenir au moins un joueur".to_string());
    }

    if game_item.left_team.len() > 6 || game_item.right_team.len() > 6 {
        return Err("La taille maximum d'une équipe est de six personnes".to_string());
    }

    Ok(())
}

pub fn compute_changes(
    left_score: u64,
    right_score: u64,
    left_team_users: Vec<UserWithElo>,
    right_team_users: Vec<UserWithElo>,
) -> Vec<UserGameLog> {
    left_team_users
        .iter()
        .map(|this| {
            UserGameLog::construct(
                this,
                left_score,
                right_score,
                &left_team_users,
                &right_team_users,
            )
        })
        .chain(right_team_users.iter().map(|this| {
            UserGameLog::construct(
                this,
                right_score,
                left_score,
                &right_team_users,
                &left_team_users,
            )
        }))
        .collect()
}

/// Calcule la variation d'elo
#[allow(unused)] // TODO REMOVE
fn compute_user_new_elo(
    this_elo: u64,
    this_score: u64,
    other_score: u64,
    this_team: Vec<u64>,
    other_team: Vec<u64>,
) -> i64 {
    // TODO TODO TODO TODO TODO
    let has_won = this_score > other_score;
    // TODO TODO TODO TODO TODO
    if has_won {
        // TODO TODO TODO TODO TODO
        100
    // TODO TODO TODO TODO TODO
    } else {
        // TODO TODO TODO TODO TODO
        -100
        // TODO TODO TODO TODO TODO
    }
}
