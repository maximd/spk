// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::path::Path;
use std::pin::Pin;

use super::status::remount_runtime;
use crate::prelude::*;
use crate::tracking::{BlobHasher, BlobRead, ManifestBuilder, PathFilter};
use crate::{encoding, graph, runtime, storage, tracking, Error, Result};

#[cfg(test)]
#[path = "./commit_test.rs"]
mod commit_test;

/// Hashes blob data in-memory.
///
/// Used in conjunction with the [`Committer`], this hasher
/// can reduce io overhead when committing large and incremental
/// manifests, or when committing to a remote repository. It has
/// the benefit of not needing to write anything to the repository
/// that already exists, but in a worst-case scenario will require
/// reading the local files twice (once for hashing and once to copy
/// into the repository)
pub struct InMemoryBlobHasher;

#[tonic::async_trait]
impl BlobHasher for InMemoryBlobHasher {
    async fn hash_blob(&self, reader: Pin<Box<dyn BlobRead>>) -> Result<encoding::Digest> {
        Ok(encoding::Digest::from_async_reader(reader).await?)
    }
}

/// Hashes payload data as it's being written to a repository.
///
/// Used in conjunction with the [`Committer`], this can reduce
/// io overhead by ensuring that each file only needs to be read
/// through once. It can also greatly decrease the commit speed
/// by requiring that each file is written to the repository even
/// if the payload already exists.
pub struct WriteToRepositoryBlobHasher<'repo> {
    repo: &'repo RepositoryHandle,
}

#[tonic::async_trait]
impl<'repo> BlobHasher for WriteToRepositoryBlobHasher<'repo> {
    async fn hash_blob(&self, reader: Pin<Box<dyn BlobRead>>) -> Result<encoding::Digest> {
        self.repo.commit_blob(reader).await
    }
}

/// Manages the process of committing files to a repository
pub struct Committer<'repo, H = WriteToRepositoryBlobHasher<'repo>, F = ()>
where
    H: BlobHasher + Send + Sync,
    F: PathFilter + Send + Sync,
{
    repo: &'repo storage::RepositoryHandle,
    builder: ManifestBuilder<H, F>,
}

impl<'repo> Committer<'repo, WriteToRepositoryBlobHasher<'repo>, ()> {
    /// Create a new committer, with the default [`WriteToRepositoryBlobHasher`].
    pub fn new(repo: &'repo storage::RepositoryHandle) -> Self {
        let builder = ManifestBuilder::new().with_blob_hasher(WriteToRepositoryBlobHasher { repo });
        Self { repo, builder }
    }
}

impl<'repo, H, F> Committer<'repo, H, F>
where
    H: BlobHasher + Send + Sync,
    F: PathFilter + Send + Sync,
{
    pub fn with_blob_hasher<H2>(self, hasher: H2) -> Committer<'repo, H2, F>
    where
        H2: BlobHasher + Send + Sync,
    {
        Committer {
            repo: self.repo,
            builder: self.builder.with_blob_hasher(hasher),
        }
    }

    /// Use this filter when committing files to storage.
    ///
    /// Only the changes/files matched by `filter` will be included.
    ///
    /// The filter is expected to match paths that are relative to the
    /// `$PREFIX` root, eg: `directory/filename` rather than
    /// `/spfs/directory/filename`.
    pub fn with_path_filter<F2>(self, filter: F2) -> Committer<'repo, H, F2>
    where
        F2: PathFilter + Send + Sync,
    {
        Committer {
            repo: self.repo,
            builder: self.builder.with_path_filter(filter),
        }
    }

    /// Commit the working file changes of a runtime to a new layer.
    pub async fn commit_layer(&self, runtime: &mut runtime::Runtime) -> Result<graph::Layer> {
        let manifest = self.commit_dir(&runtime.config.upper_dir).await?;
        self.commit_manifest(manifest, runtime).await
    }

    /// Commit a manifest of the working file changes of a runtime to a new layer.
    ///
    /// This will add the layer to the current runtime and then remount it.
    pub async fn commit_manifest(
        &self,
        manifest: tracking::Manifest,
        runtime: &mut runtime::Runtime,
    ) -> Result<graph::Layer> {
        if manifest.is_empty() {
            return Err(Error::NothingToCommit);
        }
        let layer = self
            .repo
            .create_layer(&graph::Manifest::from(&manifest))
            .await?;
        runtime.push_digest(layer.digest()?);
        runtime.status.editable = false;
        runtime.save_state_to_storage().await?;
        remount_runtime(runtime).await?;
        Ok(layer)
    }

    /// Commit the full layer stack and working files to a new platform.
    pub async fn commit_platform(&self, runtime: &mut runtime::Runtime) -> Result<graph::Platform> {
        match self.commit_layer(runtime).await {
            Ok(_) | Err(Error::NothingToCommit) => (),
            Err(err) => return Err(err),
        }

        runtime.reload_state_from_storage().await?;
        if runtime.status.stack.is_empty() {
            Err(Error::NothingToCommit)
        } else {
            self.repo
                .create_platform(runtime.status.stack.clone())
                .await
        }
    }

    /// Commit a local file system directory to this storage.
    ///
    /// This collects all files to store as blobs and maintains a
    /// render of the manifest for use immediately.
    pub async fn commit_dir<P>(&self, path: P) -> Result<tracking::Manifest>
    where
        P: AsRef<Path>,
    {
        let path = tokio::fs::canonicalize(&path)
            .await
            .map_err(|err| Error::InvalidPath(path.as_ref().to_owned(), err))?;
        let manifest = {
            tracing::info!("building manifest");
            self.builder.compute_manifest(&path).await?
        };

        tracing::info!("committing manifest");
        for node in manifest.walk() {
            if !node.entry.kind.is_blob() {
                continue;
            }
            if !self.repo.has_blob(node.entry.object).await {
                let local_path = path.join(node.path.as_str());
                let created = if node.entry.is_symlink() {
                    let content = tokio::fs::read_link(&local_path)
                        .await
                        .map_err(|err| {
                            // TODO: add better message for file missing
                            Error::StorageWriteError("read link for committing", local_path, err)
                        })?
                        .into_os_string()
                        .into_string()
                        .map_err(|_| {
                            crate::Error::String(
                                "Symlinks must point to a valid utf-8 path".to_string(),
                            )
                        })?
                        .into_bytes();
                    let reader = Box::pin(tokio::io::BufReader::new(std::io::Cursor::new(content)));
                    self.repo.commit_blob(reader).await?
                } else {
                    let file = tokio::fs::File::open(&local_path).await.map_err(|err| {
                        // TODO: add better message for file missing
                        Error::StorageWriteError("open file for committing", local_path, err)
                    })?;
                    let reader = Box::pin(tokio::io::BufReader::new(file));
                    self.repo.commit_blob(reader).await?
                };
                if created != node.entry.object {
                    return Err(Error::String(format!(
                        "File contents changed on disk during commit: {}",
                        node.path
                    )));
                }
            }
        }

        let storable = graph::Manifest::from(&manifest);
        self.repo
            .write_object(&graph::Object::Manifest(storable))
            .await?;

        Ok(manifest)
    }
}
