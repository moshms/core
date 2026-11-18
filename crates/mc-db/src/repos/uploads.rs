use mc_core::Result;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct UploadRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub library_id: Uuid,
    pub filename: String,
    pub size: i64,
    pub offset: i64,
    pub staging_key: String,
    pub status: String,
    pub error: Option<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    library_id: Uuid,
    filename: &str,
    size: i64,
    staging_key: &str,
) -> Result<UploadRecord> {
    sqlx::query_as::<_, UploadRecord>(
        "INSERT INTO uploads (id, user_id, library_id, filename, size, staging_key)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, user_id, library_id, filename, size, upload_offset AS offset, staging_key, status, error, updated_at",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(library_id)
    .bind(filename)
    .bind(size)
    .bind(staging_key)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("create upload", error))
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<UploadRecord>> {
    sqlx::query_as::<_, UploadRecord>(
        "SELECT id, user_id, library_id, filename, size, upload_offset AS offset, staging_key, status, error, updated_at
         FROM uploads
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load upload", error))
}

pub async fn update_offset(pool: &PgPool, id: Uuid, offset: i64) -> Result<()> {
    sqlx::query(
        "UPDATE uploads
         SET upload_offset = $2, updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(offset)
    .execute(pool)
    .await
    .map_err(|error| db_error("update upload offset", error))?;

    Ok(())
}

pub async fn finish(pool: &PgPool, id: Uuid, status: &str, error: Option<&str>) -> Result<()> {
    sqlx::query(
        "UPDATE uploads
         SET status = $2, error = $3, updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(error)
    .execute(pool)
    .await
    .map_err(|error| db_error("finish upload", error))?;

    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM uploads WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete upload", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<UploadRecord>> {
    sqlx::query_as::<_, UploadRecord>(
        "SELECT id, user_id, library_id, filename, size, upload_offset AS offset, staging_key, status, error, updated_at
         FROM uploads
         WHERE user_id = $1
         ORDER BY created_at DESC
         LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list uploads", error))
}

pub async fn count_active_for_user(pool: &PgPool, user_id: Uuid) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM uploads WHERE user_id = $1 AND status <> 'complete'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("count active uploads", error))
}

pub async fn stale(pool: &PgPool, hours: i64) -> Result<Vec<UploadRecord>> {
    sqlx::query_as::<_, UploadRecord>(
        "SELECT id, user_id, library_id, filename, size, upload_offset AS offset, staging_key, status, error, updated_at
         FROM uploads
         WHERE status <> 'complete'
           AND updated_at < now() - make_interval(hours => $1::int)
         ORDER BY updated_at
         LIMIT 1000",
    )
    .bind(i32::try_from(hours).unwrap_or(i32::MAX))
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list stale uploads", error))
}
