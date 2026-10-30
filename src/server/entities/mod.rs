//! Entités SeaORM décrivant le schéma PostgreSQL.
//!
//! Les colonnes calculées par PostgreSQL (`games_lost`, `win_ratio`) ne sont
//! volontairement pas exposées ici : elles ne doivent jamais être écrites par
//! l'application.

pub mod game;
pub mod game_player;
pub mod session;
pub mod user;
pub mod user_cred;
