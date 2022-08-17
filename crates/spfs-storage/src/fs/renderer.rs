// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tokio::io::AsyncReadExt;

use super::FSRepository;
use crate::{
    encoding::{self, Encodable},
    runtime::makedirs_with_perms,
    storage::{ManifestViewer, PayloadStorage, Repository},
    tracking, Error, Result,
};

#[cfg(test)]
#[path = "./renderer_test.rs"]
mod renderer_test;

pub enum RenderType {
    HardLink,
    Copy,
}

#[async_trait::async_trait]
impl ManifestViewer for FSRepository {
    async fn has_rendered_manifest(&self, digest: encoding::Digest) -> bool {
        let renders = match &self.renders {
            Some(renders) => renders,
            None => return false,
        };
        let rendered_dir = renders.build_digest_path(&digest);
        was_render_completed(&rendered_dir)
    }

    /// Return the path that the manifest would be rendered to.
    fn manifest_render_path(&self, manifest: &crate::graph::Manifest) -> Result<PathBuf> {
        Ok(self
            .get_render_storage()?
            .build_digest_path(&manifest.digest()?))
    }

    /// Create a hard-linked rendering of the given file manifest.
    ///
    /// # Errors:
    /// - if any of the blobs in the manifest are not available in this repo.
    async fn render_manifest(&self, manifest: &crate::graph::Manifest) -> Result<PathBuf> {
        let renders = self.get_render_storage()?;
        let rendered_dirpath = renders.build_digest_path(&manifest.digest()?);
        if was_render_completed(&rendered_dirpath) {
            tracing::trace!(path = ?rendered_dirpath, "render already completed");
            return Ok(rendered_dirpath);
        }
        tracing::trace!(path = ?rendered_dirpath, "rendering manifest...");

        let uuid = uuid::Uuid::new_v4().to_string();
        let working_dir = renders.workdir().join(uuid);
        makedirs_with_perms(&working_dir, 0o777)?;

        self.render_manifest_into_dir(manifest, &working_dir, RenderType::HardLink)
            .await?;

        renders.ensure_base_dir(&rendered_dirpath)?;
        match tokio::fs::rename(&working_dir, &rendered_dirpath).await {
            Ok(_) => (),
            Err(err) => match err.kind() {
                std::io::ErrorKind::AlreadyExists => {
                    if let Err(err) = open_perms_and_remove_all(&working_dir).await {
                        tracing::warn!(path=?working_dir, "failed to clean up working directory: {:?}", err);
                    }
                }
                _ => return Err(Error::StorageWriteError(rendered_dirpath, err)),
            },
        }

        mark_render_completed(&rendered_dirpath).await?;
        Ok(rendered_dirpath)
    }

    /// Remove the identified render from this storage.
    async fn remove_rendered_manifest(&self, digest: crate::encoding::Digest) -> Result<()> {
        let renders = match &self.renders {
            Some(renders) => renders,
            None => return Ok(()),
        };
        let rendered_dirpath = renders.build_digest_path(&digest);
        let uuid = uuid::Uuid::new_v4().to_string();
        let working_dirpath = renders.workdir().join(uuid);
        renders.ensure_base_dir(&working_dirpath)?;
        if let Err(err) = tokio::fs::rename(&rendered_dirpath, &working_dirpath).await {
            return match err.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(crate::Error::StorageWriteError(working_dirpath, err)),
            };
        }

        unmark_render_completed(&rendered_dirpath).await?;
        open_perms_and_remove_all(&working_dirpath).await
    }
}

impl FSRepository {
    fn get_render_storage(&self) -> Result<&super::FSHashStore> {
        match &self.renders {
            Some(renders) => Ok(renders),
            None => Err(Error::NoRenderStorage(self.address())),
        }
    }

    pub async fn render_manifest_into_dir(
        &self,
        manifest: &crate::graph::Manifest,
        target_dir: impl AsRef<Path>,
        render_type: RenderType,
    ) -> Result<()> {
        let walkable = manifest.unlock();
        let entries: Vec<_> = walkable
            .walk_abs(&target_dir.as_ref().to_string_lossy())
            .collect();
        // we used to get CAP_FOWNER here, but with async
        // it can no longer guarantee anything useful
        // (the process can happen in other threads, and
        // other code can run in the current thread)
        for node in entries.iter() {
            let res = match node.entry.kind {
                tracking::EntryKind::Tree => {
                    let path_to_create = node.path.to_path("/");
                    tokio::fs::create_dir_all(&path_to_create)
                        .await
                        .map_err(|err| Error::StorageWriteError(path_to_create, err))
                }
                tracking::EntryKind::Mask => continue,
                tracking::EntryKind::Blob => {
                    self.render_blob(node.path.to_path("/"), node.entry, &render_type)
                        .await
                }
            };
            if let Err(err) = res {
                return Err(err.wrap(format!("Failed to render [{}]", node.path)));
            }
        }

        for node in entries.iter().rev() {
            if node.entry.kind.is_mask() {
                continue;
            }
            if node.entry.is_symlink() {
                continue;
            }
            let path_to_change = node.path.to_path("/");
            if let Err(err) = tokio::fs::set_permissions(
                &path_to_change,
                std::fs::Permissions::from_mode(node.entry.mode),
            )
            .await
            {
                return Err(Error::StorageWriteError(path_to_change, err));
            }
        }

        Ok(())
    }

