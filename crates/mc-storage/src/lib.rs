mod key;
mod local;
mod memory;
mod s3;

use std::ops::Range;
use std::path::PathBuf;

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::{BoxStream, StreamExt};
use mc_core::{Result, StorageId};
use serde::{Deserialize, Serialize};

pub use key::Key;
pub use local::LocalStorage;
pub use memory::MemoryStorage;
pub use s3::{S3Config, S3Storage};

pub type EntryStream<'a> = BoxStream<'a, Result<Entry>>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Caps {
    pub read: bool,
    pub write: bool,
    pub watch: bool,
    pub presign: bool,
    pub rename: bool,
    pub local_path: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Meta {
    pub size: u64,
    pub etag: String,
    pub modified_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub key: Key,
    pub meta: Meta,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Delivery {
    NginxInternal(String),
    Presigned(String),
    Stream,
}

#[async_trait]
pub trait Storage: Send + Sync {
    fn id(&self) -> StorageId;
    fn caps(&self) -> Caps;
    fn list(&self, prefix: &Key) -> EntryStream<'_>;
    async fn stat(&self, key: &Key) -> Result<Meta>;
    async fn read_range(&self, key: &Key, range: Range<u64>) -> Result<Bytes>;
    async fn read(&self, key: &Key) -> Result<Bytes>;
    async fn write(&self, key: &Key, data: Bytes) -> Result<Meta>;
    async fn write_stream(
        &self,
        key: &Key,
        mut stream: BoxStream<'static, Result<Bytes>>,
    ) -> Result<Meta> {
        let mut data = Vec::new();
        while let Some(chunk) = stream.next().await {
            data.extend_from_slice(&chunk?);
        }
        self.write(key, Bytes::from(data)).await
    }
    async fn rename(&self, from: &Key, to: &Key) -> Result<()>;
    async fn delete(&self, key: &Key) -> Result<()>;
    async fn delivery(&self, key: &Key) -> Result<Delivery>;
    fn local_path(&self, _key: &Key) -> Option<PathBuf> {
        None
    }
}
