///
/// Les composants ne seront pas documentés car c'est suffisamment auto-descriptif
/// Puis franchement j'ai trop la flemme
///

/// Gêrant le routing
pub mod route;

/// Composant de la page du compte
pub mod account;

/// Composant du classement
pub mod leaderboard;

/// Composant pour la connexion
pub mod login;

/// Blank page qui déconnecte simplement
pub mod logout;

/// Barre latérale de navigation
pub mod navbar;

/// 404: Je me sens terriblement seul
pub mod page_not_found;

/// Composant pour rentrer une nouvelle partie
pub mod new_game;

/// Composant pour l'historique des parties
pub mod history;
