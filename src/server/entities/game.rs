use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "games")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub won_score: i64,
    pub lost_score: i64,
    pub recorded_by: Uuid,
    pub played_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::RecordedBy",
        to = "super::user::Column::Id"
    )]
    RecordedBy,
    #[sea_orm(has_many = "super::game_player::Entity")]
    GamePlayer,
}

impl Related<super::game_player::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::GamePlayer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
