/// Contrôle d'accès, rôles et sanctions
pub mod access;

/// Journaux d'activité et d'audit
pub mod audit;

/// Sous-système d'authentification
pub mod auth;

/// Sous-système de communication avec la base de donnée
pub mod db;

/// Logique métier de la modération
pub mod moderation;

/// Entités SeaORM décrivant le schéma PostgreSQL
pub mod entities;

/// Sous-système pour communiquer avec l'API Helloasso
pub mod helloasso;

/// Sous-système de gestion de l'elo
pub mod elo;

/// Utilitaires serveur
pub mod utils;
