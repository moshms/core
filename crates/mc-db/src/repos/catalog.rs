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
    pub library_id: Uuid,
    pub uploaded_by: Option<Uuid>,
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
    pub uploaded_by: Option<Uuid>,
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
    pub etag: String,
}

const TRACK_COLUMNS: &str = "
    t.id,
    t.title,
    t.disc_no,
    t.track_no,
    t.duration_ms,
    mf.key AS file_key,
    mf.size,
    mf.format,
    mf.library_id,
    mf.uploaded_by,
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

const VISIBLE_TRACK: &str = "mf.library_id = ANY($1) AND mf.scan_state = 'indexed'";

pub async fn list_artists(pool: &PgPool, visible: &[Uuid]) -> Result<Vec<ArtistSummary>> {
    list_artists_page(pool, visible, None, None, 0).await
}

pub async fn list_artists_page(
    pool: &PgPool,
    visible: &[Uuid],
    library_id: Option<Uuid>,
    limit: Option<i64>,
    offset: i64,
) -> Result<Vec<ArtistSummary>> {
    let query = "SELECT
             ar.id,
             ar.name,
             (SELECT count(DISTINCT al.id)
              FROM albums al
              JOIN tracks t ON t.album_id = al.id
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE al.album_artist_id = ar.id
                AND mf.library_id = ANY($1)
                AND ($2::uuid IS NULL OR mf.library_id = $2)
                AND mf.scan_state = 'indexed') AS album_count,
             (SELECT count(*)
              FROM track_artists ta
              JOIN tracks t ON t.id = ta.track_id
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE ta.artist_id = ar.id
                AND mf.library_id = ANY($1)
                AND ($2::uuid IS NULL OR mf.library_id = $2)
                AND mf.scan_state = 'indexed') AS track_count
         FROM artists ar
         WHERE EXISTS (
             SELECT 1
             FROM track_artists ta
             JOIN tracks t ON t.id = ta.track_id
             JOIN media_files mf ON mf.id = t.primary_file_id
             WHERE ta.artist_id = ar.id
               AND mf.library_id = ANY($1)
               AND ($2::uuid IS NULL OR mf.library_id = $2)
               AND mf.scan_state = 'indexed'
         )
         ORDER BY lower(ar.name)
         LIMIT $3 OFFSET $4";

    sqlx::query_as::<_, ArtistSummary>(query)
        .bind(visible)
        .bind(library_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list artists", error))
}

pub async fn count_artists(
    pool: &PgPool,
    visible: &[Uuid],
    library_id: Option<Uuid>,
) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*)
         FROM artists ar
         WHERE EXISTS (
             SELECT 1
             FROM track_artists ta
             JOIN tracks t ON t.id = ta.track_id
             JOIN media_files mf ON mf.id = t.primary_file_id
             WHERE ta.artist_id = ar.id
               AND mf.library_id = ANY($1)
               AND ($2::uuid IS NULL OR mf.library_id = $2)
               AND mf.scan_state = 'indexed'
         )",
    )
    .bind(visible)
    .bind(library_id)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("count artists", error))
}

pub async fn list_albums(
    pool: &PgPool,
    artist_id: Option<Uuid>,
    visible: &[Uuid],
) -> Result<Vec<AlbumSummary>> {
    list_albums_page(pool, artist_id, None, visible, None, 0, "artist").await
}

