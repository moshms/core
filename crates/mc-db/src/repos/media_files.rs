use std::collections::HashMap;

use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct MediaFileState {
    pub id: Uuid,
    pub key: String,
    pub size: i64,
    pub etag: String,
    pub mtime_ms: i64,
}

#[derive(Clone, Debug)]
pub struct NewMediaFile<'a> {
    pub library_id: Uuid,
    pub key: &'a str,
    pub size: i64,
    pub etag: &'a str,
    pub mtime_ms: i64,
    pub format: Option<&'a str>,
    pub bitrate: Option<i32>,
    pub sample_rate: Option<i32>,
    pub duration_ms: Option<i64>,
    pub content_hash: Option<&'a str>,
}

pub async fn load_indexed(
    pool: &PgPool,
    library_id: Uuid,
) -> Result<HashMap<String, MediaFileState>> {
    let rows = sqlx::query_as::<_, MediaFileState>(
        "SELECT id, key, size, etag, mtime_ms
         FROM media_files
         WHERE library_id = $1 AND scan_state = 'indexed'",
    )
    .bind(library_id)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("load media files", error))?;

    Ok(rows.into_iter().map(|row| (row.key.clone(), row)).collect())
}

pub async fn upsert(pool: &PgPool, file: &NewMediaFile<'_>) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO media_files
             (id, library_id, key, size, etag, mtime_ms, format, bitrate, sample_rate, duration_ms, content_hash, scan_state)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, 'indexed')
         ON CONFLICT (library_id, key) DO UPDATE SET
             size = EXCLUDED.size,
             etag = EXCLUDED.etag,
             mtime_ms = EXCLUDED.mtime_ms,
             content_hash = EXCLUDED.content_hash,
             format = EXCLUDED.format,
             bitrate = EXCLUDED.bitrate,
             sample_rate = EXCLUDED.sample_rate,
             duration_ms = EXCLUDED.duration_ms,
             scan_state = 'indexed',
             updated_at = now()
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(file.library_id)
    .bind(file.key)
    .bind(file.size)
    .bind(file.etag)
    .bind(file.mtime_ms)
    .bind(file.format)
    .bind(file.bitrate)
    .bind(file.sample_rate)
    .bind(file.duration_ms)
    .bind(file.content_hash)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("upsert media file", error))
}

pub async fn mark_duplicate(pool: &PgPool, library_id: Uuid, key: &str) -> Result<()> {
    sqlx::query(
        "UPDATE media_files
         SET scan_state = 'duplicate', updated_at = now()
         WHERE library_id = $1 AND key = $2",
    )
    .bind(library_id)
    .bind(key)
    .execute(pool)
    .await
    .map_err(|error| db_error("mark media file duplicate", error))?;

    Ok(())
}

pub async fn find_track_by_content_hash(
    pool: &PgPool,
    content_hash: &str,
    exclude_file_id: Uuid,
) -> Result<Option<Uuid>> {
    sqlx::query_scalar::<_, Uuid>(
        "SELECT t.id
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         WHERE mf.content_hash = $1 AND mf.id <> $2
         LIMIT 1",
    )
    .bind(content_hash)
    .bind(exclude_file_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("find duplicate track", error))
}

pub async fn mark_failed(pool: &PgPool, library_id: Uuid, key: &str) -> Result<()> {
    sqlx::query(
        "UPDATE media_files
         SET scan_state = 'failed', updated_at = now()
         WHERE library_id = $1 AND key = $2",
    )
    .bind(library_id)
    .bind(key)
    .execute(pool)
    .await
    .map_err(|error| db_error("mark media file failed", error))?;

    Ok(())
}

pub async fn mark_missing(pool: &PgPool, library_id: Uuid, keys: &[String]) -> Result<u64> {
    if keys.is_empty() {
        return Ok(0);
    }

    let result = sqlx::query(
        "UPDATE media_files
         SET scan_state = 'missing', updated_at = now()
         WHERE library_id = $1 AND key = ANY($2) AND scan_state = 'indexed'",
    )
    .bind(library_id)
    .bind(keys)
    .execute(pool)
    .await
    .map_err(|error| db_error("mark missing media files", error))?;

    Ok(result.rows_affected())
}

pub async fn delete_by_key(pool: &PgPool, library_id: Uuid, key: &str) -> Result<u64> {
    let result = sqlx::query("DELETE FROM media_files WHERE library_id = $1 AND key = $2")
        .bind(library_id)
        .bind(key)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete media file by key", error))?;

    Ok(result.rows_affected())
}
