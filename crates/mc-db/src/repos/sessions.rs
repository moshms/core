use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct SessionRecord {
    pub id: Uuid,
    pub token_hash: String,
    pub client_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_seen_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct SessionUser {
    pub user_id: Uuid,
    pub username: String,
    pub role: String,
    pub subsonic_secret: String,
}

pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    ttl_hours: i64,
    client_name: Option<&str>,
) -> Result<Uuid> {
    let id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, client_name, expires_at)
         VALUES ($1, $2, $3, $4, now() + make_interval(hours => $5::int))",
    )
    .bind(id)
    .bind(user_id)
    .bind(token_hash)
    .bind(client_name)
    .bind(i32::try_from(ttl_hours).unwrap_or(i32::MAX))
    .execute(pool)
    .await
    .map_err(|error| db_error("create session", error))?;

    Ok(id)
}

pub async fn find_active(pool: &PgPool, token_hash: &str) -> Result<Option<SessionUser>> {
    sqlx::query_as::<_, SessionUser>(
        "SELECT u.id AS user_id, u.username, u.role, u.subsonic_secret
         FROM sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1 AND s.expires_at > now() AND u.is_active",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load session", error))
}

pub async fn touch(pool: &PgPool, token_hash: &str) -> Result<()> {
    sqlx::query(
        "UPDATE sessions SET last_seen_at = now()
         WHERE token_hash = $1
           AND last_seen_at < now() - interval '5 minutes'",
    )
    .bind(token_hash)
    .execute(pool)
    .await
    .map_err(|error| db_error("touch session", error))?;

    Ok(())
}

pub async fn delete(pool: &PgPool, token_hash: &str) -> Result<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete session", error))?;

    Ok(())
}

pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<SessionRecord>> {
    sqlx::query_as::<_, SessionRecord>(
        "SELECT id, token_hash, client_name, created_at, last_seen_at, expires_at
         FROM sessions
         WHERE user_id = $1 AND expires_at > now()
         ORDER BY last_seen_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list sessions", error))
}

pub async fn delete_for_user(pool: &PgPool, user_id: Uuid) -> Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete user sessions", error))?;

    Ok(result.rows_affected())
}

pub async fn delete_others(pool: &PgPool, user_id: Uuid, keep_token_hash: &str) -> Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND token_hash <> $2")
        .bind(user_id)
        .bind(keep_token_hash)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete other sessions", error))?;

    Ok(result.rows_affected())
}

pub async fn delete_by_id(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM sessions WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete session by id", error))?;

    Ok(result.rows_affected() > 0)
}
