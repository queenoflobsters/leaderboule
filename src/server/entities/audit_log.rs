use sea_orm::entity::prelude::*;

/// Trace immuable d'une action de modération.
///
/// Les noms d'utilisateur sont dénormalisés : l'audit doit rester lisible
/// même après un renommage ou une suppression de compte.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "audit_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub actor_username: String,
    pub target_user_id: Option<Uuid>,
    pub target_username: Option<String>,
    pub target_game_id: Option<Uuid>,
    pub action: String,
    pub details: String,
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
