// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::pin::Pin;

use futures::Stream;

use crate::encoding;
use crate::Result;

#[cfg(test)]
#[path = "payload_test.rs"]
mod payload_test;

/// Stores arbitrary binary data payloads using their content digest.
#[async_trait::async_trait]
pub trait PayloadStorage: Sync + Send {
    /// Iterate all the payloads in this storage.
    fn iter_payload_digests(&self) -> Pin<Box<dyn Stream<Item = Result<encoding::Digest>> + Send>>;

    /// Return true if the identified payload exists.
    async fn has_payload(&self, digest: encoding::Digest) -> bool {
        self.open_payload(digest).await.is_ok()
    }

    /// Store the contents of the given stream, returning its digest and size
    ///
    /// # Safety
    ///
    /// It is unsafe to write payload data without also creating a blob
    /// to track that payload in the database. Usually, its better to
    /// call [`super::Repository::commit_blob`] instead.
    async unsafe fn write_data(
        &self,
        reader: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync + 'static>>,
    ) -> Result<(encoding::Digest, u64)>;

    /// Return a handle and filename to the full content of a payload.
    ///
    /// # Errors:
    /// - [`crate::Error::UnknownObject`]: if the payload does not exist in this storage
    async fn open_payload(
        &self,
        digest: encoding::Digest,
    ) -> Result<(
        Pin<Box<dyn tokio::io::AsyncRead + Send + Sync + 'static>>,
        std::path::PathBuf,
    )>;

    /// Remove the payload identified by the given digest.
    ///
    /// Errors:
    /// - [`crate::Error::UnknownObject`]: if the payload does not exist in this storage
    async fn remove_payload(&self, digest: encoding::Digest) -> Result<()>;
}

#[async_trait::async_trait]
impl<T: PayloadStorage> PayloadStorage for &T {
    fn iter_payload_digests(&self) -> Pin<Box<dyn Stream<Item = Result<encoding::Digest>> + Send>> {
        PayloadStorage::iter_payload_digests(&**self)
    }

    async unsafe fn write_data(
        &self,
        reader: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync + 'static>>,
    ) -> Result<(encoding::Digest, u64)> {
        // Safety: we are wrapping the same underlying unsafe function and
        // so the same safety holds for our callers
        unsafe { PayloadStorage::write_data(&**self, reader).await }
    }

    async fn open_payload(
        &self,
        digest: encoding::Digest,
    ) -> Result<(
        Pin<Box<dyn tokio::io::AsyncRead + Send + Sync + 'static>>,
        std::path::PathBuf,
    )> {
        PayloadStorage::open_payload(&**self, digest).await
    }

    async fn remove_payload(&self, digest: encoding::Digest) -> Result<()> {
        PayloadStorage::remove_payload(&**self, digest).await
    }
}
