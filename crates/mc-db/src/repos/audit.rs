use mc_core::Result;
use serde::Serialize;
use serde_json::Value;
use sqlx::{Executor, PgPool, Postgres};
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct AuditRecord {
    pub id: Uuid,
    pub actor: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub changes: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn log<'e, E>(
    executor: E,
    actor_id: Option<Uuid>,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    changes: Value,
) -> Result<()>
where
    E: Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "INSERT INTO audit_log (id, actor_id, action, target_type, target_id, changes)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(actor_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(changes)
    .execute(executor)
    .await
    .map_err(|error| db_error("write audit log", error))?;

    Ok(())
}

pub async fn list(pool: &PgPool, limit: i64) -> Result<Vec<AuditRecord>> {
    sqlx::query_as::<_, AuditRecord>(
        "SELECT a.id, u.username AS actor, a.action, a.target_type, a.target_id,
                a.changes, a.created_at
         FROM audit_log a
         LEFT JOIN users u ON u.id = a.actor_id
         ORDER BY a.created_at DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list audit log", error))
}
