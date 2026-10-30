use std::cmp::Ordering;
use std::collections::HashSet;

use uuid::Uuid;

use crate::{api::db::global::GameSendItem, server::db::UserWithElo};

pub const DEFAULT_ELO: u64 = 400;

/// Échelle logistique standard d'Elo.
const ELO_SCALE: f64 = 400.0;

/// K de base : à E = 0,5 et M = 1, une partie 1v1 échange 100 points.
const K_BASE: f64 = 200.0;

/// Marge de référence : 13-8 (5 points concédés) => multiplicateur 1.
const MOV_REFERENCE: f64 = 5.0;
/// Courbe concave : les écarts extrêmes comptent plus, mais pas linéairement.
const MOV_EXPONENT: f64 = 0.7;
const MOV_MIN: f64 = 0.5;
const MOV_MAX: f64 = 1.9;

/// Bonus de cohésion : une équipe hétérogène est considérée comme plus forte,
/// ce qui réduit ses gains et augmente ses pertes.
const DISPERSION_WEIGHT: f64 = 0.25;
const DISPERSION_CAP: f64 = 200.0;

/// Mélange entre contribution individuelle (attente personnelle) et part
/// égalitaire. Évite les distributions trop extrêmes.
const INDIVIDUAL_WEIGHT_BLEND: f64 = 0.65;

/// Petite marge pour garantir une probabilité strictement positive/négative.
const EXPECTATION_EPSILON: f64 = 1e-6;

/// Petit crédit accordé à chaque joueur quand le gagnant atteint 13 points.
/// C'est le seul composant volontairement non conservatif du système.
const COMPLETION_BONUS: i64 = 1;

/// Placement : les parties contenant des joueurs peu expérimentés ont des
/// enjeux plus élevés. Le facteur converge vers 1 après ~60 parties.
const PLACEMENT_MAX_BONUS: f64 = 0.5;
const PLACEMENT_DECAY_GAMES: f64 = 5.0;

/// Le résultat des performances d'un utilisateur pendant une partie.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserGameLog {
    pub id: Uuid,
    pub elo_change: i64,
    pub won: bool,
}

/// Vérifie si la partie est valide.
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

/// Calcule les variations d'Elo de tous les participants.
pub fn compute_changes(
    left_score: u64,
    right_score: u64,
    left_team_users: Vec<UserWithElo>,
    right_team_users: Vec<UserWithElo>,
) -> Vec<UserGameLog> {
    debug_assert!(left_score != right_score);
    debug_assert!(!left_team_users.is_empty() && !right_team_users.is_empty());

    let left_won = left_score > right_score;
    let winner_score = left_score.max(right_score);
    let loser_score = left_score.min(right_score);
    let finished = winner_score == 13;

    let mov = mov_multiplier(loser_score);
    let left_rating = effective_team_rating(&left_team_users);
    let right_rating = effective_team_rating(&right_team_users);
    let left_expected = expected_score(left_rating, right_rating);
    let right_expected = 1.0 - left_expected;

    let placement = placement_factor(&left_team_users, &right_team_users);
    let k_match =
        K_BASE * ((left_team_users.len() * right_team_users.len()) as f64).sqrt() * placement;

    let left_pool = if left_won {
        k_match * (1.0 - left_expected) * mov
    } else {
        -k_match * left_expected * mov
    };
    let right_pool = if left_won {
        -k_match * right_expected * mov
    } else {
        k_match * (1.0 - right_expected) * mov
    };

    let left_deltas = distribute_team(
        &left_team_users,
        right_rating,
        left_pool,
        left_won,
        finished,
    );
    let right_deltas = distribute_team(
        &right_team_users,
        left_rating,
        right_pool,
        !left_won,
        finished,
    );

    let mut updates = Vec::with_capacity(left_team_users.len() + right_team_users.len());

    for (player, elo_change) in left_team_users.iter().zip(left_deltas) {
        updates.push(UserGameLog {
            id: player.id,
            elo_change,
            won: left_won,
        });
    }

    for (player, elo_change) in right_team_users.iter().zip(right_deltas) {
        updates.push(UserGameLog {
            id: player.id,
            elo_change,
            won: !left_won,
        });
    }

    updates
}

/// Multiplicateur d'écart au score.
///
/// `loser_score` est le nombre de points de l'équipe perdante. La marge utile
/// est donc `13 - loser_score`, ce qui rend une partie inachevée à 10-7
/// équivalente à une partie terminée 13-7 : seule la difficulté réelle de la
/// partie compte. Un petit bonus de finition récompense ensuite les parties
/// qui vont jusqu'à 13.
fn mov_multiplier(loser_score: u64) -> f64 {
    let margin = 13_u64.saturating_sub(loser_score) as f64;
    ((margin / MOV_REFERENCE).powf(MOV_EXPONENT)).clamp(MOV_MIN, MOV_MAX)
}

