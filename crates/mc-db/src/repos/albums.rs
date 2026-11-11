use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

pub async fn upsert(
    pool: &PgPool,
    title: &str,
    album_artist_id: Uuid,
    year: i32,
    artwork_id: Option<Uuid>,
) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO albums (id, title, album_artist_id, year, artwork_id)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (album_artist_id, lower(title), year) DO UPDATE SET
             title = EXCLUDED.title,
             artwork_id = COALESCE(EXCLUDED.artwork_id, albums.artwork_id),
             updated_at = now()
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(title)
    .bind(album_artist_id)
    .bind(year)
    .bind(artwork_id)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("upsert album", error))
}

pub async fn delete_empty(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query(
        "DELETE FROM albums al
         WHERE NOT EXISTS (SELECT 1 FROM tracks t WHERE t.album_id = al.id)",
    )
    .execute(pool)
    .await
    .map_err(|error| db_error("delete empty albums", error))?;

    Ok(result.rows_affected())
}
