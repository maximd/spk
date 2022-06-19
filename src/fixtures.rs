// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::ops::DerefMut;
use std::sync::Arc;

use rstest::fixture;
use spfs::prelude::*;
use tokio::sync::{Mutex, MutexGuard};

use crate::storage;

lazy_static::lazy_static! {
    static ref SPFS_RUNTIME_LOCK: Mutex<()> = Mutex::new(());
}

pub struct RuntimeLock {
    original_config: spfs::Config,
    _guard: MutexGuard<'static, ()>,
    pub runtime: spfs::runtime::Runtime,
    pub tmprepo: Arc<storage::RepositoryHandle>,
    pub tmpdir: tempdir::TempDir,
}

impl Drop for RuntimeLock {
    fn drop(&mut self) {
        std::env::remove_var("SPFS_STORAGE_ROOT");
        self.original_config
            .clone()
            .make_current()
            .expect("Failed to reset spfs config after test");
    }
}

/// The types of temporary repositories that can be created.
#[derive(Debug, Eq, PartialEq)]
pub enum RepoKind {
    Mem,
    Spfs,
}

/// A temporary repository of some type for use in testing
pub struct TempRepo {
    pub repo: Arc<storage::RepositoryHandle>,
    pub tmpdir: tempdir::TempDir,
}

impl std::ops::Deref for TempRepo {
    type Target = storage::RepositoryHandle;

    fn deref(&self) -> &Self::Target {
        &*self.repo
    }
}

pub fn init_logging() {
    let sub = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::TRACE)
        .without_time()
        .with_test_writer()
        .finish();
    let _ = tracing::subscriber::set_global_default(sub);
}

/// Returns an empty spfs layer object for easy testing
pub fn empty_layer() -> spfs::graph::Layer {
    spfs::graph::Layer {
        manifest: Default::default(),
    }
}

/// Returns the digest for an empty spfs layer.
pub fn empty_layer_digest() -> spfs::Digest {
    empty_layer()
        .digest()
        .expect("Empty layer should have valid digest")
}

#[fixture]
pub fn tmpdir() -> tempdir::TempDir {
    tempdir::TempDir::new("spk-test-").expect("Failed to establish temporary directory for testing")
}

#[fixture]
pub fn tmprepo() -> storage::RepositoryHandle {
    storage::RepositoryHandle::Mem(Default::default())
}

/// Establishes a temporary spfs repo on disk.
///
/// This repo comes prefilled with an empty layer and object
/// for use in generating test data to sync around.
#[fixture]
pub async fn spfsrepo() -> TempRepo {
    make_repo(RepoKind::Spfs).await
}

/// Create a temporary repository of the desired flavor
pub async fn make_repo(kind: RepoKind) -> TempRepo {
    tracing::trace!(?kind, "creating repo for test...");

    let tmpdir = tempdir::TempDir::new("spk-test-spfs-repo")
        .expect("failed to establish tmpdir for spfs runtime");
    let repo = match kind {
        RepoKind::Spfs => {
            let storage_root = tmpdir.path().join("repo");
            let spfs_repo = spfs::storage::fs::FSRepository::create(&storage_root)
                .await
                .expect("failed to establish temporary local repo for test");
            let written = spfs_repo
                .write_data(Box::pin(std::io::Cursor::new(b"")))
                .await
                .expect("failed to add an empty object to spfs");
            let empty_manifest = spfs::graph::Manifest::default();
            let empty_layer = empty_layer();
            let _ = spfs_repo
                .write_object(&empty_layer.into())
                .await
                .expect("failed to save empty layer to spfs repo");
            let _ = spfs_repo
                .write_object(&empty_manifest.into())
                .await
                .expect("failed to save empty manifest to spfs repo");
            assert_eq!(written.0, spfs::encoding::EMPTY_DIGEST.into());
            storage::RepositoryHandle::SPFS(spfs_repo.into())
        }
        RepoKind::Mem => storage::RepositoryHandle::new_mem(),
    };

    let repo = Arc::new(repo);
    TempRepo { tmpdir, repo }
}

/// Establishes a segregated spfs runtime for use in the test.
///
/// This is a managed resource, and will cause all tests that use
/// it to run serially.
#[fixture]
pub async fn spfs_runtime() -> RuntimeLock {
    init_logging();

    // because these tests are all async, anything that is interacting
    // with spfs must be forced to run one-at-a-time
    let _guard = SPFS_RUNTIME_LOCK.lock().await;
    let mut runtime = spfs::active_runtime()
        .await
        .expect("Test must be executed in an active spfs runtime (spfs run - -- cargo test)");

    let original_config = spfs::get_config()
        .expect("failed to get original spfs config")
        .as_ref()
        .clone();

    let tmprepo = spfsrepo().await;
    let storage_root = tmprepo.tmpdir.path().join("repo");

    let mut new_config = original_config.clone();
    // update the config to use our temp dir for local storage
    std::env::set_var("SPFS_STORAGE_ROOT", &storage_root);
    new_config.storage.root = storage_root;

    let config = new_config
        .make_current()
        .expect("failed to update spfs config for test");

    // since the runtime is likely stored in the currently
    // configured local repo, we need to save a repesentation of
    // it in the newly configured tmp storage
    let runtime_storage = config
        .get_runtime_storage()
        .await
        .expect("Failed to load temporary runtime storage");
    let mut replica = runtime_storage
        .create_named_runtime(runtime.name())
        .await
        .expect("Failed to replicate runtime for test");
    std::mem::swap(runtime.deref_mut(), replica.deref_mut());
    drop(runtime);

    replica.status.stack.clear();
    replica
        .reset_all()
        .expect("Failed to reset runtime changes");
    replica
        .save_state_to_storage()
        .await
        .expect("Failed to clean up active runtime state");
    spfs::remount_runtime(&replica)
        .await
        .expect("failed to reset runtime for test");

    RuntimeLock {
        original_config,
        _guard,
        runtime: replica,
        tmpdir: tmprepo.tmpdir,
        tmprepo: tmprepo.repo,
    }
}

/// A simple trait for use in test writing that allows something to be
/// ensured to exist and be usable, whatever that means in context.
pub trait Ensure {
    fn ensure(&self);
}

impl Ensure for std::path::PathBuf {
    fn ensure(&self) {
        if let Some(parent) = self.parent() {
            std::fs::create_dir_all(parent).expect("failed to ensure parent dir for file");
        }
        std::fs::write(self, b"").expect("failed to ensure empty file");
    }
}