/// Probabilité de victoire de `rating` contre `opponent`.
fn expected_score(rating: f64, opponent: f64) -> f64 {
    let exponent = (opponent - rating) / ELO_SCALE;
    let probability = 1.0 / (1.0 + 10_f64.powf(exponent));
    probability.clamp(EXPECTATION_EPSILON, 1.0 - EXPECTATION_EPSILON)
}

/// Note effective d'une équipe.
///
/// La moyenne représente le niveau moyen ; le terme d'écart-type pénalise les
/// compositions hétérogènes en les considérant comme plus fortes. Une équipe
/// « star + débutants » est donc moins récompensée quand elle gagne et perd
/// davantage quand elle perd.
fn effective_team_rating(team: &[UserWithElo]) -> f64 {
    if team.is_empty() {
        return DEFAULT_ELO as f64;
    }

    let n = team.len() as f64;
    let mean = team.iter().map(|player| player.elo as f64).sum::<f64>() / n;
    let variance = team
        .iter()
        .map(|player| {
            let centered = player.elo as f64 - mean;
            centered * centered
        })
        .sum::<f64>()
        / n;
    let standard_deviation = variance.sqrt();

    mean + (DISPERSION_WEIGHT * standard_deviation).min(DISPERSION_CAP)
}

/// Facteur de placement basé sur l'expérience moyenne des participants.
fn placement_factor(left_team: &[UserWithElo], right_team: &[UserWithElo]) -> f64 {
    let total_players = left_team.len() + right_team.len();
    if total_players == 0 {
        return 1.0;
    }

    let total_games = left_team
        .iter()
        .chain(right_team.iter())
        .map(|player| player.games_played)
        .sum::<u64>();

    let average_games = total_games as f64 / total_players as f64;
    let bonus = PLACEMENT_MAX_BONUS * (-average_games / PLACEMENT_DECAY_GAMES).exp();

    (1.0 + bonus).clamp(1.0, 1.0 + PLACEMENT_MAX_BONUS)
}

/// Répartit le pool d'une équipe entre ses joueurs.
fn distribute_team(
    team: &[UserWithElo],
    opponent_rating: f64,
    pool: f64,
    won: bool,
    finished: bool,
) -> Vec<i64> {
    let n = team.len();
    if n == 0 {
        return Vec::new();
    }

    let contributions: Vec<f64> = team
        .iter()
        .map(|player| {
            let individual_expected = expected_score(player.elo as f64, opponent_rating);
            let contribution = if won {
                1.0 - individual_expected
            } else {
                individual_expected
            };
            contribution.clamp(EXPECTATION_EPSILON, 1.0 - EXPECTATION_EPSILON)
        })
        .collect();

    let total_contribution: f64 = contributions.iter().sum();
    let equal_share = 1.0 / n as f64;
    let weights: Vec<f64> = contributions
        .iter()
        .map(|contribution| {
            let individual_share = contribution / total_contribution;
            (1.0 - INDIVIDUAL_WEIGHT_BLEND) * equal_share
                + INDIVIDUAL_WEIGHT_BLEND * individual_share
        })
        .collect();

    let raw_deltas: Vec<f64> = weights.iter().map(|weight| pool * weight).collect();
    let target = pool.round() as i64;
    let mut deltas = round_preserving_sum(&raw_deltas, target);

    for delta in &mut deltas {
        if finished {
            *delta += COMPLETION_BONUS;
        }

        if won {
            *delta = (*delta).max(0);
        } else {
            *delta = (*delta).min(-1);
        }
    }

    deltas
}

