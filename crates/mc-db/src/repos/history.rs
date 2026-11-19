use mc_core::Result;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::catalog::TRACK_COLUMNS;
use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct PlayHistoryEntry {
    pub played_at: chrono::DateTime<chrono::Utc>,
    pub id: Uuid,
    pub title: String,
    pub disc_no: i32,
    pub track_no: i32,
    pub duration_ms: Option<i64>,
    pub artist: Option<String>,
    pub album_id: Option<Uuid>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub album_artist_id: Option<Uuid>,
    pub album_year: Option<i32>,
    pub file_key: String,
    pub size: i64,
    pub format: Option<String>,
    pub library_id: Uuid,
    pub uploaded_by: Option<Uuid>,
}

pub async fn recent(
    pool: &PgPool,
    user_id: Uuid,
    visible: &[Uuid],
    limit: i64,
) -> Result<Vec<PlayHistoryEntry>> {
    let query = format!(
        "SELECT
             pe.played_at,
             {TRACK_COLUMNS}
         FROM play_events pe
         JOIN tracks t ON t.id = pe.track_id
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE pe.user_id = $1
           AND mf.library_id = ANY($2)
           AND mf.scan_state = 'indexed'
         ORDER BY pe.played_at DESC
         LIMIT $3"
    );

    sqlx::query_as::<_, PlayHistoryEntry>(&query)
        .bind(user_id)
        .bind(visible)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list play history", error))
}

pub async fn now_playing(
    pool: &PgPool,
    visible: &[Uuid],
    minutes: i64,
    limit: i64,
) -> Result<Vec<PlayHistoryEntry>> {
    let query = format!(
        "SELECT
             pe.played_at,
             {TRACK_COLUMNS}
         FROM play_events pe
         JOIN tracks t ON t.id = pe.track_id
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE pe.played_at > now() - make_interval(mins => $1::int)
           AND mf.library_id = ANY($2)
           AND mf.scan_state = 'indexed'
         ORDER BY pe.played_at DESC
         LIMIT $3"
    );

    sqlx::query_as::<_, PlayHistoryEntry>(&query)
        .bind(i32::try_from(minutes).unwrap_or(i32::MAX))
        .bind(visible)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list now playing", error))
}