    async fn render_blob<P: AsRef<Path>>(
        &self,
        rendered_path: P,
        entry: &tracking::Entry,
        render_type: &RenderType,
    ) -> Result<()> {
        if entry.is_symlink() {
            let (mut reader, filename) = self.open_payload(entry.object).await?;
            let mut target = String::new();
            reader
                .read_to_string(&mut target)
                .await
                .map_err(|err| Error::StorageReadError(filename, err))?;
            return if let Err(err) = std::os::unix::fs::symlink(&target, &rendered_path) {
                match err.kind() {
                    std::io::ErrorKind::AlreadyExists => Ok(()),
                    _ => Err(Error::StorageWriteError(
                        rendered_path.as_ref().to_owned(),
                        err,
                    )),
                }
            } else {
                Ok(())
            };
        }
        let committed_path = self.payloads.build_digest_path(&entry.object);
        match render_type {
            RenderType::HardLink => {
                if let Err(err) = tokio::fs::hard_link(&committed_path, &rendered_path).await {
                    match err.kind() {
                        std::io::ErrorKind::AlreadyExists => (),
                        _ => {
                            return Err(Error::StorageWriteError(
                                rendered_path.as_ref().to_owned(),
                                err,
                            ))
                        }
                    }
                }
            }
            RenderType::Copy => {
                if let Err(err) = tokio::fs::copy(&committed_path, &rendered_path).await {
                    match err.kind() {
                        std::io::ErrorKind::AlreadyExists => (),
                        _ => {
                            return Err(Error::StorageWriteError(
                                rendered_path.as_ref().to_owned(),
                                err,
                            ))
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Walks down a filesystem tree, opening permissions on each file before removing
/// the entire tree.
///
/// This process handles the case when a folder may include files
/// that need to be removed but on which the user doesn't have enough permissions.
/// It does assume that the current user owns the file, as it may not be possible to
/// change permissions before removal otherwise.
#[async_recursion::async_recursion]
async fn open_perms_and_remove_all(root: &Path) -> Result<()> {
    let mut read_dir = tokio::fs::read_dir(&root)
        .await
        .map_err(|err| Error::StorageReadError(root.to_owned(), err))?;
    // TODO: parallelize this with async
    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|err| Error::StorageReadError(root.to_owned(), err))?
    {
        let entry_path = root.join(entry.file_name());
        let file_type = entry
            .file_type()
            .await
            .map_err(|err| Error::StorageReadError(root.to_owned(), err))?;
        let _ =
            tokio::fs::set_permissions(&entry_path, std::fs::Permissions::from_mode(0o777)).await;
        if file_type.is_symlink() || file_type.is_file() {
            tokio::fs::remove_file(&entry_path)
                .await
                .map_err(|err| Error::StorageWriteError(entry_path.clone(), err))?;
        }
        if file_type.is_dir() {
            open_perms_and_remove_all(&entry_path).await?;
        }
    }
    tokio::fs::remove_dir(&root)
        .await
        .map_err(|err| Error::StorageWriteError(root.to_owned(), err))?;
    Ok(())
}

fn was_render_completed<P: AsRef<Path>>(render_path: P) -> bool {
    let mut name = render_path
        .as_ref()
        .file_name()
        .expect("must have a file name")
        .to_os_string();
    name.push(".completed");
    let marker_path = render_path.as_ref().with_file_name(name);
    marker_path.exists()
}

/// panics if the given path does not have a directory name
async fn mark_render_completed<P: AsRef<Path>>(render_path: P) -> Result<()> {
    let mut name = render_path
        .as_ref()
        .file_name()
        .expect("must have a file name")
        .to_os_string();
    name.push(".completed");
    let marker_path = render_path.as_ref().with_file_name(name);
    // create if it doesn't exist but don't fail if it already exists (no exclusive open)
    tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&marker_path)
        .await
        .map_err(|err| Error::StorageWriteError(marker_path, err))?;
    Ok(())
}

async fn unmark_render_completed<P: AsRef<Path>>(render_path: P) -> Result<()> {
    let mut name = render_path
        .as_ref()
        .file_name()
        .expect("must have a file name")
        .to_os_string();
    name.push(".completed");
    let marker_path = render_path.as_ref().with_file_name(name);
    if let Err(err) = tokio::fs::remove_file(&marker_path).await {
        match err.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(Error::StorageWriteError(marker_path, err)),
        }
    } else {
        Ok(())
    }
}