pub async fn list_albums_page(
    pool: &PgPool,
    artist_id: Option<Uuid>,
    library_id: Option<Uuid>,
    visible: &[Uuid],
    limit: Option<i64>,
    offset: i64,
    sort: &str,
) -> Result<Vec<AlbumSummary>> {
    let order = match sort {
        "name" => "lower(al.title), lower(ar.name)",
        "year" => "al.year DESC NULLS LAST, lower(al.title)",
        _ => "lower(ar.name), al.year, lower(al.title)",
    };
    let query = format!(
        "SELECT
             al.id,
             al.title,
             al.year,
             al.album_artist_id,
             ar.name AS album_artist,
             al.artwork_id,
             (SELECT count(*)
              FROM tracks t
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE t.album_id = al.id
                AND mf.library_id = ANY($2)
                AND ($3::uuid IS NULL OR mf.library_id = $3)
                AND mf.scan_state = 'indexed') AS track_count,
             (SELECT COALESCE(sum(t.duration_ms), 0)::bigint
              FROM tracks t
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE t.album_id = al.id
                AND mf.library_id = ANY($2)
                AND ($3::uuid IS NULL OR mf.library_id = $3)
                AND mf.scan_state = 'indexed') AS duration_ms
         FROM albums al
         JOIN artists ar ON ar.id = al.album_artist_id
         WHERE ($1::uuid IS NULL OR al.album_artist_id = $1)
           AND EXISTS (
               SELECT 1
               FROM tracks t
               JOIN media_files mf ON mf.id = t.primary_file_id
               WHERE t.album_id = al.id
                 AND mf.library_id = ANY($2)
                 AND ($3::uuid IS NULL OR mf.library_id = $3)
                 AND mf.scan_state = 'indexed'
           )
         ORDER BY {order}
         LIMIT $4 OFFSET $5"
    );

    sqlx::query_as::<_, AlbumSummary>(&query)
        .bind(artist_id)
        .bind(visible)
        .bind(library_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list albums", error))
}

pub async fn count_albums(
    pool: &PgPool,
    artist_id: Option<Uuid>,
    visible: &[Uuid],
    library_id: Option<Uuid>,
) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*)
         FROM albums al
         WHERE ($1::uuid IS NULL OR al.album_artist_id = $1)
           AND EXISTS (
               SELECT 1
               FROM tracks t
               JOIN media_files mf ON mf.id = t.primary_file_id
               WHERE t.album_id = al.id
                 AND mf.library_id = ANY($2)
                 AND ($3::uuid IS NULL OR mf.library_id = $3)
                 AND mf.scan_state = 'indexed'
           )",
    )
    .bind(artist_id)
    .bind(visible)
    .bind(library_id)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("count albums", error))
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct GenreSummary {
    pub id: Uuid,
    pub name: String,
    pub track_count: i64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TrackFilters {
    pub library_id: Option<Uuid>,
    pub genre_id: Option<Uuid>,
    pub missing_artwork: bool,
    pub missing_year: bool,
    pub uploaded_by: Option<Uuid>,
}

pub async fn list_tracks_page(
    pool: &PgPool,
    visible: &[Uuid],
    filters: TrackFilters,
    limit: Option<i64>,
    offset: i64,
) -> Result<Vec<TrackSummary>> {
    let query = format!(
        "SELECT {TRACK_COLUMNS}
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE {VISIBLE_TRACK}
           AND ($2::uuid IS NULL OR mf.library_id = $2)
           AND ($3::uuid IS NULL OR EXISTS (
               SELECT 1 FROM track_genres tg
               WHERE tg.track_id = t.id AND tg.genre_id = $3
           ))
           AND ($4::bool = false OR al.artwork_id IS NULL)
           AND ($5::bool = false OR al.year IS NULL OR al.year <= 0)
           AND ($6::uuid IS NULL OR mf.uploaded_by = $6)
         ORDER BY lower(t.title), t.disc_no, t.track_no
         LIMIT $7 OFFSET $8"
    );

    sqlx::query_as::<_, TrackSummary>(&query)
        .bind(visible)
        .bind(filters.library_id)
        .bind(filters.genre_id)
        .bind(filters.missing_artwork)
        .bind(filters.missing_year)
        .bind(filters.uploaded_by)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list tracks", error))
}

pub async fn count_tracks(pool: &PgPool, visible: &[Uuid], filters: TrackFilters) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*)
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         WHERE mf.library_id = ANY($1)
           AND mf.scan_state = 'indexed'
           AND ($2::uuid IS NULL OR mf.library_id = $2)
           AND ($3::uuid IS NULL OR EXISTS (
               SELECT 1 FROM track_genres tg
               WHERE tg.track_id = t.id AND tg.genre_id = $3
           ))
           AND ($4::bool = false OR al.artwork_id IS NULL)
           AND ($5::bool = false OR al.year IS NULL OR al.year <= 0)
           AND ($6::uuid IS NULL OR mf.uploaded_by = $6)",
    )
    .bind(visible)
    .bind(filters.library_id)
    .bind(filters.genre_id)
    .bind(filters.missing_artwork)
    .bind(filters.missing_year)
    .bind(filters.uploaded_by)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("count tracks", error))
}

