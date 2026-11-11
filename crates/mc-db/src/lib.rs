pub mod repos;

use std::time::Duration;

use mc_core::{CoreError, Result};
use sqlx::postgres::PgPoolOptions;
use tracing::info;

pub use sqlx::PgPool;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

const MIGRATION_LOCK_ID: i64 = 0x6d63_5f6d_6967_7261;

pub async fn connect(database_url: &str, max_connections: u32) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(max_connections.max(1))
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url)
        .await
        .map_err(|error| CoreError::Unavailable(format!("postgres: {error}")))
}

pub async fn connect_and_migrate(database_url: &str, max_connections: u32) -> Result<PgPool> {
    let pool = connect(database_url, max_connections).await?;
    migrate(&pool).await?;
    Ok(pool)
}

pub async fn migrate(pool: &PgPool) -> Result<()> {
    let mut connection = pool
        .acquire()
        .await
        .map_err(|error| CoreError::Unavailable(format!("postgres: {error}")))?;

    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATION_LOCK_ID)
        .execute(&mut *connection)
        .await
        .map_err(|error| CoreError::Internal(format!("migration lock: {error}")))?;

    let result = MIGRATOR.run(&mut *connection).await;

    let unlock = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(MIGRATION_LOCK_ID)
        .execute(&mut *connection)
        .await;

    if let Err(error) = unlock {
        tracing::warn!(%error, "failed to release migration advisory lock");
    }

    match result {
        Ok(()) => {
            info!("database migrations applied");
            Ok(())
        }
        Err(error) => Err(CoreError::Internal(format!("migrations: {error}"))),
    }
}

pub async fn ping(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT 1").execute(pool).await.map(|_| ())
}
