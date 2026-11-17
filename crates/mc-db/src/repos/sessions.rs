use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

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
