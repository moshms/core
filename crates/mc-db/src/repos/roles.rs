use mc_core::Result;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::{db_error, db_error_unique};

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct RoleRecord {
    pub id: Uuid,
    pub name: String,
    pub color: Option<i32>,
    pub position: i32,
    pub permissions: i64,
    pub is_everyone: bool,
    pub member_count: i64,
}

const ROLE_COLUMNS: &str = "
    r.id,
    r.name,
    r.color,
    r.position,
    r.permissions,
    r.is_everyone,
    (SELECT count(*) FROM user_roles ur WHERE ur.role_id = r.id) AS member_count
";

pub async fn list(pool: &PgPool) -> Result<Vec<RoleRecord>> {
    let query = format!(
        "SELECT {ROLE_COLUMNS}
         FROM roles r
         ORDER BY r.position DESC, lower(r.name)"
    );

    sqlx::query_as::<_, RoleRecord>(&query)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list roles", error))
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<RoleRecord>> {
    let query = format!("SELECT {ROLE_COLUMNS} FROM roles r WHERE r.id = $1");

    sqlx::query_as::<_, RoleRecord>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|error| db_error("load role", error))
}

pub async fn everyone(pool: &PgPool) -> Result<Option<RoleRecord>> {
    let query = format!("SELECT {ROLE_COLUMNS} FROM roles r WHERE r.is_everyone");

    sqlx::query_as::<_, RoleRecord>(&query)
        .fetch_optional(pool)
        .await
        .map_err(|error| db_error("load everyone role", error))
}

pub async fn create(
    pool: &PgPool,
    name: &str,
    color: Option<i32>,
    permissions: i64,
) -> Result<RoleRecord> {
    let position = sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM roles WHERE NOT is_everyone",
    )
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("next role position", error))?;

    let query = "INSERT INTO roles (id, name, color, position, permissions)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, name, color, position, permissions, is_everyone, 0::bigint AS member_count"
        .to_string();

    sqlx::query_as::<_, RoleRecord>(&query)
        .bind(Uuid::now_v7())
        .bind(name)
        .bind(color)
        .bind(position)
        .bind(permissions)
        .fetch_one(pool)
        .await
        .map_err(|error| db_error_unique("create role", error))
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    name: &str,
    color: Option<i32>,
    permissions: i64,
) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE roles
         SET name = $2, color = $3, permissions = $4, updated_at = now()
         WHERE id = $1 AND NOT is_everyone",
    )
    .bind(id)
    .bind(name)
    .bind(color)
    .bind(permissions)
    .execute(pool)
    .await
    .map_err(|error| db_error_unique("update role", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn update_everyone(pool: &PgPool, permissions: i64) -> Result<()> {
    sqlx::query("UPDATE roles SET permissions = $1, updated_at = now() WHERE is_everyone")
        .bind(permissions)
        .execute(pool)
        .await
        .map_err(|error| db_error("update everyone role", error))?;

    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM roles WHERE id = $1 AND NOT is_everyone")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete role", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn reorder(pool: &PgPool, ids: &[Uuid]) -> Result<()> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| db_error("begin role reorder", error))?;

    let total = i32::try_from(ids.len()).unwrap_or(i32::MAX);

    for (index, id) in ids.iter().enumerate() {
        let position = total - i32::try_from(index).unwrap_or(i32::MAX);
        sqlx::query(
            "UPDATE roles SET position = $2, updated_at = now()
             WHERE id = $1 AND NOT is_everyone",
        )
        .bind(id)
        .bind(position.max(1))
        .execute(&mut *transaction)
        .await
        .map_err(|error| db_error("update role position", error))?;
    }

    sqlx::query("UPDATE roles SET position = 0 WHERE is_everyone")
        .execute(&mut *transaction)
        .await
        .map_err(|error| db_error("reset everyone position", error))?;

    transaction
        .commit()
        .await
        .map_err(|error| db_error("commit role reorder", error))?;

    Ok(())
}

pub async fn assign(pool: &PgPool, user_id: Uuid, role_id: Uuid) -> Result<()> {
    sqlx::query(
        "INSERT INTO user_roles (user_id, role_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(role_id)
    .execute(pool)
    .await
    .map_err(|error| db_error("assign role", error))?;

    Ok(())
}

pub async fn unassign(pool: &PgPool, user_id: Uuid, role_id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM user_roles WHERE user_id = $1 AND role_id = $2")
        .bind(user_id)
        .bind(role_id)
        .execute(pool)
        .await
        .map_err(|error| db_error("unassign role", error))?;

    Ok(result.rows_affected() > 0)
}

pub async fn for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<RoleRecord>> {
    let query = format!(
        "SELECT {ROLE_COLUMNS}
         FROM roles r
         JOIN user_roles ur ON ur.role_id = r.id
         WHERE ur.user_id = $1
         ORDER BY r.position DESC, lower(r.name)"
    );

    sqlx::query_as::<_, RoleRecord>(&query)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load user roles", error))
}

pub async fn ids_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Uuid>> {
    sqlx::query_scalar::<_, Uuid>("SELECT role_id FROM user_roles WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load user role ids", error))
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

pub async fn has_admin_role(pool: &PgPool, user_id: Uuid) -> Result<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
             SELECT 1
             FROM user_roles ur
             JOIN roles r ON r.id = ur.role_id
             WHERE ur.user_id = $1 AND (r.permissions & 1) <> 0
         )",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("check admin role", error))
}
