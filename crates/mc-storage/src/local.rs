use std::io::ErrorKind;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_stream::try_stream;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::StreamExt;
use mc_core::{CoreError, Result, StorageId};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};

use crate::{Caps, Delivery, Entry, EntryStream, Key, Meta, Storage};

#[derive(Debug)]
pub struct LocalStorage {
    id: StorageId,
    root: PathBuf,
}

impl LocalStorage {
    pub fn new(id: StorageId, root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();

        let metadata = std::fs::metadata(&root).map_err(|error| {
            CoreError::Unavailable(format!("local storage {}: {error}", root.display()))
        })?;

        if !metadata.is_dir() {
            return Err(CoreError::Validation(format!(
                "local storage root is not a directory: {}",
                root.display()
            )));
        }

        Ok(Self {
            id,
            root: root.canonicalize().map_err(|error| {
                CoreError::Unavailable(format!("local storage {}: {error}", root.display()))
            })?,
        })
    }

    fn resolve(&self, key: &Key) -> Result<PathBuf> {
        if key.is_root() {
            return Ok(self.root.clone());
        }

        Ok(self.root.join(key.as_str()))
    }

    fn key_from_path(&self, path: &Path) -> Result<Key> {
        let relative = path.strip_prefix(&self.root).map_err(|error| {
            CoreError::Internal(format!(
                "path outside storage root {}: {error}",
                path.display()
            ))
        })?;

        let mut parts = Vec::new();
        for component in relative.components() {
            let value = component.as_os_str().to_str().ok_or_else(|| {
                CoreError::Validation(format!("non-UTF-8 path in storage: {}", path.display()))
            })?;
            parts.push(value);
        }

        Key::new(parts.join("/"))
    }
}

#[async_trait]
impl Storage for LocalStorage {
    fn id(&self) -> StorageId {
        self.id
    }

    fn caps(&self) -> Caps {
        Caps {
            read: true,
            write: true,
            watch: true,
            presign: false,
            rename: true,
            local_path: true,
        }
    }

    fn list(&self, prefix: &Key) -> EntryStream<'_> {
        let start = match self.resolve(prefix) {
            Ok(path) => path,
            Err(error) => return futures_util::stream::once(async move { Err(error) }).boxed(),
        };

