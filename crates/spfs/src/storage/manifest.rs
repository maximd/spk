// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::pin::Pin;

use chrono::{DateTime, Utc};
use futures::stream::Stream;
use tokio_stream::StreamExt;

use crate::{encoding, graph, Result};

#[cfg(test)]
#[path = "./manifest_test.rs"]
mod manifest_test;

pub type ManifestStreamItem = Result<(encoding::Digest, graph::Manifest)>;

#[async_trait::async_trait]
pub trait ManifestStorage: graph::Database + Sync + Send {
    /// Iterate the objects in this storage which are manifests.
    fn iter_manifests<'db>(&'db self) -> Pin<Box<dyn Stream<Item = ManifestStreamItem> + 'db>> {
        use graph::Object;
        let stream = self.iter_objects().filter_map(|res| match res {
            Ok((digest, obj)) => match obj {
                Object::Manifest(manifest) => Some(Ok((digest, manifest))),
                _ => None,
            },
            Err(err) => Some(Err(err)),
        });
        Box::pin(stream)
    }

    /// Return true if the identified manifest exists in this storage.
    async fn has_manifest(&self, digest: encoding::Digest) -> bool {
        self.read_manifest(digest).await.is_ok()
    }

    /// Return the manifest identified by the given digest.
    async fn read_manifest(&self, digest: encoding::Digest) -> Result<graph::Manifest> {
        use graph::Object;
        match self.read_object(digest).await {
            Err(err) => Err(err),
            Ok(Object::Manifest(manifest)) => Ok(manifest),
            Ok(_) => Err(format!("Object is not a manifest: {:?}", digest).into()),
        }
    }
}

impl<T: ManifestStorage> ManifestStorage for &T {}

#[async_trait::async_trait]
pub trait ManifestViewer: Send + Sync {
    /// Returns true if the identified manifest has been rendered already
    async fn has_rendered_manifest(&self, digest: encoding::Digest) -> bool;

    /// Iterate the manifests that have been rendered.
    fn iter_rendered_manifests<'db>(
        &'db self,
    ) -> Pin<Box<dyn Stream<Item = Result<encoding::Digest>> + 'db>>;

    /// Returns what would be used as the local path to the root of the rendered manifest.
    ///
    /// This path does not necessarily exist or contain a valid render.
    fn manifest_render_path(&self, manifest: &graph::Manifest) -> Result<std::path::PathBuf>;

    /// Returns the location of the render proxy path
    fn proxy_path(&self) -> Option<&std::path::Path>;

    /// Create a rendered view of the given manifest on the local disk.
    ///
    /// Returns the local path to the root of the rendered manifest
    async fn render_manifest(
        &self,
        manifest: &graph::Manifest,
        pull_from: Option<&crate::storage::RepositoryHandle>,
    ) -> Result<std::path::PathBuf>;

    /// Cleanup a previously rendered manifest from the local disk.
    async fn remove_rendered_manifest(&self, digest: encoding::Digest) -> Result<()>;

    /// Cleanup a previously rendered manifest from the local disk, if it is
    /// older than a threshold.
    async fn remove_rendered_manifest_if_older_than(
        &self,
        older_than: DateTime<Utc>,
        digest: encoding::Digest,
    ) -> Result<()>;
}
