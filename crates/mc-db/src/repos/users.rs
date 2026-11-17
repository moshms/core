use mc_core::Result;
use serde::Serialize;
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use super::{db_error, db_error_unique};

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub is_active: bool,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct UserRecord {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub subsonic_secret: String,
    pub is_active: bool,
}

pub async fn count(pool: &PgPool) -> Result<i64> {
    sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(|error| db_error("count users", error))
}

pub async fn find_by_username(pool: &PgPool, username: &str) -> Result<Option<UserRecord>> {
    sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, password_hash, subsonic_secret, is_active
         FROM users
         WHERE lower(username) = lower($1)",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load user", error))
}

pub async fn create<'e, E>(
    executor: E,
    username: &str,
    password_hash: &str,
    subsonic_secret: &str,
) -> Result<UserRecord>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query_as::<_, UserRecord>(
        "INSERT INTO users (id, username, password_hash, subsonic_secret)
         VALUES ($1, $2, $3, $4)
         RETURNING id, username, password_hash, subsonic_secret, is_active",
    )
    .bind(Uuid::now_v7())
    .bind(username)
    .bind(password_hash)
    .bind(subsonic_secret)
    .fetch_one(executor)
    .await
    .map_err(|error| db_error_unique("create user", error))
}

pub async fn list(pool: &PgPool) -> Result<Vec<UserSummary>> {
    sqlx::query_as::<_, UserSummary>(
        "SELECT id, username, is_active
         FROM users
         ORDER BY lower(username)",
    )
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list users", error))
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<UserRecord>> {
    sqlx::query_as::<_, UserRecord>(
        "SELECT id, username, password_hash, subsonic_secret, is_active
         FROM users
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load user", error))
}

pub async fn update_active<'e, E>(executor: E, id: Uuid, is_active: bool) -> Result<bool>
where
    E: Executor<'e, Database = Postgres>,
{
    let result = sqlx::query("UPDATE users SET is_active = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(is_active)
        .execute(executor)
        .await
        .map_err(|error| db_error("update user activity", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn update_password<'e, E>(executor: E, id: Uuid, password_hash: &str) -> Result<bool>
where
    E: Executor<'e, Database = Postgres>,
{
    let result =
        sqlx::query("UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(password_hash)
            .execute(executor)
            .await
            .map_err(|error| db_error("update user password", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete<'e, E>(executor: E, id: Uuid) -> Result<bool>
where
    E: Executor<'e, Database = Postgres>,
{
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(executor)
        .await
        .map_err(|error| db_error("delete user", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn count_active_admins(pool: &PgPool) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(DISTINCT u.id)
         FROM users u
         JOIN user_roles ur ON ur.user_id = u.id
         JOIN roles r ON r.id = ur.role_id
         WHERE u.is_active AND (r.permissions & 1) <> 0",
    )
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("count active admins", error))
}

pub async fn update_subsonic_secret(
    pool: &PgPool,
    id: Uuid,
    subsonic_secret: &str,
) -> Result<bool> {
    let result =
        sqlx::query("UPDATE users SET subsonic_secret = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(subsonic_secret)
            .execute(pool)
            .await
            .map_err(|error| db_error("update subsonic secret", error))?;

    Ok(result.rows_affected() > 0)
}
