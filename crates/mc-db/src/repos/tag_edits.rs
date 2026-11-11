use mc_core::Result;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct TagEditRecord {
    pub id: Uuid,
    pub user_id: Uuid,
    pub track_ids: Vec<Uuid>,
    pub changes: Value,
    pub artwork_mime: Option<String>,
    #[serde(skip_serializing)]
    pub artwork_data: Option<Vec<u8>>,
    pub status: String,
    pub error: Option<String>,
}

pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    track_ids: &[Uuid],
    changes: &Value,
    artwork_mime: Option<&str>,
    artwork_data: Option<&[u8]>,
) -> Result<TagEditRecord> {
    sqlx::query_as::<_, TagEditRecord>(
        "INSERT INTO tag_edits (id, user_id, track_ids, changes, artwork_mime, artwork_data)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, user_id, track_ids, changes, artwork_mime, artwork_data, status, error",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(track_ids)
    .bind(changes)
    .bind(artwork_mime)
    .bind(artwork_data)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("create tag edit", error))
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<TagEditRecord>> {
    sqlx::query_as::<_, TagEditRecord>(
        "SELECT id, user_id, track_ids, changes, artwork_mime, artwork_data, status, error
         FROM tag_edits
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load tag edit", error))
}

pub async fn list(pool: &PgPool, limit: i64) -> Result<Vec<TagEditRecord>> {
    sqlx::query_as::<_, TagEditRecord>(
        "SELECT id, user_id, track_ids, changes, artwork_mime, artwork_data, status, error
         FROM tag_edits
         ORDER BY created_at DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list tag edits", error))
}

pub async fn set_status(pool: &PgPool, id: Uuid, status: &str) -> Result<()> {
    sqlx::query("UPDATE tag_edits SET status = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(pool)
        .await
        .map_err(|error| db_error("update tag edit status", error))?;

    Ok(())
}

pub async fn finish(pool: &PgPool, id: Uuid, status: &str, error: Option<&str>) -> Result<()> {
    sqlx::query(
        "UPDATE tag_edits
         SET status = $2, error = $3, updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(error)
    .execute(pool)
    .await
    .map_err(|error| db_error("finish tag edit", error))?;

    Ok(())
}
