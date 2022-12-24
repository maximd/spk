// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::sync::Arc;

use rand::Rng;
use rstest::{fixture, rstest};
use storage::RepositoryHandle;

use super::Syncer;
use crate::config::Config;
use crate::fixtures::*;
use crate::prelude::*;
use crate::{encoding, graph, storage, tracking, Error};

#[rstest]
#[tokio::test]
async fn test_sync_ref_unknown(#[future] config: (tempfile::TempDir, Config)) {
    init_logging();
    let (_handle, config) = config.await;
    let local = config.get_local_repository().await.unwrap().into();
    let origin = config.get_remote("origin").await.unwrap();
    let syncer = Syncer::new(&local, &origin);
    match syncer.sync_ref("--test-unknown--").await {
        Err(Error::UnknownReference(_)) => (),
        Err(err) => panic!("expected unknown reference error, got {:?}", err),
        Ok(_) => panic!("expected unknown reference error, got success"),
    }

    match syncer
        .sync_ref(encoding::Digest::default().to_string())
        .await
    {
        Err(Error::UnknownObject(_)) => (),
        Err(err) => panic!("expected unknown object error, got {:?}", err),
        Ok(_) => panic!("expected unknown object error, got success"),
    }
}

#[rstest]
#[tokio::test]
async fn test_push_ref(#[future] config: (tempfile::TempDir, Config)) {
    init_logging();
    let (tmpdir, config) = config.await;
    let src_dir = tmpdir.path().join("source");
    ensure(src_dir.join("dir/file.txt"), "hello");
    ensure(src_dir.join("dir2/otherfile.txt"), "hello2");
    ensure(src_dir.join("dir//dir/dir/file.txt"), "hello, world");

    let local = Arc::new(config.get_local_repository().await.unwrap().into());
    let remote = config.get_remote("origin").await.unwrap();
    let manifest = crate::commit_dir(Arc::clone(&local), src_dir.as_path())
        .await
        .unwrap();
    let layer = local
        .create_layer(&graph::Manifest::from(&manifest))
        .await
        .unwrap();
    let tag = tracking::TagSpec::parse("testing").unwrap();
    local
        .push_tag(&tag, &layer.digest().unwrap())
        .await
        .unwrap();

    let syncer = Syncer::new(&local, &remote);
    syncer.sync_ref(tag.to_string()).await.unwrap();

    assert!(remote.read_ref("testing").await.is_ok());
    assert!(remote.has_layer(layer.digest().unwrap()).await);

    assert!(syncer.sync_ref(tag.to_string()).await.is_ok());
}

#[rstest]
#[case::fs(tmprepo("fs"), tmprepo("fs"))]
#[case::tar(tmprepo("tar"), tmprepo("tar"))]
#[cfg_attr(feature = "server", case::rpc(tmprepo("rpc"), tmprepo("rpc")))]
#[tokio::test]
async fn test_sync_ref(
    #[case]
    #[future]
    repo_a: TempRepo,
    #[case]
    #[future]
    repo_b: TempRepo,
    tmpdir: tempfile::TempDir,
) {
    init_logging();
    let repo_a = repo_a.await;
    let repo_b = repo_b.await;

    let src_dir = tmpdir.path().join("source");
    ensure(src_dir.join("dir/file.txt"), "hello");
    ensure(src_dir.join("dir2/otherfile.txt"), "hello2");
    ensure(src_dir.join("dir//dir/dir/file.txt"), "hello, world");

    let manifest = crate::commit_dir(repo_a.repo(), src_dir.as_path())
        .await
        .unwrap();
    let layer = repo_a
        .create_layer(&graph::Manifest::from(&manifest))
        .await
        .unwrap();
    let platform = repo_a
        .create_platform(vec![layer.digest().unwrap()])
        .await
        .unwrap();
    let tag = tracking::TagSpec::parse("testing").unwrap();
    repo_a
        .push_tag(&tag, &platform.digest().unwrap())
        .await
        .unwrap();

    Syncer::new(&repo_a, &repo_b)
        .sync_ref("testing")
        .await
        .expect("failed to sync ref");

    assert!(repo_b.read_ref("testing").await.is_ok());
    assert!(repo_b.has_platform(platform.digest().unwrap()).await);
    assert!(repo_b.has_layer(layer.digest().unwrap()).await);

    Syncer::new(&repo_b, &repo_a)
        .sync_ref("testing")
        .await
        .expect("failed to sync back");

    assert!(repo_a.read_ref("testing").await.is_ok());
    assert!(repo_a.has_layer(layer.digest().unwrap()).await);
}

