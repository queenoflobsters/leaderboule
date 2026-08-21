use std::{collections::HashSet};

use crate::api::db::global::GameSendItem;

pub const DEFAULT_ELO: u64 = 400;

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
    let has_duplicates = game_item.left_team.iter().chain(game_item.right_team.iter()).any(|item| !seen.insert(item));
    if has_duplicates {
        return Err("Un joueur ne peut pas apparaître deux fois dans une partie".to_string());
    }

    if game_item.left_team.is_empty() || game_item.right_team.is_empty() {
        return Err("Les deux équipes doivent contenir au moins un joueur".to_string());
    }

    if game_item.left_team.len() > 6 || game_item.right_team.len() > 6 {
        return Err("La taille maximum d'une équipe est de six personnes".to_string())
    }

    Ok(())
    
}
