use mc_core::Result;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::catalog::TrackSummary;
use super::db_error;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct PlaylistRecord {
    pub id: Uuid,
    pub name: String,
    pub owner: String,
    pub owner_id: Uuid,
    pub is_public: bool,
    pub song_count: i64,
    pub duration_ms: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlaylistDetail {
    pub playlist: PlaylistRecord,
    pub tracks: Vec<TrackSummary>,
    pub hidden_count: i64,
}

const PLAYLIST_COLUMNS: &str = "
    p.id,
    p.name,
    u.username AS owner,
    p.owner_id,
    p.is_public,
    (SELECT count(*)
     FROM playlist_entries pe
     JOIN tracks t ON t.id = pe.track_id
     JOIN media_files mf ON mf.id = t.primary_file_id
     WHERE pe.playlist_id = p.id
       AND mf.library_id = ANY($1)
       AND mf.scan_state = 'indexed') AS song_count,
    (SELECT COALESCE(sum(t.duration_ms), 0)::bigint
     FROM playlist_entries pe
     JOIN tracks t ON t.id = pe.track_id
     JOIN media_files mf ON mf.id = t.primary_file_id
     WHERE pe.playlist_id = p.id
       AND mf.library_id = ANY($1)
       AND mf.scan_state = 'indexed') AS duration_ms
";

pub async fn list_with_counts(pool: &PgPool, visible: &[Uuid]) -> Result<Vec<PlaylistRecord>> {
    let query = format!(
        "SELECT {PLAYLIST_COLUMNS}
         FROM playlists p
         JOIN users u ON u.id = p.owner_id
         ORDER BY lower(p.name)"
    );

    sqlx::query_as::<_, PlaylistRecord>(&query)
        .bind(visible)
        .fetch_all(pool)
        .await
        .map_err(|error| db_error("list playlists", error))
}

pub async fn get(
    pool: &PgPool,
    playlist_id: Uuid,
    visible: &[Uuid],
) -> Result<Option<PlaylistDetail>> {
    let query = format!(
        "SELECT {PLAYLIST_COLUMNS}
         FROM playlists p
         JOIN users u ON u.id = p.owner_id
         WHERE p.id = $2"
    );

    let Some(playlist) = sqlx::query_as::<_, PlaylistRecord>(&query)
        .bind(visible)
        .bind(playlist_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| db_error("load playlist", error))?
    else {
        return Ok(None);
    };

    let tracks = sqlx::query_as::<_, TrackSummary>(
        "SELECT
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
         FROM playlist_entries pe
         JOIN tracks t ON t.id = pe.track_id
         JOIN media_files mf ON mf.id = t.primary_file_id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = al.album_artist_id
         WHERE pe.playlist_id = $2
           AND mf.library_id = ANY($1)
           AND mf.scan_state = 'indexed'
         ORDER BY pe.position",
    )
    .bind(visible)
    .bind(playlist_id)
    .fetch_all(pool)
    .await
    .map_err(|error| db_error("load playlist tracks", error))?;

    let hidden_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*)
         FROM playlist_entries pe
         JOIN tracks t ON t.id = pe.track_id
         JOIN media_files mf ON mf.id = t.primary_file_id
         WHERE pe.playlist_id = $2
           AND NOT (mf.library_id = ANY($1) AND mf.scan_state = 'indexed')",
    )
    .bind(visible)
    .bind(playlist_id)
    .fetch_one(pool)
    .await
    .map_err(|error| db_error("count hidden playlist tracks", error))?;

    Ok(Some(PlaylistDetail {
        playlist,
        tracks,
        hidden_count,
    }))
}

pub async fn create(pool: &PgPool, owner_id: Uuid, name: &str, track_ids: &[Uuid]) -> Result<Uuid> {
    let playlist_id = Uuid::now_v7();
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| db_error("begin playlist transaction", error))?;

    sqlx::query("INSERT INTO playlists (id, owner_id, name) VALUES ($1, $2, $3)")
        .bind(playlist_id)
        .bind(owner_id)
        .bind(name)
        .execute(&mut *transaction)
        .await
        .map_err(|error| db_error("create playlist", error))?;

    insert_tracks(&mut transaction, playlist_id, track_ids).await?;

    transaction
        .commit()
        .await
        .map_err(|error| db_error("commit playlist transaction", error))?;

    Ok(playlist_id)
}

pub async fn update(
    pool: &PgPool,
    playlist_id: Uuid,
    name: Option<&str>,
    is_public: Option<bool>,
    add: &[Uuid],
    remove: &[usize],
) -> Result<bool> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| db_error("begin playlist update transaction", error))?;

    let exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM playlists WHERE id = $1)")
            .bind(playlist_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|error| db_error("check playlist", error))?;

    if !exists {
        transaction
            .rollback()
            .await
            .map_err(|error| db_error("rollback playlist update", error))?;
        return Ok(false);
    }

    if let Some(name) = name {
        sqlx::query("UPDATE playlists SET name = $2, updated_at = now() WHERE id = $1")
            .bind(playlist_id)
            .bind(name)
            .execute(&mut *transaction)
            .await
            .map_err(|error| db_error("rename playlist", error))?;
    }

    if let Some(is_public) = is_public {
        sqlx::query("UPDATE playlists SET is_public = $2, updated_at = now() WHERE id = $1")
            .bind(playlist_id)
            .bind(is_public)
            .execute(&mut *transaction)
            .await
            .map_err(|error| db_error("update playlist visibility", error))?;
    }

    let mut track_ids = sqlx::query_scalar::<_, Uuid>(
        "SELECT track_id FROM playlist_entries WHERE playlist_id = $1 ORDER BY position",
    )
    .bind(playlist_id)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|error| db_error("load playlist entries", error))?;

    if !remove.is_empty() {
        let remove = remove
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        track_ids = track_ids
            .into_iter()
            .enumerate()
            .filter(|(index, _)| !remove.contains(index))
            .map(|(_, track_id)| track_id)
            .collect();
    }

    track_ids.extend_from_slice(add);

    sqlx::query("DELETE FROM playlist_entries WHERE playlist_id = $1")
        .bind(playlist_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| db_error("clear playlist entries", error))?;

    insert_tracks(&mut transaction, playlist_id, &track_ids).await?;

    transaction
        .commit()
        .await
        .map_err(|error| db_error("commit playlist update transaction", error))?;

    Ok(true)
}

pub async fn delete(pool: &PgPool, playlist_id: Uuid) -> Result<bool> {
    let result = sqlx::query("DELETE FROM playlists WHERE id = $1")
        .bind(playlist_id)
        .execute(pool)
        .await
        .map_err(|error| db_error("delete playlist", error))?;

    Ok(result.rows_affected() > 0)
}

async fn insert_tracks(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    playlist_id: Uuid,
    track_ids: &[Uuid],
) -> Result<()> {
    for (position, track_id) in track_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO playlist_entries (playlist_id, track_id, position)
             VALUES ($1, $2, $3)",
        )
        .bind(playlist_id)
        .bind(track_id)
        .bind(i32::try_from(position).unwrap_or(i32::MAX))
        .execute(&mut **transaction)
        .await
        .map_err(|error| db_error("insert playlist entry", error))?;
    }

    Ok(())
}