pub async fn list_genres(
    pool: &PgPool,
    visible: &[Uuid],
    library_id: Option<Uuid>,
) -> Result<Vec<GenreSummary>> {
    sqlx::query_as::<_, GenreSummary>(
        "SELECT g.id, g.name, count(DISTINCT t.id) AS track_count
         FROM genres g
         JOIN track_genres tg ON tg.genre_id = g.id
         JOIN tracks t ON t.id = tg.track_id
         JOIN media_files mf ON mf.id = t.primary_file_id
         WHERE mf.library_id = ANY($1)
           AND ($2::uuid IS NULL OR mf.library_id = $2)
           AND mf.scan_state = 'indexed'
         GROUP BY g.id, g.name
         ORDER BY lower(g.name)",
    )
    .bind(visible)
    .bind(library_id)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("list genres", error))
}

pub async fn get_artist(
    pool: &PgPool,
    artist_id: Uuid,
    visible: &[Uuid],
) -> Result<Option<ArtistDetail>> {
    let artist = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT ar.id, ar.name
         FROM artists ar
         WHERE ar.id = $1
           AND EXISTS (
               SELECT 1
               FROM track_artists ta
               JOIN tracks t ON t.id = ta.track_id
               JOIN media_files mf ON mf.id = t.primary_file_id
               WHERE ta.artist_id = ar.id
                 AND mf.library_id = ANY($2)
                 AND mf.scan_state = 'indexed'
           )",
    )
    .bind(artist_id)
    .bind(visible)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load artist", error))?;

    let Some((id, name)) = artist else {
        return Ok(None);
    };

    let albums = list_albums(pool, Some(artist_id), visible).await?;
    Ok(Some(ArtistDetail { id, name, albums }))
}

pub async fn get_album(
    pool: &PgPool,
    album_id: Uuid,
    visible: &[Uuid],
) -> Result<Option<AlbumDetail>> {
    let album = sqlx::query_as::<_, AlbumSummary>(
        "SELECT
             al.id,
             al.title,
             al.year,
             al.album_artist_id,
             ar.name AS album_artist,
             al.artwork_id,
             (SELECT count(*)
              FROM tracks t
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE t.album_id = al.id
                AND mf.library_id = ANY($2)
                AND mf.scan_state = 'indexed') AS track_count,
             (SELECT COALESCE(sum(t.duration_ms), 0)::bigint
              FROM tracks t
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE t.album_id = al.id
                AND mf.library_id = ANY($2)
                AND mf.scan_state = 'indexed') AS duration_ms
         FROM albums al
         JOIN artists ar ON ar.id = al.album_artist_id
         WHERE al.id = $1
           AND EXISTS (
               SELECT 1
               FROM tracks t
               JOIN media_files mf ON mf.id = t.primary_file_id
               WHERE t.album_id = al.id
                 AND mf.library_id = ANY($2)
                 AND mf.scan_state = 'indexed'
           )",
    )
    .bind(album_id)
    .bind(visible)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load album", error))?;

    let Some(album) = album else {
        return Ok(None);
    };

    let query = format!(
        "SELECT {TRACK_COLUMNS}
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE t.album_id = $2 AND {VISIBLE_TRACK}
         ORDER BY t.disc_no, t.track_no, lower(t.title)"
    );

    let tracks = sqlx::query_as::<_, TrackSummary>(&query)
        .bind(visible)
        .bind(album_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load album tracks", error))?;

    Ok(Some(AlbumDetail { album, tracks }))
}

pub async fn get_track(
    pool: &PgPool,
    track_id: Uuid,
    visible: &[Uuid],
) -> Result<Option<TrackDetail>> {
    let query = format!(
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
             mf.sample_rate,
             mf.uploaded_by
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE t.id = $2 AND {VISIBLE_TRACK}"
    );

    sqlx::query_as::<_, TrackDetail>(&query)
        .bind(visible)
        .bind(track_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| db_error("load track", error))
}

pub async fn get_tracks_by_ids(
    pool: &PgPool,
    ids: &[Uuid],
    visible: &[Uuid],
) -> Result<Vec<TrackSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let query = format!(
        "SELECT {TRACK_COLUMNS}
         FROM unnest($2::uuid[]) WITH ORDINALITY AS q(id, ord)
         JOIN tracks t ON t.id = q.id
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE {VISIBLE_TRACK}
         ORDER BY q.ord"
    );

    sqlx::query_as::<_, TrackSummary>(&query)
        .bind(visible)
        .bind(ids)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load tracks", error))
}

