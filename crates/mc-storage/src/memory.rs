use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::StreamExt;
use mc_core::{CoreError, Result, StorageId};

use crate::{Caps, Delivery, Entry, EntryStream, Key, Meta, Storage};

#[derive(Clone, Debug)]
struct Object {
    data: Bytes,
    modified_ms: u64,
}

#[derive(Debug)]
pub struct MemoryStorage {
    id: StorageId,
    objects: RwLock<BTreeMap<String, Object>>,
}

impl MemoryStorage {
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: StorageId::new(),
            objects: RwLock::new(BTreeMap::new()),
        }
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            })
    }

    fn meta(object: &Object) -> Meta {
        Meta {
            size: object.data.len() as u64,
            etag: format!("{}-{}", object.modified_ms, object.data.len()),
            modified_ms: object.modified_ms,
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Storage for MemoryStorage {
    fn id(&self) -> StorageId {
        self.id
    }

    fn caps(&self) -> Caps {
        Caps {
            read: true,
            write: true,
            watch: false,
            presign: false,
            rename: true,
            local_path: false,
        }
    }

    fn list(&self, prefix: &Key) -> EntryStream<'_> {
        let prefix = prefix.as_str();
        let objects = self
            .objects
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let entries = objects
            .iter()
            .filter(|(key, _)| {
                prefix.is_empty()
                    || key.as_str() == prefix
                    || key.as_str().starts_with(&format!("{prefix}/"))
            })
            .map(|(key, object)| {
                Ok(Entry {
                    key: Key::new(key.clone())?,
                    meta: Self::meta(object),
                })
            })
            .collect::<Vec<_>>();

        futures_util::stream::iter(entries).boxed()
    }

    async fn stat(&self, key: &Key) -> Result<Meta> {
        let objects = self
            .objects
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let object = objects
            .get(key.as_str())
            .ok_or_else(|| CoreError::not_found("object", key))?;
        Ok(Self::meta(object))
    }

    async fn read_range(&self, key: &Key, range: Range<u64>) -> Result<Bytes> {
        if range.start > range.end {
            return Err(CoreError::Validation(format!(
                "invalid byte range {}..{}",
                range.start, range.end
            )));
        }

        let objects = self
            .objects
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let object = objects
            .get(key.as_str())
            .ok_or_else(|| CoreError::not_found("object", key))?;

        let start = usize::try_from(range.start).unwrap_or(usize::MAX);
        let end = usize::try_from(range.end).unwrap_or(usize::MAX);
        let start = start.min(object.data.len());
        let end = end.min(object.data.len());

        Ok(object.data.slice(start..end))
    }

    async fn read(&self, key: &Key) -> Result<Bytes> {
        let objects = self
            .objects
            .read()
            .unwrap_or_else(|error| error.into_inner());
        let object = objects
            .get(key.as_str())
            .ok_or_else(|| CoreError::not_found("object", key))?;
        Ok(object.data.clone())
    }

    async fn write(&self, key: &Key, data: Bytes) -> Result<Meta> {
        let mut objects = self
            .objects
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let object = Object {
            data,
            modified_ms: Self::now_ms(),
        };
        let meta = Self::meta(&object);
        objects.insert(key.as_str().to_owned(), object);
        Ok(meta)
    }

    async fn rename(&self, from: &Key, to: &Key) -> Result<()> {
        let mut objects = self
            .objects
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let mut object = objects
            .remove(from.as_str())
            .ok_or_else(|| CoreError::not_found("object", from))?;
        object.modified_ms = Self::now_ms();
        objects.insert(to.as_str().to_owned(), object);
        Ok(())
    }

    async fn delete(&self, key: &Key) -> Result<()> {
        let mut objects = self
            .objects
            .write()
            .unwrap_or_else(|error| error.into_inner());
        objects
            .remove(key.as_str())
            .map(|_| ())
            .ok_or_else(|| CoreError::not_found("object", key))
    }

    async fn delivery(&self, key: &Key) -> Result<Delivery> {
        self.stat(key).await?;
        Ok(Delivery::Stream)
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::MemoryStorage;
    use crate::{Key, Storage};

    #[tokio::test]
    async fn roundtrips_objects() {
        let storage = MemoryStorage::new();
        let key = Key::new("a/b.txt").unwrap();

        storage
            .write(&key, Bytes::from_static(b"hello"))
            .await
            .unwrap();
        assert_eq!(
            storage.read(&key).await.unwrap(),
            Bytes::from_static(b"hello")
        );

        storage.delete(&key).await.unwrap();
        assert!(storage.stat(&key).await.is_err());
    }
}
