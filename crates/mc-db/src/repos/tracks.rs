use mc_core::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug)]
pub struct NewTrack<'a> {
    pub title: &'a str,
    pub album_id: Uuid,
    pub disc_no: i32,
    pub track_no: i32,
    pub year: i32,
    pub duration_ms: Option<i64>,
    pub lyrics: Option<&'a str>,
    pub primary_file_id: Uuid,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackArtist<'a> {
    pub artist_id: Uuid,
    pub role: &'a str,
    pub position: i32,
}

pub async fn upsert(pool: &PgPool, track: &NewTrack<'_>) -> Result<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO tracks
             (id, title, album_id, disc_no, track_no, year, duration_ms, lyrics, primary_file_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (primary_file_id) DO UPDATE SET
             title = EXCLUDED.title,
             album_id = EXCLUDED.album_id,
             disc_no = EXCLUDED.disc_no,
             track_no = EXCLUDED.track_no,
             year = EXCLUDED.year,
             duration_ms = EXCLUDED.duration_ms,
             lyrics = EXCLUDED.lyrics,
             updated_at = now()
         RETURNING id",
    )
    .bind(Uuid::now_v7())
    .bind(track.title)
    .bind(track.album_id)
    .bind(track.disc_no)
    .bind(track.track_no)
    .bind(track.year)
    .bind(track.duration_ms)
    .bind(track.lyrics)
    .bind(track.primary_file_id)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("upsert track", error))
}

pub async fn set_artists(pool: &PgPool, track_id: Uuid, artists: &[TrackArtist<'_>]) -> Result<()> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| db_error("begin track artists transaction", error))?;

    sqlx::query("DELETE FROM track_artists WHERE track_id = $1")
        .bind(track_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| db_error("clear track artists", error))?;

    for artist in artists {
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role, position)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (track_id, artist_id, role) DO UPDATE SET position = EXCLUDED.position",
        )
        .bind(track_id)
        .bind(artist.artist_id)
        .bind(artist.role)
        .bind(artist.position)
        .execute(&mut *transaction)
        .await
        .map_err(|error| db_error("insert track artist", error))?;
    }

    transaction
        .commit()
        .await
        .map_err(|error| db_error("commit track artists transaction", error))
}
