//! Migrations de la base PostgreSQL.
//!
//! Le schéma complet vit dans `src/init.sql` afin de rester lisible et
//! facilement comparable à l'ancien schéma SurrealDB. SeaORM s'occupe de
//! suivre les migrations appliquées dans la table `seaql_migrations`.

use sea_orm_migration::prelude::*;

const INIT_SQL: &str = include_str!("../init.sql");

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20261030_000001_initial_schema::Migration)]
    }
}

mod m20261030_000001_initial_schema {
    use super::*;

    pub struct Migration;

    impl MigrationName for Migration {
        fn name(&self) -> &str {
            "m20261030_000001_initial_schema"
        }
    }

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .get_connection()
                .execute_unprepared(INIT_SQL)
                .await?;
            Ok(())
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    DROP TABLE IF EXISTS user_creds;
                    DROP TABLE IF EXISTS sessions;
                    DROP TABLE IF EXISTS game_players;
                    DROP TABLE IF EXISTS games;
                    DROP FUNCTION IF EXISTS users_sync_best_elo;
                    DROP TABLE IF EXISTS users;
                    "#,
                )
                .await?;
            Ok(())
        }
    }
}
