pub use sea_orm_migration::prelude::*;

mod m20230611_000001_create_files_table;
mod m20230616_000049_create_repldb_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230611_000001_create_files_table::Migration),
            Box::new(m20230616_000049_create_repldb_table::Migration),
        ]
    }
}
