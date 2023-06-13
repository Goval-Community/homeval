use deno_core::error::AnyError;
use log::debug;
use migration::MigratorTrait;
use sea_orm::{ConnectOptions, Database};
use std::time::Duration;
use tokio::sync::OnceCell;

pub static DATABASE: OnceCell<sea_orm::DatabaseConnection> = OnceCell::const_new();

// TODO: allow disabling of db at runtime as well as compile time
pub async fn setup() -> Result<(), AnyError> {
    let connect_options = ConnectOptions::new(std::env::var("HOMEVAL_DB")?)
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging_level(log::LevelFilter::Trace)
        .to_owned();

    debug!("Connecting to database");
    let db = Database::connect(connect_options).await?;

    debug!("Running migrations");
    migration::Migrator::up(&db, None).await?;

    debug!("Setting database once cell");
    DATABASE.set(db).unwrap();

    debug!("Done with database setup");
    Ok(())
}
