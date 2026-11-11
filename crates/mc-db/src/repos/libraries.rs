use mc_core::{CoreError, Result};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct LibraryRecord {
    pub id: Uuid,
    pub name: String,
    pub storage_id: Uuid,
    pub root_prefix: String,
    pub is_import_target: bool,
}

pub async fn get_by_name(pool: &PgPool, name: &str) -> Result<Option<LibraryRecord>> {
    sqlx::query_as::<_, LibraryRecord>(
        "SELECT id, name, storage_id, root_prefix, is_import_target
         FROM libraries
         WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load library", error))
}

pub async fn list(pool: &PgPool) -> Result<Vec<LibraryRecord>> {
    sqlx::query_as::<_, LibraryRecord>(
        "SELECT id, name, storage_id, root_prefix, is_import_target
         FROM libraries
         ORDER BY lower(name)",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list libraries", error))
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<LibraryRecord>> {
    sqlx::query_as::<_, LibraryRecord>(
        "SELECT id, name, storage_id, root_prefix, is_import_target
         FROM libraries
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load library", error))
}

pub async fn create(
    pool: &PgPool,
    name: &str,
    storage_id: Uuid,
    root_prefix: &str,
    is_import_target: bool,
) -> Result<LibraryRecord> {
    sqlx::query_as::<_, LibraryRecord>(
        "INSERT INTO libraries (id, name, storage_id, root_prefix, is_import_target)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, name, storage_id, root_prefix, is_import_target",
    )
    .bind(Uuid::now_v7())
    .bind(name)
    .bind(storage_id)
    .bind(root_prefix)
    .bind(is_import_target)
    .fetch_one(pool)
    .await
    .map_err(|error| super::db_error_unique("create library", error))
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM libraries WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete library", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn count_by_storage(pool: &PgPool, storage_id: Uuid) -> Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM libraries WHERE storage_id = $1")
        .bind(storage_id)
        .fetch_one(pool)
        .await
        .map_err(|error| db_error("count libraries by storage", error))
}

pub async fn ensure_default(pool: &PgPool, name: &str, root: &str) -> Result<LibraryRecord> {
    if let Some(library) = get_by_name(pool, name).await? {
        return Ok(library);
    }

    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| db_error("begin library transaction", error))?;

    let storage_id = Uuid::now_v7();
    let config = serde_json::json!({ "root": root });

    sqlx::query(
        "INSERT INTO storages (id, name, kind, config, access, status)
         VALUES ($1, $2, 'local', $3, 'rw', 'online')",
    )
    .bind(storage_id)
    .bind("local")
    .bind(config)
    .execute(&mut *transaction)
    .await
    .map_err(|error| db_error("create local storage", error))?;

    let inserted = sqlx::query_as::<_, LibraryRecord>(
        "INSERT INTO libraries (id, name, storage_id, root_prefix)
         VALUES ($1, $2, $3, '')
         ON CONFLICT (name) DO NOTHING
         RETURNING id, name, storage_id, root_prefix, is_import_target",
    )
    .bind(Uuid::now_v7())
    .bind(name)
    .bind(storage_id)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(|error| db_error("create library", error))?;

    if let Some(library) = inserted {
        transaction
            .commit()
            .await
            .map_err(|error| db_error("commit library transaction", error))?;
        return Ok(library);
    }

    transaction
        .rollback()
        .await
        .map_err(|error| db_error("rollback library transaction", error))?;

    get_by_name(pool, name)
        .await?
        .ok_or_else(|| CoreError::Internal(format!("library {name:?} disappeared during creation")))
}