#[rstest]
#[case::fs(tmprepo("fs"), tmprepo("fs"))]
#[case::tar(tmprepo("tar"), tmprepo("tar"))]
#[cfg_attr(feature = "server", case::rpc(tmprepo("rpc"), tmprepo("rpc")))]
#[tokio::test]
async fn test_sync_missing_from_source(
    #[case]
    #[future]
    repo_a: TempRepo,
    #[case]
    #[future]
    repo_b: TempRepo,
    tmpdir: tempfile::TempDir,
) {
    init_logging();
    let repo_a = repo_a.await;
    let repo_b = repo_b.await;

    // when sync targets exist in the destination already
    // and we are not forcefully re-syncing, the syncer
    // should not fail no matter what type of target is being synced
    //
    // this ensures that callers don't need to pre-check
    // all of their targets, allowing that logic to live
    // in the syncer (DRY)

    let src_dir = tmpdir.path().join("source");
    ensure(src_dir.join("dir/file.txt"), "hello");
    ensure(src_dir.join("dir2/otherfile.txt"), "hello2");
    ensure(src_dir.join("dir//dir/dir/file.txt"), "hello, world");

    let manifest = crate::commit_dir(repo_b.repo(), src_dir.as_path())
        .await
        .unwrap();
    let layer = repo_b
        .create_layer(&graph::Manifest::from(&manifest))
        .await
        .unwrap();
    let platform = repo_b
        .create_platform(vec![layer.digest().unwrap()])
        .await
        .unwrap();
    let tag = tracking::TagSpec::parse("testing").unwrap();
    repo_b
        .push_tag(&tag, &platform.digest().unwrap())
        .await
        .unwrap();

    let syncer = Syncer::new(&repo_a, &repo_b);

    let platform_digest = platform.digest().unwrap();
    let partial = platform_digest[..10].into();
    syncer
        .sync_digest(platform_digest)
        .await
        .expect("Should not fail when object is already in destination");
    syncer
        .sync_partial_digest(partial)
        .await
        .expect("Should not fail when object is already in destination");
    syncer
        .sync_env(tag.into())
        .await
        .expect("Should not fail when object is already in destination");
    syncer
        .sync_env(platform_digest.into())
        .await
        .expect("Should not fail when object is already in destination");
}

#[rstest]
#[case::fs(tmprepo("fs"), tmprepo("fs"))]
#[case::tar(tmprepo("tar"), tmprepo("tar"))]
#[cfg_attr(feature = "server", case::rpc(tmprepo("rpc"), tmprepo("rpc")))]
#[tokio::test]
async fn test_sync_through_tar(
    #[case]
    #[future]
    repo_a: TempRepo,
    #[case]
    #[future]
    repo_b: TempRepo,
    tmpdir: tempfile::TempDir,
) {
    init_logging();
    let repo_a = repo_a.await;
    let repo_b = repo_b.await;

    let dir = tmpdir.path();
    let src_dir = dir.join("source");
    ensure(src_dir.join("dir/file.txt"), "hello");
    ensure(src_dir.join("dir2/otherfile.txt"), "hello2");
    ensure(src_dir.join("dir//dir/dir/file.txt"), "hello, world");

    let repo_tar: RepositoryHandle = storage::tar::TarRepository::create(dir.join("repo.tar"))
        .await
        .unwrap()
        .into();

    let manifest = crate::commit_dir(repo_a.repo(), src_dir.as_path())
        .await
        .unwrap();
    let layer = repo_a
        .create_layer(&graph::Manifest::from(&manifest))
        .await
        .unwrap();
    let platform = repo_a
        .create_platform(vec![layer.digest().unwrap()])
        .await
        .unwrap();
    let tag = tracking::TagSpec::parse("testing").unwrap();
    repo_a
        .push_tag(&tag, &platform.digest().unwrap())
        .await
        .unwrap();

    Syncer::new(&repo_a, &repo_tar)
        .sync_ref("testing")
        .await
        .unwrap();
    drop(repo_tar);
    let repo_tar = storage::tar::TarRepository::open(dir.join("repo.tar"))
        .await
        .unwrap()
        .into();
    Syncer::new(&repo_tar, &repo_b)
        .sync_ref("testing")
        .await
        .unwrap();

    assert!(repo_b.read_ref("testing").await.is_ok());
    assert!(repo_b.has_layer(layer.digest().unwrap()).await);
}