/// Arrondit un vecteur de deltas en conservant exactement la somme cible.
fn round_preserving_sum(raw: &[f64], target: i64) -> Vec<i64> {
    let mut rounded: Vec<i64> = raw.iter().map(|value| value.floor() as i64).collect();
    let mut remaining = target - rounded.iter().sum::<i64>();

    if remaining <= 0 {
        // Cas défensif : ne devrait pas arriver avec les pools calculés.
        while remaining < 0 {
            if let Some((index, _)) = raw
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap_or(Ordering::Equal))
            {
                rounded[index] -= 1;
            }
            remaining += 1;
        }
        return rounded;
    }

    let mut order: Vec<usize> = (0..raw.len()).collect();
    order.sort_by(|&left, &right| {
        let left_fraction = raw[left] - raw[left].floor();
        let right_fraction = raw[right] - raw[right].floor();
        right_fraction
            .partial_cmp(&left_fraction)
            .unwrap_or(Ordering::Equal)
    });

    for index in 0..remaining as usize {
        rounded[order[index % order.len()]] += 1;
    }

    rounded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(uuid: u128, elo: u64) -> UserWithElo {
        UserWithElo {
            id: Uuid::from_u128(uuid),
            elo,
            games_played: 50,
        }
    }

    fn player_with_games(uuid: u128, elo: u64, games_played: u64) -> UserWithElo {
        UserWithElo {
            id: Uuid::from_u128(uuid),
            elo,
            games_played,
        }
    }

    fn deltas(
        left_score: u64,
        right_score: u64,
        left: Vec<UserWithElo>,
        right: Vec<UserWithElo>,
    ) -> Vec<i64> {
        compute_changes(left_score, right_score, left, right)
            .into_iter()
            .map(|update| update.elo_change)
            .collect()
    }

    #[test]
    fn equal_one_v_one_typical_game_is_about_100() {
        let changes = deltas(13, 8, vec![player(1, 1500)], vec![player(2, 1500)]);
        assert_eq!(changes, vec![101, -99]);
    }

    #[test]
    fn fanny_multiplier_is_substantial() {
        let changes = deltas(13, 0, vec![player(1, 1500)], vec![player(2, 1500)]);
        assert_eq!(changes, vec![191, -189]);
    }

    #[test]
    fn huge_upset_rewards_the_underdog() {
        let changes = deltas(13, 4, vec![player(1, 1300)], vec![player(2, 1700)]);
        assert!(changes[0] > 250, "underdog gain: {}", changes[0]);
        assert_eq!(changes[0] + changes[1], 2);
    }

    #[test]
    fn expected_favorite_close_win_gains_little() {
        let changes = deltas(13, 12, vec![player(1, 1700)], vec![player(2, 1300)]);
        assert!(changes[0] <= 12, "favorite gain: {}", changes[0]);
        assert!(changes[1] >= -10, "underdog loss: {}", changes[1]);
    }

    #[test]
    fn team_pool_is_shared_equally_for_equal_players() {
        let changes = deltas(
            13,
            8,
            vec![player(1, 1500), player(2, 1500)],
            vec![player(3, 1500), player(4, 1500)],
        );
        assert_eq!(changes, vec![101, 101, -99, -99]);
    }

    #[test]
    fn a_star_gains_less_than_a_carried_teammate() {
        let changes = deltas(
            13,
            8,
            vec![player(1, 2000), player(2, 1000)],
            vec![player(3, 1500), player(4, 1500)],
        );
        assert!(
            changes[0] < changes[1],
            "star: {}, teammate: {}",
            changes[0],
            changes[1]
        );
        assert!(changes[0] > 0 && changes[1] > 0);
    }

    #[test]
    fn winner_never_loses_even_in_extreme_cases() {
        let changes = deltas(13, 12, vec![player(1, 2500)], vec![player(2, 400)]);
        assert!(changes[0] >= 0, "winner delta: {}", changes[0]);
        assert!(changes[1] <= -1, "loser delta: {}", changes[1]);
    }

    #[test]
    fn unfinished_game_uses_loser_score_not_winner_score() {
        let unfinished = deltas(10, 7, vec![player(1, 1500)], vec![player(2, 1500)]);
        let finished = deltas(13, 7, vec![player(1, 1500)], vec![player(2, 1500)]);
        // Même marge (loser = 7), seul le bonus de finition diffère.
        assert_eq!(unfinished[0] + 1, finished[0]);
        assert_eq!(unfinished[1] + 1, finished[1]);
    }

    #[test]
    fn unfinished_match_exchange_is_zero_sum() {
        let changes = deltas(10, 7, vec![player(1, 1500)], vec![player(2, 1500)]);
        assert_eq!(changes.iter().sum::<i64>(), 0);
    }

    #[test]
    fn mov_is_monotonic_and_bounded() {
        let mut previous = 0.0;
        for loser_score in (0..=12).rev() {
            let multiplier = mov_multiplier(loser_score);
            assert!((MOV_MIN..=MOV_MAX).contains(&multiplier));
            assert!(multiplier >= previous);
            previous = multiplier;
        }
    }

    #[test]
    fn equal_three_v_three_keeps_individual_stakes_around_100() {
        let changes = deltas(
            13,
            8,
            vec![player(1, 1500), player(2, 1500), player(3, 1500)],
            vec![player(4, 1500), player(5, 1500), player(6, 1500)],
        );
        assert_eq!(changes, vec![101, 101, 101, -99, -99, -99]);
    }

    #[test]
    fn placement_factor_accelerates_new_players() {
        let new_player_changes = deltas(
            13,
            8,
            vec![player_with_games(1, 1500, 0)],
            vec![player_with_games(2, 1500, 0)],
        );
        let veteran_changes = deltas(
            13,
            8,
            vec![player_with_games(1, 1500, 100)],
            vec![player_with_games(2, 1500, 100)],
        );
        assert!(new_player_changes[0] > veteran_changes[0]);
    }
}
