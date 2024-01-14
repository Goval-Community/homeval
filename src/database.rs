use anyhow::Result;
use log::{debug, warn};
use migration::MigratorTrait;
use sea_orm::{ConnectOptions, Database};
use std::time::Duration;
use tokio::sync::OnceCell;

pub static DATABASE: OnceCell<sea_orm::DatabaseConnection> = OnceCell::const_new();

// TODO: allow disabling of db at runtime as well as compile time
pub async fn setup() -> Result<()> {
    let db_url = match std::env::var("HOMEVAL_DB") {
        Ok(url) => url,
        Err(err) => {
            warn!(
                "Encountered error fetching $HOMEVAL_DB: `{}`. Disabling database integration.",
                err
            );
            return Ok(());
        }
    };

    let connect_options = ConnectOptions::new(db_url)
        .acquire_timeout(Duration::from_secs(5))
        .sqlx_logging_level(log::LevelFilter::Trace)
        .to_owned();

    debug!("Connecting to database");

    // Hard fail on db connection failure because if $HOMEVAL_DB is supplied it
    // is expected that the user wants a db connection so missing one is an issue.
    let db = Database::connect(connect_options).await?;

    debug!("Running migrations");
    migration::Migrator::up(&db, None).await?;

    debug!("Setting database once cell");
    DATABASE.set(db).unwrap();

    debug!("Done with database setup");

    Ok(())
}
