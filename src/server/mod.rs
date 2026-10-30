/// Sous-système d'authentification
pub mod auth;

/// Sous-système de communication avec la base de donnée
pub mod db;

/// Entités SeaORM décrivant le schéma PostgreSQL
pub mod entities;

/// Migrations de la base de données
pub mod migration;

/// Sous-système pour communiquer avec l'API Helloasso
pub mod helloasso;

/// Sous-système de gestion de l'elo
pub mod elo;

/// Utilitaires serveur
pub mod utils;