        let storage = self;
        let prefix = prefix.clone();
        Box::pin(try_stream! {
            let metadata = match fs::metadata(&start).await {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    Err(CoreError::not_found("object", &prefix))?;
                    unreachable!();
                }
                Err(error) => {
                    Err(CoreError::Unavailable(format!("stat {}: {error}", start.display())))?;
                    unreachable!();
                }
            };

            if metadata.is_file() {
                yield Entry {
                    key: storage.key_from_path(&start)?,
                    meta: meta_from_metadata(&metadata),
                };
                return;
            }

            let mut stack = vec![start];
            while let Some(directory) = stack.pop() {
                let mut reader = fs::read_dir(&directory).await.map_err(|error| {
                    CoreError::Unavailable(format!("list {}: {error}", directory.display()))
                })?;

                while let Some(entry) = reader.next_entry().await.map_err(|error| {
                    CoreError::Unavailable(format!("list {}: {error}", directory.display()))
                })? {
                    let file_name = entry.file_name();
                    if file_name.to_string_lossy().starts_with('.') {
                        continue;
                    }

                    let file_type = entry.file_type().await.map_err(|error| {
                        CoreError::Unavailable(format!(
                            "stat {}: {error}",
                            entry.path().display()
                        ))
                    })?;

                    if file_type.is_symlink() {
                        continue;
                    }

                    if file_type.is_dir() {
                        stack.push(entry.path());
                        continue;
                    }

                    if !file_type.is_file() {
                        continue;
                    }

                    let path = entry.path();
                    let metadata = entry.metadata().await.map_err(|error| {
                        CoreError::Unavailable(format!("stat {}: {error}", path.display()))
                    })?;

                    yield Entry {
                        key: storage.key_from_path(&path)?,
                        meta: meta_from_metadata(&metadata),
                    };
                }
            }
        })
    }

    async fn stat(&self, key: &Key) -> Result<Meta> {
        let path = self.resolve(key)?;
        let metadata = fs::metadata(&path)
            .await
            .map_err(|error| io_error("stat", &path, error))?;

        if !metadata.is_file() {
            return Err(CoreError::not_found("object", key));
        }

        Ok(meta_from_metadata(&metadata))
    }

    async fn read_range(&self, key: &Key, range: Range<u64>) -> Result<Bytes> {
        if range.start > range.end {
            return Err(CoreError::Validation(format!(
                "invalid byte range {}..{}",
                range.start, range.end
            )));
        }

        let path = self.resolve(key)?;
        let mut file = fs::File::open(&path)
            .await
            .map_err(|error| io_error("open", &path, error))?;

        if range.start > 0 {
            file.seek(SeekFrom::Start(range.start))
                .await
                .map_err(|error| io_error("seek", &path, error))?;
        }

        let length = range.end - range.start;
        let mut buffer = Vec::with_capacity(usize::try_from(length).unwrap_or(0));
        file.take(length)
            .read_to_end(&mut buffer)
            .await
            .map_err(|error| io_error("read", &path, error))?;

        Ok(Bytes::from(buffer))
    }

    async fn read(&self, key: &Key) -> Result<Bytes> {
        let path = self.resolve(key)?;
        let data = fs::read(&path)
            .await
            .map_err(|error| io_error("read", &path, error))?;
        Ok(Bytes::from(data))
    }

    async fn write(&self, key: &Key, data: Bytes) -> Result<Meta> {
        let path = self.resolve(key)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|error| io_error("create directory", parent, error))?;
        }

        fs::write(&path, &data)
            .await
            .map_err(|error| io_error("write", &path, error))?;

        self.stat(key).await
    }

    async fn write_stream(
        &self,
        key: &Key,
        mut stream: futures_util::stream::BoxStream<'static, Result<Bytes>>,
    ) -> Result<Meta> {
        use futures_util::StreamExt;

        let path = self.resolve(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|error| io_error("create directory", parent, error))?;
        }
        let mut file = fs::File::create(&path)
            .await
            .map_err(|error| io_error("create", &path, error))?;
        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk?)
                .await
                .map_err(|error| io_error("write", &path, error))?;
        }
        file.flush()
            .await
            .map_err(|error| io_error("flush", &path, error))?;
        drop(file);

        self.stat(key).await
    }

    async fn rename(&self, from: &Key, to: &Key) -> Result<()> {
        let source = self.resolve(from)?;
        let target = self.resolve(to)?;

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|error| io_error("create directory", parent, error))?;
        }

        fs::rename(&source, &target)
            .await
            .map_err(|error| io_error("rename", &source, error))
    }

    async fn delete(&self, key: &Key) -> Result<()> {
        let path = self.resolve(key)?;
        fs::remove_file(&path)
            .await
            .map_err(|error| io_error("delete", &path, error))
    }

    async fn delivery(&self, key: &Key) -> Result<Delivery> {
        self.stat(key).await?;
        if self.root == Path::new("/library") {
            Ok(Delivery::NginxInternal(format!(
                "/_media/{}",
                encode_path(key.as_str())
            )))
        } else {
            Ok(Delivery::Stream)
        }
    }

    fn local_path(&self, key: &Key) -> Option<PathBuf> {
        self.resolve(key).ok()
    }
}

fn meta_from_metadata(metadata: &std::fs::Metadata) -> Meta {
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        });
    let size = metadata.len();

    Meta {
        size,
        etag: format!("{modified_ms}-{size}"),
        modified_ms,
    }
}

fn encode_path(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(char::from(byte));
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }

    encoded
}

fn io_error(action: &str, path: &Path, error: std::io::Error) -> CoreError {
    if error.kind() == ErrorKind::NotFound {
        CoreError::not_found("object", path.display())
    } else {
        CoreError::Unavailable(format!("{action} {}: {error}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::meta_from_metadata;

    #[test]
    fn metadata_maps_to_etag() {
        let path = std::env::temp_dir().join(format!(
            "mc-storage-meta-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"hello").unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        let meta = meta_from_metadata(&metadata);

        assert_eq!(meta.size, 5);
        assert!(meta.etag.ends_with("-5"));

        std::fs::remove_file(path).unwrap();
    }
}
