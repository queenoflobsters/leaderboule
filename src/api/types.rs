//! Types métier partagés entre le client et le serveur.

use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
use sea_orm::sea_query::StringLen;

/// Rôle applicatif d'un utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(
    feature = "server",
    derive(sea_orm::DeriveActiveEnum, sea_orm::EnumIter)
)]
#[cfg_attr(
    feature = "server",
    sea_orm(rs_type = "String", db_type = "String(StringLen::None)")
)]
pub enum UserRole {
    #[default]
    #[cfg_attr(feature = "server", sea_orm(string_value = "user"))]
    User,
    #[cfg_attr(feature = "server", sea_orm(string_value = "admin"))]
    Admin,
}

impl UserRole {
    pub const fn is_admin(self) -> bool {
        matches!(self, Self::Admin)
    }

    pub const fn label_fr(self) -> &'static str {
        match self {
            Self::User => "Utilisateur",
            Self::Admin => "Administrateur",
        }
    }
}

/// État de modération d'un compte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(
    feature = "server",
    derive(sea_orm::DeriveActiveEnum, sea_orm::EnumIter)
)]
#[cfg_attr(
    feature = "server",
    sea_orm(rs_type = "String", db_type = "String(StringLen::None)")
)]
pub enum UserStatus {
    #[default]
    #[cfg_attr(feature = "server", sea_orm(string_value = "active"))]
    Active,
    #[cfg_attr(feature = "server", sea_orm(string_value = "suspended"))]
    Suspended,
    #[cfg_attr(feature = "server", sea_orm(string_value = "banned"))]
    Banned,
}

impl UserStatus {
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }

    pub const fn label_fr(self) -> &'static str {
        match self {
            Self::Active => "Actif",
            Self::Suspended => "Suspendu",
            Self::Banned => "Banni",
        }
    }
}

/// Type de sanction appliqué à un compte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SanctionKind {
    Suspend,
    Ban,
}
