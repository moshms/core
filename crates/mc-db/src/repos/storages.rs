use mc_core::Result;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct StorageRecord {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    pub config: Value,
    pub access: String,
    pub status: String,
}

pub async fn list(pool: &PgPool) -> Result<Vec<StorageRecord>> {
    sqlx::query_as::<_, StorageRecord>(
        "SELECT id, name, kind, config, access, status
         FROM storages
         ORDER BY lower(name)",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list storages", error))
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<StorageRecord>> {
    sqlx::query_as::<_, StorageRecord>(
        "SELECT id, name, kind, config, access, status
         FROM storages
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load storage", error))
}

pub async fn create(
    pool: &PgPool,
    name: &str,
    kind: &str,
    config: Value,
    access: &str,
) -> Result<StorageRecord> {
    sqlx::query_as::<_, StorageRecord>(
        "INSERT INTO storages (id, name, kind, config, access, status)
         VALUES ($1, $2, $3, $4, $5, 'online')
         RETURNING id, name, kind, config, access, status",
    )
    .bind(Uuid::now_v7())
    .bind(name)
    .bind(kind)
    .bind(config)
    .bind(access)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("create storage", error))
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM storages WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete storage", error))?;

    Ok(result.rows_affected() > 0)
}
