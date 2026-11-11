use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug)]
pub struct NewArtwork<'a> {
    pub hash: &'a str,
    pub source: &'a str,
    pub mime: &'a str,
    pub data: &'a [u8],
}

pub async fn upsert(pool: &PgPool, artwork: &NewArtwork<'_>) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO artworks (id, hash, source, mime, size_bytes, data)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (hash) DO UPDATE SET hash = EXCLUDED.hash
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(artwork.hash)
    .bind(artwork.source)
    .bind(artwork.mime)
    .bind(i64::try_from(artwork.data.len()).unwrap_or(i64::MAX))
    .bind(artwork.data)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("upsert artwork", error))
}