pub async fn search(
    pool: &PgPool,
    query: &str,
    limit: i64,
    visible: &[Uuid],
) -> Result<SearchResults> {
    let pattern = format!("%{query}%");

    let artist_query = "SELECT
             ar.id,
             ar.name,
             (SELECT count(DISTINCT al.id)
              FROM albums al
              JOIN tracks t ON t.album_id = al.id
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE al.album_artist_id = ar.id
                AND mf.library_id = ANY($1)
                AND mf.scan_state = 'indexed') AS album_count,
             (SELECT count(*)
              FROM track_artists ta
              JOIN tracks t ON t.id = ta.track_id
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE ta.artist_id = ar.id
                AND mf.library_id = ANY($1)
                AND mf.scan_state = 'indexed') AS track_count
         FROM artists ar
         WHERE ar.name ILIKE $2
           AND EXISTS (
               SELECT 1
               FROM track_artists ta
               JOIN tracks t ON t.id = ta.track_id
               JOIN media_files mf ON mf.id = t.primary_file_id
               WHERE ta.artist_id = ar.id
                 AND mf.library_id = ANY($1)
                 AND mf.scan_state = 'indexed'
           )
         ORDER BY lower(ar.name)
         LIMIT $3"
        .to_string();
    let artists = sqlx::query_as::<_, ArtistSummary>(&artist_query)
        .bind(visible)
        .bind(&pattern)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("search artists", error))?;

    let album_query = "SELECT
             al.id,
             al.title,
             al.year,
             al.album_artist_id,
             ar.name AS album_artist,
             al.artwork_id,
             (SELECT count(*)
              FROM tracks t
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE t.album_id = al.id
                AND mf.library_id = ANY($1)
                AND mf.scan_state = 'indexed') AS track_count,
             (SELECT COALESCE(sum(t.duration_ms), 0)::bigint
              FROM tracks t
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE t.album_id = al.id
                AND mf.library_id = ANY($1)
                AND mf.scan_state = 'indexed') AS duration_ms
         FROM albums al
         JOIN artists ar ON ar.id = al.album_artist_id
         WHERE al.title ILIKE $2
           AND EXISTS (
               SELECT 1
               FROM tracks t
               JOIN media_files mf ON mf.id = t.primary_file_id
               WHERE t.album_id = al.id
                 AND mf.library_id = ANY($1)
                 AND mf.scan_state = 'indexed'
           )
         ORDER BY lower(al.title)
         LIMIT $3"
        .to_string();
    let albums = sqlx::query_as::<_, AlbumSummary>(&album_query)
        .bind(visible)
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
         WHERE t.title ILIKE $2 AND {VISIBLE_TRACK}
         ORDER BY lower(t.title)
         LIMIT $3"
    );
    let tracks = sqlx::query_as::<_, TrackSummary>(&track_query)
        .bind(visible)
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

pub async fn get_album_artwork(
    pool: &PgPool,
    album_id: Uuid,
    visible: &[Uuid],
) -> Result<Option<ArtworkRecord>> {
    sqlx::query_as::<_, ArtworkRecord>(
        "SELECT aw.mime, aw.data, aw.hash AS etag
         FROM albums al
         JOIN artworks aw ON aw.id = al.artwork_id
         WHERE al.id = $2
           AND EXISTS (
               SELECT 1
               FROM tracks t
               JOIN media_files mf ON mf.id = t.primary_file_id
               WHERE t.album_id = al.id
                 AND mf.library_id = ANY($1)
                 AND mf.scan_state = 'indexed'
           )",
    )
    .bind(visible)
    .bind(album_id)
    .fetch_optional(pool)
    .await
    .map_err(|error| db_error("load album artwork", error))
}

