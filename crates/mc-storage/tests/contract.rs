use std::sync::Arc;

use bytes::Bytes;
use futures_util::TryStreamExt;
use mc_storage::{Delivery, Key, LocalStorage, MemoryStorage, S3Config, S3Storage, Storage};
use tempfile::TempDir;

async fn run_contract(storage: Arc<dyn Storage>) {
    let root = Key::root();
    let key = Key::new("artist/album/01 track.flac").unwrap();

    let meta = storage
        .write(&key, Bytes::from_static(b"hello world"))
        .await
        .unwrap();
    assert_eq!(meta.size, 11);

    let stat = storage.stat(&key).await.unwrap();
    assert_eq!(stat, meta);

    let all = storage.read(&key).await.unwrap();
    assert_eq!(all, Bytes::from_static(b"hello world"));

    let range = storage.read_range(&key, 6..11).await.unwrap();
    assert_eq!(range, Bytes::from_static(b"world"));

    let entries = storage.list(&root).try_collect::<Vec<_>>().await.unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, key);
    assert_eq!(entries[0].meta.size, meta.size);
    assert_eq!(entries[0].meta.etag, meta.etag);

    let prefixed = storage
        .list(&Key::new("artist/album").unwrap())
        .try_collect::<Vec<_>>()
        .await
        .unwrap();
    assert_eq!(prefixed.len(), 1);

    let renamed = Key::new("artist/album/01 renamed.flac").unwrap();
    storage.rename(&key, &renamed).await.unwrap();
    assert!(storage.stat(&key).await.is_err());
    assert!(storage.stat(&renamed).await.is_ok());

    let delivery = storage.delivery(&renamed).await.unwrap();
    match (&delivery, storage.caps().local_path) {
        (Delivery::NginxInternal(path), true) => {
            assert_eq!(path, "/_media/artist/album/01%20renamed.flac");
        }
        (Delivery::Stream, _) => {}
        (Delivery::Presigned(url), false) if storage.caps().presign => {
            assert!(url.starts_with("http"));
        }
        other => panic!("unexpected delivery: {other:?}"),
    }

    storage.delete(&renamed).await.unwrap();
    assert!(storage.stat(&renamed).await.is_err());
    assert!(storage.delete(&renamed).await.is_err());
}

#[tokio::test]
async fn memory_backend_satisfies_contract() {
    run_contract(Arc::new(MemoryStorage::new())).await;
}

#[tokio::test]
async fn local_backend_satisfies_contract() {
    let directory = TempDir::new().unwrap();
    let storage = LocalStorage::new(mc_core::StorageId::new(), directory.path()).unwrap();
    run_contract(Arc::new(storage)).await;
}

#[tokio::test]
async fn s3_backend_satisfies_contract() {
    let Ok(endpoint) = std::env::var("MC_TEST_S3_ENDPOINT") else {
        eprintln!("MC_TEST_S3_ENDPOINT is not set, skipping");
        return;
    };
    let bucket = std::env::var("MC_TEST_S3_BUCKET").expect("MC_TEST_S3_BUCKET");
    let access_key_id = std::env::var("MC_TEST_S3_ACCESS_KEY").expect("MC_TEST_S3_ACCESS_KEY");
    let secret_access_key = std::env::var("MC_TEST_S3_SECRET_KEY").expect("MC_TEST_S3_SECRET_KEY");
    let root = format!(
        "test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let config = S3Config {
        endpoint,
        region: std::env::var("MC_TEST_S3_REGION").unwrap_or_else(|_| "us-east-1".to_owned()),
        bucket,
        access_key_id,
        secret_access_key,
        root,
        virtual_host_style: false,
        presign_seconds: 600,
    };
    let storage = S3Storage::new(mc_core::StorageId::new(), &config).unwrap();

    run_contract(Arc::new(storage)).await;
}
