use mc_core::Result;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct OverwriteRecord {
    pub resource_type: String,
    pub resource_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub allow: i64,
    pub deny: i64,
}

pub async fn list_for_resource(
    pool: &PgPool,
    resource_type: &str,
    resource_id: Uuid,
) -> Result<Vec<OverwriteRecord>> {
    sqlx::query_as::<_, OverwriteRecord>(
        "SELECT resource_type, resource_id, target_type, target_id, allow, deny
         FROM permission_overwrites
         WHERE resource_type = $1 AND resource_id = $2
         ORDER BY target_type, target_id",
    )
    .bind(resource_type)
    .bind(resource_id)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list permission overwrites", error))
}

pub async fn list_for_user(
    pool: &PgPool,
    resource_type: &str,
    user_id: Uuid,
    role_ids: &[Uuid],
) -> Result<Vec<OverwriteRecord>> {
    sqlx::query_as::<_, OverwriteRecord>(
        "SELECT resource_type, resource_id, target_type, target_id, allow, deny
         FROM permission_overwrites
         WHERE resource_type = $1
           AND (
                (target_type = 'role' AND target_id = ANY($2))
                OR (target_type = 'user' AND target_id = $3)
           )",
    )
    .bind(resource_type)
    .bind(role_ids)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list user permission overwrites", error))
}

pub async fn upsert(
    pool: &PgPool,
    resource_type: &str,
    resource_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    allow: i64,
    deny: i64,
) -> Result<OverwriteRecord> {
    sqlx::query_as::<_, OverwriteRecord>(
        "INSERT INTO permission_overwrites
             (resource_type, resource_id, target_type, target_id, allow, deny)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (resource_type, resource_id, target_type, target_id)
         DO UPDATE SET allow = EXCLUDED.allow, deny = EXCLUDED.deny
         RETURNING resource_type, resource_id, target_type, target_id, allow, deny",
    )
    .bind(resource_type)
    .bind(resource_id)
    .bind(target_type)
    .bind(target_id)
    .bind(allow)
    .bind(deny)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("upsert permission overwrite", error))
}

pub async fn delete(
    pool: &PgPool,
    resource_type: &str,
    resource_id: Uuid,
    target_type: &str,
    target_id: Uuid,
) -> Result<bool> {
    let result = sqlx::query(
        "DELETE FROM permission_overwrites
         WHERE resource_type = $1 AND resource_id = $2
           AND target_type = $3 AND target_id = $4",
    )
    .bind(resource_type)
    .bind(resource_id)
    .bind(target_type)
    .bind(target_id)
    .execute(pool)
    .await
    .map_err(|error| db_error("delete permission overwrite", error))?;

    Ok(result.rows_affected() > 0)
}
