use mc_core::Result;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct ApiTokenRecord {
    pub id: Uuid,
    pub name: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ApiTokenUser {
    pub user_id: Uuid,
    pub username: String,
    pub subsonic_secret: String,
}

pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    name: &str,
    token_hash: &str,
    expires_in_days: Option<i64>,
) -> Result<Uuid> {
    let id = Uuid::now_v7();
    let expires_in_days = expires_in_days.map(|days| i32::try_from(days).unwrap_or(i32::MAX));

    sqlx::query(
        "INSERT INTO api_tokens (id, user_id, name, token_hash, expires_at)
         VALUES ($1, $2, $3, $4, now() + make_interval(days => $5::int))",
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(token_hash)
    .bind(expires_in_days)
    .execute(pool)
    .await
    .map_err(|error| db_error("create api token", error))?;

    Ok(id)
}

pub async fn find_user(pool: &PgPool, token_hash: &str) -> Result<Option<ApiTokenUser>> {
    sqlx::query_as::<_, ApiTokenUser>(
        "SELECT u.id AS user_id, u.username, u.subsonic_secret
         FROM api_tokens t
         JOIN users u ON u.id = t.user_id
         WHERE t.token_hash = $1
           AND (t.expires_at IS NULL OR t.expires_at > now())
           AND u.is_active",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load api token", error))
}

pub async fn touch(pool: &PgPool, token_hash: &str) -> Result<()> {
    sqlx::query(
        "UPDATE api_tokens SET last_used_at = now()
         WHERE token_hash = $1
           AND (last_used_at IS NULL OR last_used_at < now() - interval '5 minutes')",
    )
    .bind(token_hash)
    .execute(pool)
    .await
    .map_err(|error| db_error("touch api token", error))?;

    Ok(())
}

pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<ApiTokenRecord>> {
    sqlx::query_as::<_, ApiTokenRecord>(
        "SELECT id, name, expires_at, created_at, last_used_at
         FROM api_tokens WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list api tokens", error))
}

pub async fn delete(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM api_tokens WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete api token", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_for_user(pool: &PgPool, user_id: Uuid) -> Result<u64> {
    let result = sqlx::query("DELETE FROM api_tokens WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete user api tokens", error))?;

    Ok(result.rows_affected())
}
