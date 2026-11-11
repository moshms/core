use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use mc_core::{Result, StorageId};
use mc_db::repos::libraries;
use mc_media::{MediaMetadata, MediaParser};
use mc_scanner::{ScanMode, Scanner};
use mc_storage::{LocalStorage, Storage};
use tempfile::TempDir;

#[derive(Default)]
struct CountingParser {
    calls: AtomicUsize,
}

impl MediaParser for CountingParser {
    fn parse(&self, _data: &[u8]) -> Result<MediaMetadata> {
        self.calls.fetch_add(1, Ordering::SeqCst);

        Ok(MediaMetadata {
            title: Some("Song".to_owned()),
            artist: Some("Artist".to_owned()),
            album_artist: Some("Artist".to_owned()),
            album: Some("Album".to_owned()),
            year: Some(2024),
            track_no: Some(1),
            disc_no: Some(1),
            genres: vec!["Rock".to_owned()],
            duration_ms: Some(1_000),
            ..MediaMetadata::default()
        })
    }
}

#[tokio::test]
async fn rescan_skips_unchanged_files() {
    let Ok(database_url) = std::env::var("MC_TEST_DATABASE_URL") else {
        eprintln!("MC_TEST_DATABASE_URL is not set, skipping");
        return;
    };

    let pool = mc_db::connect(&database_url, 5).await.unwrap();
    mc_db::migrate(&pool).await.unwrap();

    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("song.mp3"), b"not really audio").unwrap();

    let name = format!(
        "test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let library = libraries::ensure_default(&pool, &name, directory.path().to_str().unwrap())
        .await
        .unwrap();

    let storage: Arc<dyn Storage> = Arc::new(
        LocalStorage::new(StorageId::from_uuid(library.storage_id), directory.path()).unwrap(),
    );
    let parser = Arc::new(CountingParser::default());
    let scanner = Scanner::new(pool.clone(), parser.clone());

    let first = scanner
        .scan_library(&library, storage.clone(), ScanMode::Full)
        .await
        .unwrap();
    assert_eq!(first.files_seen, 1);
    assert_eq!(first.added, 1);
    assert_eq!(first.tags_read, 1);

    let second = scanner
        .scan_library(&library, storage.clone(), ScanMode::Full)
        .await
        .unwrap();
    assert_eq!(second.files_seen, 1);
    assert_eq!(second.unchanged, 1);
    assert_eq!(second.added, 0);
    assert_eq!(second.updated, 0);
    assert_eq!(second.tags_read, 0);
    assert_eq!(parser.calls.load(Ordering::SeqCst), 1);

    sqlx::query("DELETE FROM libraries WHERE id = $1")
        .bind(library.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM storages WHERE id = $1")
        .bind(library.storage_id)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn deduplicates_identical_content() {
    let Ok(database_url) = std::env::var("MC_TEST_DATABASE_URL") else {
        eprintln!("MC_TEST_DATABASE_URL is not set, skipping");
        return;
    };

    let pool = mc_db::connect(&database_url, 5).await.unwrap();
    mc_db::migrate(&pool).await.unwrap();

    let directory = TempDir::new().unwrap();
    std::fs::write(directory.path().join("one.mp3"), b"identical bytes").unwrap();
    std::fs::write(directory.path().join("two.mp3"), b"identical bytes").unwrap();

    let name = format!(
        "dedup-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let library = libraries::ensure_default(&pool, &name, directory.path().to_str().unwrap())
        .await
        .unwrap();
    let storage: Arc<dyn Storage> = Arc::new(
        LocalStorage::new(StorageId::from_uuid(library.storage_id), directory.path()).unwrap(),
    );
    let scanner = Scanner::new(pool.clone(), Arc::new(CountingParser::default()));

    let stats = scanner
        .scan_library(&library, storage, ScanMode::Full)
        .await
        .unwrap();

    assert_eq!(stats.files_seen, 2);
    assert_eq!(stats.added, 1);
    assert_eq!(stats.duplicates, 1);

    let tracks: i64 = sqlx::query_scalar(
        "SELECT count(*)
         FROM tracks t
         JOIN media_files mf ON mf.id = t.primary_file_id
         WHERE mf.library_id = $1",
    )
    .bind(library.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let files: i64 = sqlx::query_scalar("SELECT count(*) FROM media_files WHERE library_id = $1")
        .bind(library.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let duplicates: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM media_files WHERE library_id = $1 AND scan_state = 'duplicate'",
    )
    .bind(library.id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(tracks, 1);
    assert_eq!(files, 2);
    assert_eq!(duplicates, 1);

    sqlx::query("DELETE FROM libraries WHERE id = $1")
        .bind(library.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM storages WHERE id = $1")
        .bind(library.storage_id)
        .execute(&pool)
        .await
        .unwrap();
}
