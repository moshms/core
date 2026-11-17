use std::ops::Range;
use std::time::Duration;

use async_stream::try_stream;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::{StreamExt, TryStreamExt};
use mc_core::{CoreError, Result, StorageId};
use opendal::services::S3;
use opendal::{ErrorKind, Metadata, Operator};

use crate::{Caps, Delivery, Entry, EntryStream, Key, Meta, Storage};

#[derive(Clone, Debug)]
pub struct S3Config {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub root: String,
    pub virtual_host_style: bool,
    pub presign_seconds: u64,
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            region: "us-east-1".to_owned(),
            bucket: String::new(),
            access_key_id: String::new(),
            secret_access_key: String::new(),
            root: String::new(),
            virtual_host_style: false,
            presign_seconds: 3600,
        }
    }
}

pub struct S3Storage {
    id: StorageId,
    operator: Operator,
    presign_seconds: u64,
}

impl S3Storage {
    pub fn new(id: StorageId, config: &S3Config) -> Result<Self> {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        opendal_http_transport_reqwest::install_default();

        let mut builder = S3::default()
            .endpoint(&config.endpoint)
            .region(&config.region)
            .bucket(&config.bucket)
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key)
            .root(&config.root);

        if config.virtual_host_style {
            builder = builder.enable_virtual_host_style();
        }

        let operator = Operator::new(builder)
            .map_err(|error| CoreError::Validation(format!("s3 config: {error}")))?;

        Ok(Self {
            id,
            operator,
            presign_seconds: config.presign_seconds.max(60),
        })
    }
}

#[async_trait]
impl Storage for S3Storage {
    fn id(&self) -> StorageId {
        self.id
    }

    fn caps(&self) -> Caps {
        Caps {
            read: true,
            write: true,
            watch: false,
            presign: true,
            rename: true,
            local_path: false,
        }
    }

    fn list(&self, prefix: &Key) -> EntryStream<'_> {
        let operator = self.operator.clone();
        let prefix = prefix.as_str().to_owned();

        Box::pin(try_stream! {
            let mut lister = operator
                .lister_with(&prefix)
                .recursive(true)
                .await
                .map_err(|error| opendal_error("list objects", error))?;

            while let Some(entry) = lister
                .try_next()
                .await
                .map_err(|error| opendal_error("list objects", error))?
            {
                if !entry.metadata().is_file() {
                    continue;
                }

                yield Entry {
                    key: Key::new(entry.path())?,
                    meta: meta_from_opendal(entry.metadata()),
                };
            }
        })
    }

    async fn stat(&self, key: &Key) -> Result<Meta> {
        let metadata = self
            .operator
            .stat(key.as_str())
            .await
            .map_err(|error| opendal_error("stat object", error))?;

        Ok(meta_from_opendal(&metadata))
    }

    async fn read_range(&self, key: &Key, range: Range<u64>) -> Result<Bytes> {
        if range.start > range.end {
            return Err(CoreError::Validation(format!(
                "invalid byte range {}..{}",
                range.start, range.end
            )));
        }

        let buffer = self
            .operator
            .read_with(key.as_str())
            .range(range)
            .await
            .map_err(|error| opendal_error("read object range", error))?;

        Ok(buffer.to_bytes())
    }

    async fn read(&self, key: &Key) -> Result<Bytes> {
        let buffer = self
            .operator
            .read(key.as_str())
            .await
            .map_err(|error| opendal_error("read object", error))?;

        Ok(buffer.to_bytes())
    }

    async fn write(&self, key: &Key, data: Bytes) -> Result<Meta> {
        self.operator
            .write(key.as_str(), data.to_vec())
            .await
            .map_err(|error| opendal_error("write object", error))?;

        self.stat(key).await
    }

    async fn write_stream(
        &self,
        key: &Key,
        mut stream: futures_util::stream::BoxStream<'static, Result<Bytes>>,
    ) -> Result<Meta> {
        let mut writer = self
            .operator
            .writer(key.as_str())
            .await
            .map_err(|error| opendal_error("open object writer", error))?;
        while let Some(chunk) = stream.next().await {
            writer
                .write(chunk?)
                .await
                .map_err(|error| opendal_error("write object stream", error))?;
        }
        let metadata = writer
            .close()
            .await
            .map_err(|error| opendal_error("close object writer", error))?;

        Ok(meta_from_opendal(&metadata))
    }

    async fn rename(&self, from: &Key, to: &Key) -> Result<()> {
        self.operator
            .copy(from.as_str(), to.as_str())
            .await
            .map_err(|error| opendal_error("copy object", error))?;
        self.operator
            .delete(from.as_str())
            .await
            .map_err(|error| opendal_error("delete renamed object", error))
    }

    async fn delete(&self, key: &Key) -> Result<()> {
        self.stat(key).await?;
        self.operator
            .delete(key.as_str())
            .await
            .map_err(|error| opendal_error("delete object", error))
    }

    async fn delivery(&self, key: &Key) -> Result<Delivery> {
        let request = self
            .operator
            .presign_read_with(key.as_str(), Duration::from_secs(self.presign_seconds))
            .await
            .map_err(|error| opendal_error("presign object", error))?;

        Ok(Delivery::Presigned(request.uri().to_string()))
    }
}

fn meta_from_opendal(metadata: &Metadata) -> Meta {
    let size = metadata.content_length();
    let modified_ms = metadata
        .last_modified()
        .map(|timestamp| timestamp.into_inner().as_millisecond())
        .and_then(|millis| u64::try_from(millis).ok())
        .unwrap_or(0);
    let etag = metadata
        .etag()
        .map(|etag| etag.trim_matches('"').to_owned())
        .filter(|etag| !etag.is_empty())
        .unwrap_or_else(|| format!("{modified_ms}-{size}"));

    Meta {
        size,
        etag,
        modified_ms,
    }
}

fn opendal_error(context: &str, error: opendal::Error) -> CoreError {
    if error.kind() == ErrorKind::NotFound {
        CoreError::not_found("object", context)
    } else {
        CoreError::Unavailable(format!("{context}: {error}"))
    }
}
