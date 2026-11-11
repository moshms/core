use mc_core::{CoreError, Result};
use mc_db::PgPool;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct JobRecord {
    pub id: Uuid,
    pub kind: String,
    pub payload: Value,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
}

pub async fn enqueue(pool: &PgPool, kind: &str, payload: Value) -> Result<Uuid> {
    let id = Uuid::now_v7();

    sqlx::query(
        "INSERT INTO jobs (id, kind, payload)
         VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(kind)
    .bind(payload)
    .execute(pool)
    .await
    .map_err(|error| CoreError::Internal(format!("enqueue job: {error}")))?;

    Ok(id)
}

pub async fn claim_one(
    pool: &PgPool,
    worker_id: &str,
    kinds: &[&str],
) -> Result<Option<JobRecord>> {
    let kinds = kinds
        .iter()
        .map(|kind| (*kind).to_owned())
        .collect::<Vec<_>>();
    sqlx::query_as::<_, JobRecord>(
        "WITH next AS (
             SELECT id
             FROM jobs
             WHERE status = 'pending' AND run_at <= now() AND kind = ANY($2)
             ORDER BY run_at
             FOR UPDATE SKIP LOCKED
             LIMIT 1
         )
         UPDATE jobs
         SET status = 'running', locked_by = $1, attempts = attempts + 1, updated_at = now()
         FROM next
         WHERE jobs.id = next.id
         RETURNING jobs.id, jobs.kind, jobs.payload, jobs.status, jobs.attempts, jobs.max_attempts, jobs.last_error",
    )
    .bind(worker_id)
    .bind(&kinds)
    .fetch_optional(pool)
    .await
    .map_err(|error| CoreError::Internal(format!("claim job: {error}")))
}

pub async fn complete(pool: &PgPool, id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE jobs
         SET status = 'done', locked_by = NULL, last_error = NULL, updated_at = now()
         WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| CoreError::Internal(format!("complete job: {error}")))?;

    Ok(())
}

pub async fn fail(pool: &PgPool, job: &JobRecord, error: &str) -> Result<()> {
    let retry = job.attempts < job.max_attempts;
    let status = if retry { "pending" } else { "needs_review" };

    sqlx::query(
        "UPDATE jobs
         SET status = $2,
             locked_by = NULL,
             last_error = $3,
             run_at = now() + make_interval(secs => $4),
             updated_at = now()
         WHERE id = $1",
    )
    .bind(job.id)
    .bind(status)
    .bind(error)
    .bind(if retry { 30 } else { 0 })
    .execute(pool)
    .await
    .map_err(|db_error| CoreError::Internal(format!("fail job: {db_error}")))?;

    Ok(())
}

pub async fn retry(pool: &PgPool, id: Uuid) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE jobs
         SET status = 'pending', attempts = 0, run_at = now(), locked_by = NULL,
             last_error = NULL, updated_at = now()
         WHERE id = $1 AND status IN ('needs_review', 'failed', 'done')",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| CoreError::Internal(format!("retry job: {error}")))?;

    Ok(result.rows_affected() > 0)
}

pub async fn list(pool: &PgPool, limit: i64) -> Result<Vec<JobRecord>> {
    sqlx::query_as::<_, JobRecord>(
        "SELECT id, kind, payload, status, attempts, max_attempts, last_error
         FROM jobs
         ORDER BY created_at DESC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|error| CoreError::Internal(format!("list jobs: {error}")))
}

pub async fn heartbeat(pool: &PgPool, worker_id: &str, version: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO worker_heartbeats (worker_id, last_seen_at, version)
         VALUES ($1, now(), $2)
         ON CONFLICT (worker_id) DO UPDATE SET
             last_seen_at = now(),
             version = EXCLUDED.version",
    )
    .bind(worker_id)
    .bind(version)
    .execute(pool)
    .await
    .map_err(|error| CoreError::Internal(format!("worker heartbeat: {error}")))?;

    Ok(())
}

pub async fn is_worker_online(pool: &PgPool, max_age_seconds: i64) -> Result<bool> {
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(
             SELECT 1 FROM worker_heartbeats
             WHERE last_seen_at > now() - make_interval(secs => $1)
         )",
    )
    .bind(i32::try_from(max_age_seconds).unwrap_or(i32::MAX))
    .fetch_one(pool)
    .await
    .map_err(|error| CoreError::Internal(format!("worker heartbeat check: {error}")))
}
