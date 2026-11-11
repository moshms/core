use std::collections::HashMap;

use mc_core::Result;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct ArtistSummary {
    pub id: Uuid,
    pub name: String,
    pub album_count: i64,
    pub track_count: i64,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct AlbumSummary {
    pub id: Uuid,
    pub title: String,
    pub year: i32,
    pub album_artist_id: Uuid,
    pub album_artist: String,
    pub artwork_id: Option<Uuid>,
    pub track_count: i64,
    pub duration_ms: i64,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct TrackSummary {
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
}

#[derive(Clone, Debug, Serialize)]
pub struct AlbumDetail {
    #[serde(flatten)]
    pub album: AlbumSummary,
    pub tracks: Vec<TrackSummary>,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct TrackDetail {
    pub id: Uuid,
    pub title: String,
    pub album_id: Option<Uuid>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub album_artist_id: Option<Uuid>,
    pub artist: Option<String>,
    pub disc_no: i32,
    pub track_no: i32,
    pub year: i32,
    pub duration_ms: Option<i64>,
    pub file_key: String,
    pub size: i64,
    pub library_id: Uuid,
    pub format: Option<String>,
    pub bitrate: Option<i32>,
    pub sample_rate: Option<i32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtistDetail {
    pub id: Uuid,
    pub name: String,
    pub albums: Vec<AlbumSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResults {
    pub artists: Vec<ArtistSummary>,
    pub albums: Vec<AlbumSummary>,
    pub tracks: Vec<TrackSummary>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ArtworkRecord {
    pub mime: String,
    pub data: Vec<u8>,
}

const ARTIST_COLUMNS: &str = "
    ar.id,
    ar.name,
    (SELECT count(*) FROM albums al WHERE al.album_artist_id = ar.id) AS album_count,
    (SELECT count(*) FROM track_artists ta WHERE ta.artist_id = ar.id) AS track_count
";

const ALBUM_COLUMNS: &str = "
    al.id,
    al.title,
    al.year,
    al.album_artist_id,
    ar.name AS album_artist,
    al.artwork_id,
    (SELECT count(*) FROM tracks t WHERE t.album_id = al.id) AS track_count,
    (SELECT COALESCE(sum(t.duration_ms), 0)::bigint FROM tracks t WHERE t.album_id = al.id) AS duration_ms
";

const TRACK_COLUMNS: &str = "
    t.id,
    t.title,
    t.disc_no,
    t.track_no,
    t.duration_ms,
    mf.key AS file_key,
    mf.size,
    mf.format,
    al.id AS album_id,
    al.title AS album,
    ar.name AS album_artist,
    ar.id AS album_artist_id,
    al.year AS album_year,
    (SELECT string_agg(a.name, ', ' ORDER BY ta.position)
     FROM track_artists ta
     JOIN artists a ON a.id = ta.artist_id
     WHERE ta.track_id = t.id AND ta.role = 'main') AS artist
";

pub async fn list_artists(pool: &PgPool) -> Result<Vec<ArtistSummary>> {
    let query = format!("SELECT {ARTIST_COLUMNS} FROM artists ar ORDER BY lower(ar.name)");

    sqlx::query_as::<_, ArtistSummary>(&query)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list artists", error))
}

pub async fn list_albums(pool: &PgPool, artist_id: Option<Uuid>) -> Result<Vec<AlbumSummary>> {
    let query = format!(
        "SELECT {ALBUM_COLUMNS}
         FROM albums al
         JOIN artists ar ON ar.id = al.album_artist_id
         WHERE ($1::uuid IS NULL OR al.album_artist_id = $1)
         ORDER BY lower(ar.name), al.year, lower(al.title)"
    );

    sqlx::query_as::<_, AlbumSummary>(&query)
        .bind(artist_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list albums", error))
}

pub async fn get_artist(pool: &PgPool, artist_id: Uuid) -> Result<Option<ArtistDetail>> {
    let artist = sqlx::query_as::<_, (Uuid, String)>("SELECT id, name FROM artists WHERE id = $1")
        .bind(artist_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| db_error("load artist", error))?;

    let Some((id, name)) = artist else {
        return Ok(None);
    };

    let albums = list_albums(pool, Some(artist_id)).await?;
    Ok(Some(ArtistDetail { id, name, albums }))
}

pub async fn get_album(pool: &PgPool, album_id: Uuid) -> Result<Option<AlbumDetail>> {
    let album_query = format!(
        "SELECT {ALBUM_COLUMNS}
         FROM albums al
         JOIN artists ar ON ar.id = al.album_artist_id
         WHERE al.id = $1"
    );

    let Some(album) = sqlx::query_as::<_, AlbumSummary>(&album_query)
        .bind(album_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| db_error("load album", error))?
    else {
        return Ok(None);
    };

    let track_query = format!(
        "SELECT {TRACK_COLUMNS}
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE t.album_id = $1
         ORDER BY t.disc_no, t.track_no, lower(t.title)"
    );

    let tracks = sqlx::query_as::<_, TrackSummary>(&track_query)
        .bind(album_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load album tracks", error))?;

    Ok(Some(AlbumDetail { album, tracks }))
}

pub async fn get_track(pool: &PgPool, track_id: Uuid) -> Result<Option<TrackDetail>> {
    sqlx::query_as::<_, TrackDetail>(
        "SELECT
             t.id,
             t.title,
             t.album_id,
             al.title AS album,
             ar.name AS album_artist,
             al.album_artist_id,
             (SELECT string_agg(a.name, ', ' ORDER BY ta.position)
              FROM track_artists ta
              JOIN artists a ON a.id = ta.artist_id
              WHERE ta.track_id = t.id AND ta.role = 'main') AS artist,
             t.disc_no,
             t.track_no,
             t.year,
             t.duration_ms,
             mf.key AS file_key,
             mf.size,
             mf.library_id,
             mf.format,
             mf.bitrate,
             mf.sample_rate
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE t.id = $1",
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load track", error))
}

pub async fn get_tracks_by_ids(pool: &PgPool, ids: &[Uuid]) -> Result<Vec<TrackSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let query = format!(
        "SELECT {TRACK_COLUMNS}
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE t.id = ANY($1)"
    );

    let rows = sqlx::query_as::<_, TrackSummary>(&query)
        .bind(ids)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load tracks", error))?;

    let by_id = rows
        .into_iter()
        .map(|track| (track.id, track))
        .collect::<HashMap<_, _>>();

    Ok(ids.iter().filter_map(|id| by_id.get(id).cloned()).collect())
}

pub async fn search(pool: &PgPool, query: &str, limit: i64) -> Result<SearchResults> {
    let pattern = format!("%{query}%");

    let artist_query = format!(
        "SELECT {ARTIST_COLUMNS}
         FROM artists ar
         WHERE ar.name ILIKE $1
         ORDER BY lower(ar.name)
         LIMIT $2"
    );
    let artists = sqlx::query_as::<_, ArtistSummary>(&artist_query)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("search artists", error))?;

    let album_query = format!(
        "SELECT {ALBUM_COLUMNS}
         FROM albums al
         JOIN artists ar ON ar.id = al.album_artist_id
         WHERE al.title ILIKE $1
         ORDER BY lower(al.title)
         LIMIT $2"
    );
    let albums = sqlx::query_as::<_, AlbumSummary>(&album_query)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("search albums", error))?;

    let track_query = format!(
        "SELECT {TRACK_COLUMNS}
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE t.title ILIKE $1
         ORDER BY lower(t.title)
         LIMIT $2"
    );
    let tracks = sqlx::query_as::<_, TrackSummary>(&track_query)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("search tracks", error))?;

    Ok(SearchResults {
        artists,
        albums,
        tracks,
    })
}

pub async fn get_album_artwork(pool: &PgPool, album_id: Uuid) -> Result<Option<ArtworkRecord>> {
    sqlx::query_as::<_, ArtworkRecord>(
        "SELECT aw.mime, aw.data
         FROM albums al
         JOIN artworks aw ON aw.id = al.artwork_id
         WHERE al.id = $1",
    )
    .bind(album_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load album artwork", error))
}

pub async fn starred_tracks(pool: &PgPool, user_id: Uuid) -> Result<Vec<TrackSummary>> {
    let query = format!(
        "SELECT {TRACK_COLUMNS}
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         JOIN annotations an ON an.item_type = 'track' AND an.item_id = t.id
         WHERE an.user_id = $1 AND an.starred_at IS NOT NULL
         ORDER BY an.starred_at DESC"
    );

    sqlx::query_as::<_, TrackSummary>(&query)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load starred tracks", error))
}

pub async fn starred_albums(pool: &PgPool, user_id: Uuid) -> Result<Vec<AlbumSummary>> {
    let query = format!(
        "SELECT {ALBUM_COLUMNS}
         FROM albums al
         JOIN artists ar ON ar.id = al.album_artist_id
         JOIN annotations an ON an.item_type = 'album' AND an.item_id = al.id
         WHERE an.user_id = $1 AND an.starred_at IS NOT NULL
         ORDER BY an.starred_at DESC"
    );

    sqlx::query_as::<_, AlbumSummary>(&query)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load starred albums", error))
}

pub async fn starred_artists(pool: &PgPool, user_id: Uuid) -> Result<Vec<ArtistSummary>> {
    let query = format!(
        "SELECT {ARTIST_COLUMNS}
         FROM artists ar
         JOIN annotations an ON an.item_type = 'artist' AND an.item_id = ar.id
         WHERE an.user_id = $1 AND an.starred_at IS NOT NULL
         ORDER BY an.starred_at DESC"
    );

    sqlx::query_as::<_, ArtistSummary>(&query)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load starred artists", error))
}
