use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

pub async fn upsert(pool: &PgPool, name: &str) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO artists (id, name)
         VALUES ($1, $2)
         ON CONFLICT (lower(name)) DO UPDATE SET updated_at = now()
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("upsert artist", error))
}

pub async fn delete_empty(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query(
        "DELETE FROM artists ar
         WHERE NOT EXISTS (SELECT 1 FROM albums al WHERE al.album_artist_id = ar.id)
           AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = ar.id)",
    )
    .execute(pool)
    .await
    .map_err(|error| db_error("delete empty artists", error))?;

    Ok(result.rows_affected())
}

pub async fn set_artwork(pool: &PgPool, artist_id: Uuid, artwork_id: Option<Uuid>) -> Result<bool> {
    let result =
        sqlx::query("UPDATE artists SET artwork_id = $2, updated_at = now() WHERE id = $1")
            .bind(artist_id)
            .bind(artwork_id)
            .execute(pool)
            .await
            .map_err(|error| db_error("update artist artwork", error))?;

    Ok(result.rows_affected() > 0)
}
