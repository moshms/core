use mc_core::Result;
use serde::Serialize;
use sqlx::PgPool;

use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct AdminStats {
    pub users: i64,
    pub libraries: i64,
    pub tracks: i64,
    pub albums: i64,
    pub artists: i64,
    pub media_bytes: i64,
    pub jobs_pending: i64,
    pub jobs_running: i64,
    pub jobs_failed: i64,
}

pub async fn admin(pool: &PgPool) -> Result<AdminStats> {
    sqlx::query_as::<_, AdminStats>(
        "SELECT
             (SELECT count(*) FROM users) AS users,
             (SELECT count(*) FROM libraries) AS libraries,
             (SELECT count(*) FROM tracks) AS tracks,
             (SELECT count(*) FROM albums) AS albums,
             (SELECT count(*) FROM artists) AS artists,
             (SELECT COALESCE(sum(size), 0)::bigint
              FROM media_files WHERE scan_state = 'indexed') AS media_bytes,
             (SELECT count(*) FROM jobs WHERE status = 'pending') AS jobs_pending,
             (SELECT count(*) FROM jobs WHERE status = 'running') AS jobs_running,
             (SELECT count(*) FROM jobs WHERE status = 'failed') AS jobs_failed",
    )
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("load admin stats", error))
}
