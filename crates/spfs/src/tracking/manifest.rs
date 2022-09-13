// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::os::unix::fs::MetadataExt;
use std::os::unix::prelude::FileTypeExt;
use std::pin::Pin;
use std::sync::Arc;

use futures::stream::FuturesUnordered;
use futures::TryStreamExt;
use itertools::Itertools;
use relative_path::RelativePathBuf;
use tokio::fs::DirEntry;

use super::entry::{Entry, EntryKind};
use crate::encoding;
use crate::filesystem;
use crate::{Error, Result};

#[cfg(test)]
#[path = "./manifest_test.rs"]
mod manifest_test;

#[derive(Default, Debug, Eq, PartialEq, Clone)]
pub struct Manifest {
    root: Entry,
}

impl Manifest {
    pub fn new(root: Entry) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Entry {
        &self.root
    }

    pub fn take_root(self) -> Entry {
        self.root
    }

    /// Return true if this manifest has no contents.
    pub fn is_empty(&self) -> bool {
        self.root.entries.len() == 0
    }

    /// Get an entry in this manifest given it's filepath.
    pub fn get_path<P: AsRef<str>>(&self, path: P) -> Option<&Entry> {
        const TRIM_START: &[char] = &['/', '.'];
        const TRIM_END: &[char] = &['/'];
        let path = path
            .as_ref()
            .trim_start_matches(TRIM_START)
            .trim_end_matches(TRIM_END);
        let mut entry = &self.root;
        if path.is_empty() {
            return Some(entry);
        }
        for step in path.split('/') {
            if let EntryKind::Tree = entry.kind {
                let next = entry.entries.get(step);
                entry = match next {
                    Some(entry) => entry,
                    None => return None,
                };
            } else {
                return None;
            }
        }

        Some(entry)
    }

    /// List the contents of a directory in this manifest.
    ///
    /// None is returned if the directory does not exist or the provided entry is
    /// not a directory
    pub fn list_dir(&self, path: &str) -> Option<Vec<String>> {
        let entry = self.get_path(path)?;
        match entry.kind {
            EntryKind::Tree => Some(entry.entries.keys().cloned().collect()),
            _ => None,
        }
    }

    /// Walk the contents of this manifest top-down and depth-first.
    pub fn walk(&self) -> ManifestWalker<'_> {
        ManifestWalker::new(&self.root)
    }

    /// Same as walk(), but joins all entry paths to the given root.
    pub fn walk_abs<P: Into<RelativePathBuf>>(&self, root: P) -> ManifestWalker<'_> {
        self.walk().with_prefix(root)
    }

    /// Add a new directory entry to this manifest
    pub fn mkdir<P: AsRef<str>>(&mut self, path: P) -> Result<&mut Entry> {
        let entry = Entry::default();
        self.mknod(path, entry)
    }

    /// Ensure that all levels of the given directory name exist.
    ///
    /// Entries that do not exist are created with a reasonable default
    /// file mode, but can and should be replaced by a new entry in the
    /// case where this is not desired.
    pub fn mkdirs<P: AsRef<str>>(&mut self, path: P) -> Result<&mut Entry> {
        const TRIM_PAT: &[char] = &['/', '.'];
        let path = path.as_ref().trim_start_matches(TRIM_PAT);
        if path.is_empty() {
            return Err(nix::errno::Errno::EEXIST.into());
        }
        let path = RelativePathBuf::from(path).normalize();
        let mut entry = &mut self.root;
        for step in path.components() {
            match step {
                relative_path::Component::Normal(step) => {
                    let entries = &mut entry.entries;
                    if entries.get_mut(step).is_none() {
                        entries.insert(step.to_string(), Entry::default());
                    }
                    entry = entries.get_mut(step).unwrap();
                    if !entry.kind.is_tree() {
                        return Err(nix::errno::Errno::ENOTDIR.into());
                    }
                }
                // do not expect any other components after normalizing
                _ => continue,
            }
        }
        Ok(entry)
    }

    /// Make a new file entry in this manifest
    pub fn mkfile<'m>(&'m mut self, path: &str) -> Result<&'m mut Entry> {
        let entry = Entry {
            kind: EntryKind::Blob,
            ..Default::default()
        };
        self.mknod(path, entry)
    }

    pub fn mknod<P: AsRef<str>>(&mut self, path: P, new_entry: Entry) -> Result<&mut Entry> {
        use relative_path::Component;
        const TRIM_PAT: &[char] = &['/', '.'];

        let path = path.as_ref().trim_start_matches(TRIM_PAT);
        if path.is_empty() {
            return Err(nix::errno::Errno::EEXIST.into());
        }
        let path = RelativePathBuf::from(path).normalize();
        let mut entry = &mut self.root;
        let mut components = path.components();
        let last = components.next_back();
        for step in components {
            match step {
                Component::Normal(step) => match entry.entries.get_mut(step) {
                    None => {
                        return Err(nix::errno::Errno::ENOENT.into());
                    }
                    Some(e) => {
                        if !e.kind.is_tree() {
                            return Err(nix::errno::Errno::ENOTDIR.into());
                        }
                        entry = e;
                    }
                },
                // do not expect any other components after normalizing
                _ => continue,
            }
        }
        match last {
            None => Err(nix::errno::Errno::ENOENT.into()),
            Some(Component::Normal(step)) => {
                entry.entries.insert(step.to_string(), new_entry);
                Ok(entry.entries.get_mut(step).unwrap())
            }
            _ => Err(nix::errno::Errno::EIO.into()),
        }
    }

    /// Layer another manifest on top of this one
    pub fn update(&mut self, other: &Self) {
        self.root.update(&other.root)
    }
}

