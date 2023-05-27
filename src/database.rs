use async_once::AsyncOnce;
use lazy_static::lazy_static;
use log::{error, info};
use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, DatabaseConnection};

lazy_static! {
    pub static ref DATABASE_CONNECTION: AsyncOnce<Option<DatabaseConnection>> =
        AsyncOnce::new(async {
            info!("Setting up database");
            match std::env::var("HOMEVAL_DB") {
                Ok(db_url) => Some(Database::connect(db_url).await.unwrap()),
                Err(err) => {
                    error!("Error fetching db url env var, disabling db. {}", err);
                    None
                }
            }
        });
}
