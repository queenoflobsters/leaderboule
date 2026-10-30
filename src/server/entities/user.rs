use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub email: String,
    #[sea_orm(unique)]
    pub username: String,
    pub member_since: i64,
    pub elo: i64,
    pub best_elo: i64,
    pub games_played: i64,
    pub games_won: i64,
}

impl Model {
    /// Nombre de parties perdues (colonne calculée côté PostgreSQL).
    pub fn games_lost(&self) -> u64 {
        self.games_played.saturating_sub(self.games_won).max(0) as u64
    }

    /// Ratio de victoires en pourcentage, comme l'ancien champ `win_ratio`.
    pub fn win_ratio(&self) -> f32 {
        if self.games_played == 0 {
            0.0
        } else {
            self.games_won as f32 * 100.0 / self.games_played as f32
        }
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::user_cred::Entity")]
    UserCred,
    #[sea_orm(has_many = "super::session::Entity")]
    Session,
    #[sea_orm(has_many = "super::game_player::Entity")]
    GamePlayer,
}

impl Related<super::user_cred::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UserCred.def()
    }
}

impl Related<super::session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Session.def()
    }
}

impl Related<super::game_player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GamePlayer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