/// Walks all entries in a manifest depth-first
pub struct ManifestWalker<'m> {
    prefix: RelativePathBuf,
    children: std::collections::hash_map::Iter<'m, String, Entry>,
    active_child: Option<Box<ManifestWalker<'m>>>,
}

impl<'m> ManifestWalker<'m> {
    fn new(root: &'m Entry) -> Self {
        ManifestWalker {
            prefix: RelativePathBuf::from("/"),
            children: root.entries.iter(),
            active_child: None,
        }
    }

    fn with_prefix<P: Into<RelativePathBuf>>(mut self, prefix: P) -> Self {
        self.prefix = prefix.into();
        self
    }
}

impl<'m> Iterator for ManifestWalker<'m> {
    type Item = ManifestNode<'m>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(active_child) = self.active_child.as_mut() {
            match active_child.next() {
                Some(next) => return Some(next),
                None => {
                    self.active_child = None;
                }
            }
        }

        match self.children.next() {
            None => None,
            Some((name, child)) => {
                if child.kind.is_tree() {
                    self.active_child = Some(
                        ManifestWalker::new(child)
                            .with_prefix(&self.prefix.join(name))
                            .into(),
                    );
                }
                Some(ManifestNode {
                    path: self.prefix.join(name),
                    entry: child,
                })
            }
        }
    }
}

struct DigestFromAsyncReader {}

#[tonic::async_trait]
impl ManifestBuilderHasher for DigestFromAsyncReader {
    async fn hasher(
        &self,
        reader: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync + 'static>>,
    ) -> Result<encoding::Digest> {
        Ok(encoding::Digest::from_async_reader(reader).await?)
    }
}

pub async fn compute_manifest<P: AsRef<std::path::Path> + Send>(path: P) -> Result<Manifest> {
    let builder = ManifestBuilder::new(DigestFromAsyncReader {});
    builder.compute_manifest(path).await
}

#[async_trait::async_trait]
pub trait ManifestBuilderHasher {
    async fn hasher(
        &self,
        reader: Pin<Box<dyn tokio::io::AsyncRead + Send + Sync + 'static>>,
    ) -> Result<encoding::Digest>;
}

pub struct ManifestBuilder<H>
where
    H: ManifestBuilderHasher + Send + Sync + 'static,
{
    hasher: H,
}

