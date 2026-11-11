use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

pub async fn start(pool: &PgPool, library_id: Uuid, mode: &str) -> Result<Uuid> {
    let id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO scan_runs (id, library_id, mode, status)
         VALUES ($1, $2, $3, 'running')",
    )
    .bind(id)
    .bind(library_id)
    .bind(mode)
    .execute(pool)
    .await
    .map_err(|error| db_error("start scan run", error))?;

    Ok(id)
}

pub async fn finish(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    counters: &serde_json::Value,
    error: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "UPDATE scan_runs
         SET status = $2, counters = $3, error = $4, finished_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(counters)
    .bind(error)
    .execute(pool)
    .await
    .map_err(|db_error_value| db_error("finish scan run", db_error_value))?;

    Ok(())
}