pub async fn starred_tracks(
    pool: &PgPool,
    user_id: Uuid,
    visible: &[Uuid],
) -> Result<Vec<TrackSummary>> {
    let query = format!(
        "SELECT {TRACK_COLUMNS}
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         JOIN annotations an ON an.item_type = 'track' AND an.item_id = t.id
         WHERE an.user_id = $2 AND an.starred_at IS NOT NULL AND {VISIBLE_TRACK}
         ORDER BY an.starred_at DESC"
    );

    sqlx::query_as::<_, TrackSummary>(&query)
        .bind(visible)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load starred tracks", error))
}

pub async fn starred_albums(
    pool: &PgPool,
    user_id: Uuid,
    visible: &[Uuid],
) -> Result<Vec<AlbumSummary>> {
    let query = "SELECT
             al.id,
             al.title,
             al.year,
             al.album_artist_id,
             ar.name AS album_artist,
             al.artwork_id,
             (SELECT count(*)
              FROM tracks t
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE t.album_id = al.id
                AND mf.library_id = ANY($1)
                AND mf.scan_state = 'indexed') AS track_count,
             (SELECT COALESCE(sum(t.duration_ms), 0)::bigint
              FROM tracks t
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE t.album_id = al.id
                AND mf.library_id = ANY($1)
                AND mf.scan_state = 'indexed') AS duration_ms
         FROM albums al
         JOIN artists ar ON ar.id = al.album_artist_id
         JOIN annotations an ON an.item_type = 'album' AND an.item_id = al.id
         WHERE an.user_id = $2 AND an.starred_at IS NOT NULL
           AND EXISTS (
               SELECT 1
               FROM tracks t
               JOIN media_files mf ON mf.id = t.primary_file_id
               WHERE t.album_id = al.id
                 AND mf.library_id = ANY($1)
                 AND mf.scan_state = 'indexed'
           )
         ORDER BY an.starred_at DESC"
        .to_string();

    sqlx::query_as::<_, AlbumSummary>(&query)
        .bind(visible)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load starred albums", error))
}

pub async fn starred_artists(
    pool: &PgPool,
    user_id: Uuid,
    visible: &[Uuid],
) -> Result<Vec<ArtistSummary>> {
    let query = "SELECT
             ar.id,
             ar.name,
             (SELECT count(DISTINCT al.id)
              FROM albums al
              JOIN tracks t ON t.album_id = al.id
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE al.album_artist_id = ar.id
                AND mf.library_id = ANY($1)
                AND mf.scan_state = 'indexed') AS album_count,
             (SELECT count(*)
              FROM track_artists ta
              JOIN tracks t ON t.id = ta.track_id
              JOIN media_files mf ON mf.id = t.primary_file_id
              WHERE ta.artist_id = ar.id
                AND mf.library_id = ANY($1)
                AND mf.scan_state = 'indexed') AS track_count
         FROM artists ar
         JOIN annotations an ON an.item_type = 'artist' AND an.item_id = ar.id
         WHERE an.user_id = $2 AND an.starred_at IS NOT NULL
           AND EXISTS (
               SELECT 1
               FROM track_artists ta
               JOIN tracks t ON t.id = ta.track_id
               JOIN media_files mf ON mf.id = t.primary_file_id
               WHERE ta.artist_id = ar.id
                 AND mf.library_id = ANY($1)
                 AND mf.scan_state = 'indexed'
           )
         ORDER BY an.starred_at DESC"
        .to_string();

    sqlx::query_as::<_, ArtistSummary>(&query)
        .bind(visible)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("load starred artists", error))
}

pub async fn list_artist_tracks(
    pool: &PgPool,
    artist_id: Uuid,
    visible: &[Uuid],
    limit: i64,
) -> Result<Vec<TrackSummary>> {
    list_artist_tracks_page(pool, artist_id, visible, limit, 0).await
}

pub async fn list_artist_tracks_page(
    pool: &PgPool,
    artist_id: Uuid,
    visible: &[Uuid],
    limit: i64,
    offset: i64,
) -> Result<Vec<TrackSummary>> {
    let query = format!(
        "SELECT {TRACK_COLUMNS}
         FROM track_artists ta
         JOIN tracks t ON t.id = ta.track_id
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE ta.artist_id = $2 AND {VISIBLE_TRACK}
         ORDER BY lower(t.title), t.disc_no, t.track_no
         LIMIT $3 OFFSET $4"
    );

    sqlx::query_as::<_, TrackSummary>(&query)
        .bind(visible)
        .bind(artist_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list artist tracks", error))
}