impl<H> ManifestBuilder<H>
where
    H: ManifestBuilderHasher + Send + Sync + 'static,
{
    pub fn new(hasher: H) -> Self {
        Self { hasher }
    }

    /// Build a manifest that describes a directory's contents.
    pub async fn compute_manifest<P: AsRef<std::path::Path> + Send>(
        self,
        path: P,
    ) -> Result<Manifest> {
        tracing::trace!("computing manifest for {:?}", path.as_ref());
        let mut manifest = Manifest::default();
        manifest.root = Self::compute_tree_node(Arc::new(self), path, manifest.root).await?;
        Ok(manifest)
    }

    #[async_recursion::async_recursion]
    async fn compute_tree_node<P: AsRef<std::path::Path> + Send>(
        mb: Arc<ManifestBuilder<H>>,
        dirname: P,
        mut tree_node: Entry,
    ) -> Result<Entry> {
        tree_node.kind = EntryKind::Tree;
        let base = dirname.as_ref();
        let mut read_dir = tokio::fs::read_dir(base)
            .await
            .map_err(|err| Error::StorageReadError(base.to_owned(), err))?;
        let mut futures = FuturesUnordered::new();
        while let Some(dir_entry) = read_dir
            .next_entry()
            .await
            .map_err(|err| Error::StorageReadError(base.to_owned(), err))?
        {
            let dir_entry = Arc::new(dir_entry);
            let mb = Arc::clone(&mb);
            let path = base.join(dir_entry.file_name());
            let entry = {
                let dir_entry = Arc::clone(&dir_entry);
                tokio::spawn(async move {
                    (
                        Arc::clone(&dir_entry),
                        Self::compute_node(mb, path, dir_entry, Entry::default()).await,
                    )
                })
            };
            futures.push(entry);
        }
        while let Some((dir_entry, entry)) = futures.try_next().await? {
            tree_node
                .entries
                .insert(dir_entry.file_name().to_string_lossy().to_string(), entry?);
        }
        tree_node.size = tree_node.entries.len() as u64;
        Ok(tree_node)
    }

    async fn compute_node<P: AsRef<std::path::Path> + Send>(
        mb: Arc<ManifestBuilder<H>>,
        path: P,
        dir_entry: Arc<DirEntry>,
        mut entry: Entry,
    ) -> Result<Entry> {
        let stat_result = match tokio::fs::symlink_metadata(&path).await {
            Ok(r) => r,
            Err(lstat_err) if lstat_err.kind() == std::io::ErrorKind::NotFound => {
                // Heuristic: if lstat fails with ENOENT, but `dir_entry` exists,
                // then the directory entry exists but it might be a whiteout file.
                // Assume so if `dir_entry` says it is a character device.
                match dir_entry.file_type().await {
                    Ok(ft) if ft.is_char_device() => {
                        // XXX: mode and size?
                        entry.kind = EntryKind::Mask;
                        entry.object = encoding::NULL_DIGEST.into();
                        return Ok(entry);
                    }
                    Ok(_) => {
                        return Err(Error::String(format!(
                            "Unexpected non-char device file: {}",
                            path.as_ref().display()
                        )))
                    }
                    Err(err) => return Err(Error::StorageReadError(path.as_ref().to_owned(), err)),
                }
            }
            Err(err) => return Err(Error::StorageReadError(path.as_ref().to_owned(), err)),
        };

        entry.mode = stat_result.mode();
        entry.size = stat_result.size();

        let file_type = stat_result.file_type();
        if file_type.is_symlink() {
            let link_target = tokio::fs::read_link(&path)
                .await
                .map_err(|err| Error::StorageReadError(path.as_ref().to_owned(), err))?
                .into_os_string()
                .into_string()
                .map_err(|_| {
                    crate::Error::String("Symlinks must point to a valid utf-8 path".to_string())
                })?
                .into_bytes();
            entry.kind = EntryKind::Blob;
            entry.object = mb
                .hasher
                .hasher(Box::pin(std::io::Cursor::new(link_target)))
                .await?;
        } else if file_type.is_dir() {
            entry = Self::compute_tree_node(mb, path, entry).await?;
        } else if filesystem::overlayfs::is_removed_entry(&stat_result) {
            entry.kind = EntryKind::Mask;
            entry.object = encoding::NULL_DIGEST.into();
        } else if !stat_result.is_file() {
            return Err(format!("unsupported special file: {:?}", path.as_ref()).into());
        } else {
            entry.kind = EntryKind::Blob;
            let reader = tokio::io::BufReader::new(
                tokio::fs::File::open(&path)
                    .await
                    .map_err(|err| Error::StorageReadError(path.as_ref().to_owned(), err))?,
            );
            entry.object = mb.hasher.hasher(Box::pin(reader)).await?;
        }
        Ok(entry)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ManifestNode<'a> {
    pub path: RelativePathBuf,
    pub entry: &'a Entry,
}

impl<'a> Ord for ManifestNode<'a> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use itertools::EitherOrBoth::{Both, Left, Right};
        use relative_path::Component::Normal;
        use std::cmp::Ordering;

        let self_path = self.path.normalize();
        let other_path = other.path.normalize();
        let mut path_iter = self_path
            .components()
            .zip_longest(other_path.components())
            .peekable();

        loop {
            let item = path_iter.next();
            if let Some(item) = item {
                // we only expect normal path components here due to the fact that
                // we are normalizing the path before iteration, any '.' or '..' entries
                // will mess with this comparison process.
                match item {
                    Both(Normal(left), Normal(right)) => {
                        let kinds = match path_iter.peek() {
                            Some(Both(Normal(_), Normal(_))) => (EntryKind::Tree, EntryKind::Tree),
                            Some(Left(_)) => (EntryKind::Tree, other.entry.kind),
                            Some(Right(_)) => (self.entry.kind, EntryKind::Tree),
                            _ => (self.entry.kind, other.entry.kind),
                        };
                        // let the entry type take precedence over any name
                        // - this is to ensure directories are sorted first
                        let cmp = match kinds.1.cmp(&kinds.0) {
                            Ordering::Equal => left.cmp(right),
                            cmp => cmp,
                        };
                        if let Ordering::Equal = cmp {
                            continue;
                        }
                        return cmp;
                    }
                    Left(_) => {
                        return std::cmp::Ordering::Greater;
                    }
                    Right(_) => {
                        return std::cmp::Ordering::Less;
                    }
                    _ => continue,
                }
            } else {
                break;
            }
        }
        std::cmp::Ordering::Equal
    }
}

impl<'a> PartialOrd for ManifestNode<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
