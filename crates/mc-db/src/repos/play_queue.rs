use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug)]
pub struct PlayQueueRecord {
    pub track_ids: Vec<Uuid>,
    pub current_index: i32,
    pub position_ms: i64,
}

#[derive(sqlx::FromRow)]
struct PlayQueueRow {
    track_ids: serde_json::Value,
    current_index: i32,
    position_ms: i64,
}

pub async fn get(pool: &PgPool, user_id: Uuid) -> Result<Option<PlayQueueRecord>> {
    let row = sqlx::query_as::<_, PlayQueueRow>(
        "SELECT track_ids, current_index, position_ms
         FROM play_queues
         WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load play queue", error))?;

    let Some(row) = row else {
        return Ok(None);
    };

    let track_ids = row
        .track_ids
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .filter_map(|value| Uuid::parse_str(value).ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(Some(PlayQueueRecord {
        track_ids,
        current_index: row.current_index,
        position_ms: row.position_ms,
    }))
}

pub async fn save(
    pool: &PgPool,
    user_id: Uuid,
    track_ids: &[Uuid],
    current_index: i32,
    position_ms: i64,
) -> Result<()> {
    let track_ids = serde_json::Value::Array(
        track_ids
            .iter()
            .map(|id| serde_json::Value::String(id.to_string()))
            .collect(),
    );

    sqlx::query(
        "INSERT INTO play_queues (user_id, track_ids, current_index, position_ms, changed_at)
         VALUES ($1, $2, $3, $4, now())
         ON CONFLICT (user_id) DO UPDATE SET
             track_ids = EXCLUDED.track_ids,
             current_index = EXCLUDED.current_index,
             position_ms = EXCLUDED.position_ms,
             changed_at = now()",
    )
    .bind(user_id)
    .bind(track_ids)
    .bind(current_index)
    .bind(position_ms)
    .execute(pool)
    .await
    .map_err(|error| db_error("save play queue", error))?;

    Ok(())
}