#[rstest]
#[case::fs(tmprepo("fs"), tmprepo("fs"))]
#[case::tar(tmprepo("tar"), tmprepo("tar"))]
#[cfg_attr(feature = "server", case::rpc(tmprepo("rpc"), tmprepo("rpc")))]
#[tokio::test]
async fn test_sync_with_payloads(
    #[case]
    #[future]
    repo_a: TempRepo,
    #[case]
    #[future]
    repo_b: TempRepo,
    tmpdir: tempfile::TempDir,
) {
    init_logging();
    let repo_a = repo_a.await;
    let repo_b = repo_b.await;

    let total_files = rand::thread_rng().gen_range(10_000..20_000);
    generate_file_tree(tmpdir.path(), total_files);

    let manifest = crate::commit_dir(repo_a.repo(), tmpdir.path())
        .await
        .expect("should not fail to commit generated dir");

    // try to introduce contention issues by running the sync many times
    let syncer = Syncer::new(&repo_a, &repo_b);
    let _ = tokio::try_join!(
        syncer.sync_manifest((&manifest).into()),
        syncer.sync_manifest((&manifest).into()),
        syncer.sync_manifest((&manifest).into()),
        syncer.sync_manifest((&manifest).into())
    )
    .expect("Should not fail to sync");
    let errors = match &*repo_b.repo() {
        RepositoryHandle::FS(repo) => crate::graph::check_database_integrity(repo).await,
        RepositoryHandle::Tar(repo) => crate::graph::check_database_integrity(repo).await,
        RepositoryHandle::Rpc(repo) => crate::graph::check_database_integrity(repo).await,
        RepositoryHandle::Proxy(repo) => crate::graph::check_database_integrity(&**repo).await,
    };
    for error in errors.iter() {
        tracing::error!(%error);
    }
    if !errors.is_empty() {
        panic!("Expected no database integrity issues");
    }
}

fn generate_file_tree(root: &std::path::Path, mut file_count: usize) {
    let mut rng = rand::thread_rng();
    fn gen_name(len: usize) -> String {
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(len)
            .map(char::from)
            .collect()
    }
    if file_count > 10 {
        let dir_count = rng.gen_range(1..5);
        let dir_portion = file_count / dir_count;
        for _dir in 0..dir_count {
            let dir_name = gen_name(12);
            let dir_path = root.join(dir_name);
            std::fs::create_dir(&dir_path).unwrap();
            let dir_file_count = rng.gen_range(1..dir_portion);
            generate_file_tree(&dir_path, dir_file_count);
            file_count -= dir_file_count;
        }
    }
    for _file in 0..file_count {
        let file_name = gen_name(12);
        // we want to create some payloads that are the same
        let file_data = gen_name(2);
        let file_path = root.join(file_name);
        std::fs::write(&file_path, file_data).unwrap();
    }
}

#[fixture]
async fn config(tmpdir: tempfile::TempDir) -> (tempfile::TempDir, Config) {
    let repo_path = tmpdir.path().join("repo");
    crate::storage::fs::FSRepository::create(&repo_path)
        .await
        .expect("failed to make repo for test");
    let origin_path = tmpdir.path().join("origin");
    crate::storage::fs::FSRepository::create(&origin_path)
        .await
        .expect("failed to make repo for test");
    let mut conf = Config::default();
    conf.remote.insert(
        "origin".to_string(),
        crate::config::Remote::Address(crate::config::RemoteAddress {
            address: url::Url::from_file_path(&origin_path).unwrap(),
        }),
    );
    conf.storage.root = repo_path;
    (tmpdir, conf)
}
