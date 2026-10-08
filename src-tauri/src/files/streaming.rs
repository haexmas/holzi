//! Where the media server reads a file from (spec 044 FR-012, FR-013, research R4; after haex-vault
//! `remote_storage/streaming/source.rs`): its size, its media type and a reader for one byte range.
//! The server copies that reader through a fixed buffer, so no file is ever held whole in memory.
//! Storages (US5) add a source that reads ranges from S3.

use std::io::SeekFrom;
use std::path::PathBuf;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt};

/// A file the media server can serve.
#[async_trait]
pub trait StreamingSource: Send + Sync {
    /// Its size in bytes.
    async fn size(&self) -> std::io::Result<u64>;
    /// A reader for `len` bytes from `start`.
    async fn open_range(
        &self,
        start: u64,
        len: u64,
    ) -> std::io::Result<Box<dyn AsyncRead + Send + Unpin>>;
    /// Its media type.
    fn content_type(&self) -> &str;
}

/// A file on this device.
pub struct LocalFileSource {
    path: PathBuf,
    content_type: String,
}

impl LocalFileSource {
    pub fn new(path: PathBuf, content_type: impl Into<String>) -> Self {
        Self {
            path,
            content_type: content_type.into(),
        }
    }
}

#[async_trait]
impl StreamingSource for LocalFileSource {
    async fn size(&self) -> std::io::Result<u64> {
        Ok(tokio::fs::metadata(&self.path).await?.len())
    }

    async fn open_range(
        &self,
        start: u64,
        len: u64,
    ) -> std::io::Result<Box<dyn AsyncRead + Send + Unpin>> {
        let mut file = tokio::fs::File::open(&self.path).await?;
        file.seek(SeekFrom::Start(start)).await?;
        Ok(Box::new(file.take(len)))
    }

    fn content_type(&self) -> &str {
        &self.content_type
    }
}
