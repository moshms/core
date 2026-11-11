use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use futures_util::StreamExt;
use mc_core::Result;
use mc_db::repos::libraries::LibraryRecord;
use mc_db::repos::media_files::{MediaFileState, NewMediaFile};
use mc_db::repos::tracks::{NewTrack, TrackArtist};
use mc_db::repos::{albums, artists, artworks, genres, media_files, scan_runs, tracks};
use mc_db::PgPool;
use mc_media::{ArtworkData, MediaMetadata, MediaParser};
use mc_storage::{Entry, Key, Meta, Storage};
use serde::Serialize;
use tracing::{info, warn};

const UNKNOWN_ARTIST: &str = "Unknown Artist";
const UNKNOWN_ALBUM: &str = "Unknown Album";

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "mp4", "aac", "wav", "aiff", "aif", "wv", "ape",
    "mpc", "wma",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanMode {
    Full,
}

impl ScanMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct ScanStats {
    pub files_seen: u64,
    pub added: u64,
    pub updated: u64,
    pub unchanged: u64,
    pub removed: u64,
    pub tags_read: u64,
    pub duplicates: u64,
    pub errors: u64,
}

pub struct Scanner {
    pool: PgPool,
    parser: Arc<dyn MediaParser>,
}

impl Scanner {
    #[must_use]
    pub fn new(pool: PgPool, parser: Arc<dyn MediaParser>) -> Self {
        Self { pool, parser }
    }

    pub async fn scan_library(
        &self,
        library: &LibraryRecord,
        storage: Arc<dyn Storage>,
        mode: ScanMode,
    ) -> Result<ScanStats> {
        let run_id = scan_runs::start(&self.pool, library.id, mode.as_str()).await?;
        let result = self.scan(library, storage).await;

        match &result {
            Ok(stats) => {
                let counters = serde_json::to_value(stats).unwrap_or_default();
                scan_runs::finish(&self.pool, run_id, "completed", &counters, None).await?;
            }
            Err(error) => {
                let counters = serde_json::json!({});
                scan_runs::finish(
                    &self.pool,
                    run_id,
                    "failed",
                    &counters,
                    Some(&error.to_string()),
                )
                .await?;
            }
        }

        result
    }

    async fn scan(&self, library: &LibraryRecord, storage: Arc<dyn Storage>) -> Result<ScanStats> {
        let existing = media_files::load_indexed(&self.pool, library.id).await?;
        let prefix = Key::new(library.root_prefix.as_str())?;
        let mut stream = storage.list(&prefix);
        let mut stats = ScanStats::default();
        let mut seen = HashSet::new();

        while let Some(item) = stream.next().await {
            let entry = match item {
                Ok(entry) => entry,
                Err(error) => {
                    stats.errors += 1;
                    warn!(%error, "storage listing failed");
                    continue;
                }
            };

            if !is_audio_key(entry.key.as_str()) {
                continue;
            }

            stats.files_seen += 1;
            seen.insert(entry.key.as_str().to_owned());

            if !needs_index(existing.get(entry.key.as_str()), &entry.meta) {
                stats.unchanged += 1;
                continue;
            }

            let was_known = existing.contains_key(entry.key.as_str());
            match self
                .index_entry(library, &storage, &entry, was_known, &mut stats)
                .await
            {
                Ok(IndexOutcome::Indexed) => {
                    if was_known {
                        stats.updated += 1;
                    } else {
                        stats.added += 1;
                    }
                }
                Ok(IndexOutcome::Duplicate) => {
                    stats.duplicates += 1;
                }
                Err(error) => {
                    stats.errors += 1;
                    warn!(key = %entry.key, %error, "failed to index file");

                    if let Err(mark_error) =
                        media_files::mark_failed(&self.pool, library.id, entry.key.as_str()).await
                    {
                        warn!(key = %entry.key, %mark_error, "failed to mark media file");
                    }
                }
            }
        }

        let removed = existing
            .keys()
            .filter(|key| !seen.contains(*key))
            .cloned()
            .collect::<Vec<_>>();
        stats.removed = media_files::mark_missing(&self.pool, library.id, &removed).await?;

        info!(?stats, "scan finished");
        Ok(stats)
    }

    async fn index_entry(
        &self,
        library: &LibraryRecord,
        storage: &Arc<dyn Storage>,
        entry: &Entry,
        was_known: bool,
        stats: &mut ScanStats,
    ) -> Result<IndexOutcome> {
        let data = storage.read(&entry.key).await?;
        stats.tags_read += 1;
        let metadata = self.parser.parse(&data)?;
        let content_hash = blake3::hash(&data).to_hex().to_string();

        let title = metadata
            .title
            .as_deref()
            .and_then(clean)
            .map(str::to_owned)
            .unwrap_or_else(|| file_stem(entry.key.as_str()));
        let artist_name = metadata
            .artist
            .as_deref()
            .and_then(clean)
            .unwrap_or(UNKNOWN_ARTIST);
        let album_artist_name = metadata
            .album_artist
            .as_deref()
            .and_then(clean)
            .unwrap_or(artist_name);
        let album_title = metadata
            .album
            .as_deref()
            .and_then(clean)
            .unwrap_or(UNKNOWN_ALBUM);
        let year = metadata.year.unwrap_or(0);

        let artwork_id = self.store_artwork(metadata.artwork.as_ref()).await?;
        let file_id = self
            .store_media_file(library, entry, &metadata, &content_hash)
            .await?;

        if !was_known {
            if let Some(existing_track) =
                media_files::find_track_by_content_hash(&self.pool, &content_hash, file_id).await?
            {
                media_files::mark_duplicate(&self.pool, library.id, entry.key.as_str()).await?;
                tracing::debug!(
                    key = %entry.key,
                    track = %existing_track,
                    "duplicate media file skipped"
                );
                return Ok(IndexOutcome::Duplicate);
            }
        }

        let main_artist_id = artists::upsert(&self.pool, artist_name).await?;
        let album_artist_id = if album_artist_name == artist_name {
            main_artist_id
        } else {
            artists::upsert(&self.pool, album_artist_name).await?
        };
        let album_id =
            albums::upsert(&self.pool, album_title, album_artist_id, year, artwork_id).await?;

        let track_id = tracks::upsert(
            &self.pool,
            &NewTrack {
                title: &title,
                album_id,
                disc_no: i32::try_from(metadata.disc_no.unwrap_or(0)).unwrap_or(0),
                track_no: i32::try_from(metadata.track_no.unwrap_or(0)).unwrap_or(0),
                year,
                duration_ms: metadata.duration_ms,
                primary_file_id: file_id,
            },
        )
        .await?;

        let mut track_artists = vec![TrackArtist {
            artist_id: main_artist_id,
            role: "main",
            position: 0,
        }];
        if album_artist_id != main_artist_id {
            track_artists.push(TrackArtist {
                artist_id: album_artist_id,
                role: "album",
                position: 1,
            });
        }
        tracks::set_artists(&self.pool, track_id, &track_artists).await?;
        genres::set_for_track(&self.pool, track_id, &metadata.genres).await?;

        Ok(IndexOutcome::Indexed)
    }

    async fn store_artwork(&self, artwork: Option<&ArtworkData>) -> Result<Option<uuid::Uuid>> {
        let Some(artwork) = artwork else {
            return Ok(None);
        };

        artworks::upsert(
            &self.pool,
            &artworks::NewArtwork {
                hash: &artwork.hash,
                source: "embedded",
                mime: &artwork.mime,
                data: &artwork.data,
            },
        )
        .await
        .map(Some)
    }

    async fn store_media_file(
        &self,
        library: &LibraryRecord,
        entry: &Entry,
        metadata: &MediaMetadata,
        content_hash: &str,
    ) -> Result<uuid::Uuid> {
        media_files::upsert(
            &self.pool,
            &NewMediaFile {
                library_id: library.id,
                key: entry.key.as_str(),
                size: i64::try_from(entry.meta.size).unwrap_or(i64::MAX),
                etag: &entry.meta.etag,
                mtime_ms: i64::try_from(entry.meta.modified_ms).unwrap_or(i64::MAX),
                format: metadata.format.as_deref(),
                bitrate: metadata.bitrate.and_then(|value| i32::try_from(value).ok()),
                sample_rate: metadata
                    .sample_rate
                    .and_then(|value| i32::try_from(value).ok()),
                duration_ms: metadata.duration_ms,
                content_hash: Some(content_hash),
            },
        )
        .await
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndexOutcome {
    Indexed,
    Duplicate,
}

fn needs_index(existing: Option<&MediaFileState>, meta: &Meta) -> bool {
    let Some(existing) = existing else {
        return true;
    };

    let size = i64::try_from(meta.size).unwrap_or(i64::MAX);
    let mtime_ms = i64::try_from(meta.modified_ms).unwrap_or(i64::MAX);

    existing.size != size || existing.mtime_ms != mtime_ms
}

fn is_audio_key(key: &str) -> bool {
    Path::new(key)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let extension = extension.to_ascii_lowercase();
            AUDIO_EXTENSIONS.contains(&extension.as_str())
        })
        .unwrap_or(false)
}

fn file_stem(key: &str) -> String {
    Path::new(key)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("Untitled")
        .to_owned()
}

fn clean(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{is_audio_key, needs_index, ScanMode};
    use mc_db::repos::media_files::MediaFileState;
    use mc_storage::Meta;

    #[test]
    fn detects_audio_extensions_case_insensitively() {
        assert!(is_audio_key("Artist/Album/Track.FLAC"));
        assert!(is_audio_key("track.mp3"));
        assert!(!is_audio_key("cover.jpg"));
        assert!(!is_audio_key("notes"));
    }

    #[test]
    fn unchanged_metadata_skips_indexing() {
        let state = MediaFileState {
            id: uuid::Uuid::nil(),
            key: "track.mp3".to_owned(),
            size: 10,
            etag: "1-10".to_owned(),
            mtime_ms: 1,
        };
        let same = Meta {
            size: 10,
            etag: "1-10".to_owned(),
            modified_ms: 1,
        };
        let changed = Meta {
            size: 11,
            etag: "1-11".to_owned(),
            modified_ms: 1,
        };

        assert!(!needs_index(Some(&state), &same));
        assert!(needs_index(Some(&state), &changed));
        assert!(needs_index(None, &same));
    }

    #[test]
    fn scan_mode_has_stable_name() {
        assert_eq!(ScanMode::Full.as_str(), "full");
    }
}
