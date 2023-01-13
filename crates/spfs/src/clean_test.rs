// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use rstest::rstest;
use storage::prelude::*;
use tokio::time::sleep;

use super::{
    clean_untagged_objects,
    get_all_attached_objects,
    get_all_unattached_objects,
    get_all_unattached_payloads,
};
use crate::encoding::Encodable;
use crate::fixtures::*;
use crate::{graph, storage, tracking, Error};

#[rstest]
#[tokio::test]
async fn test_get_attached_objects(#[future] tmprepo: TempRepo) {
    let tmprepo = tmprepo.await;
    let reader = Box::pin("hello, world".as_bytes());
    let payload_digest = tmprepo.commit_blob(reader).await.unwrap();

    assert_eq!(
        get_all_attached_objects(&tmprepo).await.unwrap(),
        Default::default(),
        "single blob should not be attached"
    );
    let mut expected = HashSet::new();
    expected.insert(payload_digest);
    assert_eq!(
        get_all_unattached_objects(&tmprepo).await.unwrap(),
        expected,
        "single blob should be unattached"
    );
}

#[rstest]
#[tokio::test]
async fn test_get_attached_payloads(#[future] tmprepo: TempRepo) {
    let tmprepo = tmprepo.await;
    let reader = Box::pin("hello, world".as_bytes());
    // Safety: we explicitly do not want this
    // payload to appear attached to the graph
    let (payload_digest, _) = unsafe { tmprepo.write_data(reader).await.unwrap() };
    let mut expected = HashSet::new();
    expected.insert(payload_digest);
    assert_eq!(
        get_all_unattached_payloads(&tmprepo).await.unwrap(),
        expected,
        "single payload should be attached when no blob"
    );

    let blob = graph::Blob::new(payload_digest, 0);
    tmprepo.write_blob(blob).await.unwrap();

    assert_eq!(
        get_all_unattached_payloads(&tmprepo).await.unwrap(),
        Default::default(),
        "single payload should be attached to blob"
    );
}

#[rstest]
#[tokio::test]
async fn test_get_attached_unattached_objects_blob(
    #[future] tmprepo: TempRepo,
    tmpdir: tempfile::TempDir,
) {
    init_logging();
    let tmprepo = tmprepo.await;

    let data_dir = tmpdir.path().join("data");
    ensure(data_dir.join("file.txt"), "hello, world");

    let manifest = crate::commit_dir(tmprepo.repo(), data_dir.as_path())
        .await
        .unwrap();
    let layer = tmprepo
        .create_layer(&graph::Manifest::from(&manifest))
        .await
        .unwrap();
    let tag = tracking::TagSpec::parse("my_tag").unwrap();
    tmprepo
        .push_tag(&tag, &layer.digest().unwrap())
        .await
        .unwrap();
    let blob_digest = manifest
        .root()
        .entries
        .get("file.txt")
        .expect("file should exist in committed manifest")
        .object;

    assert!(
        get_all_attached_objects(&tmprepo)
            .await
            .unwrap()
            .contains(&blob_digest),
        "blob in manifest in tag should be attached"
    );
    assert!(
        !get_all_unattached_objects(&tmprepo)
            .await
            .unwrap()
            .contains(&blob_digest),
        "blob in manifest in tag should not be unattached"
    );
}

#[rstest]
#[tokio::test]
async fn test_clean_untagged_objects(#[future] tmprepo: TempRepo, tmpdir: tempfile::TempDir) {
    init_logging();
    let tmprepo = tmprepo.await;

    // Group 1: untagged objects
    let data_dir_1 = tmpdir.path().join("data");
    ensure(data_dir_1.join("dir/dir/test.file"), "1 hello");
    ensure(data_dir_1.join("dir/dir/test.file2"), "1 hello, world");
    ensure(data_dir_1.join("dir/dir/test.file4"), "1 hello, world");

    let manifest1 = crate::commit_dir(tmprepo.repo(), data_dir_1.as_path())
        .await
        .unwrap();

    // Group 2: tagged objects
    let data_dir_2 = tmpdir.path().join("data2");
    ensure(data_dir_2.join("dir/dir/test.file"), "2 hello");
    ensure(data_dir_2.join("dir/dir/test.file2"), "2 hello, world");

    let manifest2 = crate::commit_dir(tmprepo.repo(), data_dir_2.as_path())
        .await
        .unwrap();
    let layer = tmprepo
        .create_layer(&graph::Manifest::from(&manifest2))
        .await
        .unwrap();
    let tag = tracking::TagSpec::parse("tagged_manifest").unwrap();
    tmprepo
        .push_tag(&tag, &layer.digest().unwrap())
        .await
        .unwrap();

    // Note current time now.
    let time_before_group_three = Utc::now();

    // Ensure these new files are created a measurable amount of time after
    // the noted time.
    sleep(Duration::from_millis(250)).await;

    // Group 3: untagged objects created after grabbing time.
    let data_dir_3 = tmpdir.path().join("data");
    ensure(data_dir_3.join("dir/dir/test.file"), "3 hello");
    ensure(data_dir_3.join("dir/dir/test.file2"), "3 hello, world");
    ensure(data_dir_3.join("dir/dir/test.file4"), "3 hello, world");

    let manifest3 = crate::commit_dir(tmprepo.repo(), data_dir_3.as_path())
        .await
        .unwrap();

    // Clean objects older than group 3.
    clean_untagged_objects(time_before_group_three, &tmprepo, false)
        .await
        .expect("failed to clean objects");

    for node in manifest1.walk() {
        if !node.entry.kind.is_blob() {
            continue;
        }
        let res = tmprepo.open_payload(node.entry.object).await;
        if let Err(Error::UnknownObject(_)) = res {
            continue;
        }
        if let Err(err) = res {
            println!("{:?}", err);
        }
        panic!(
            "expected object to be cleaned but it was not: {:?}",
            node.entry.object
        );
    }

    for node in manifest2.walk() {
        if !node.entry.kind.is_blob() {
            continue;
        }
        tmprepo
            .open_payload(node.entry.object)
            .await
            .expect("expected payload not to be cleaned");
    }

    // Group 3 should not have been cleaned...

    for node in manifest3.walk() {
        if !node.entry.kind.is_blob() {
            continue;
        }
        tmprepo
            .open_payload(node.entry.object)
            .await
            .expect("expected payload not to be cleaned");
    }
}

#[rstest]
#[tokio::test]
async fn test_clean_untagged_objects_layers_platforms(#[future] tmprepo: TempRepo) {
    let tmprepo = tmprepo.await;
    let manifest = tracking::Manifest::default();
    let layer = tmprepo
        .create_layer(&graph::Manifest::from(&manifest))
        .await
        .unwrap();
    let platform = tmprepo
        .create_platform(vec![layer.digest().unwrap()])
        .await
        .unwrap();

    clean_untagged_objects(Utc::now(), &tmprepo, false)
        .await
        .expect("failed to clean objects");

    if let Err(Error::UnknownObject(_)) = tmprepo.read_layer(layer.digest().unwrap()).await {
        // ok
    } else {
        panic!("expected layer to be cleaned")
    }

    if let Err(Error::UnknownObject(_)) = tmprepo.read_platform(platform.digest().unwrap()).await {
        // ok
    } else {
        panic!("expected platform to be cleaned")
    }
}

#[rstest]
#[tokio::test]
async fn test_clean_manifest_renders(tmpdir: tempfile::TempDir) {
    let tmprepo = Arc::new(
        storage::fs::FSRepository::create(tmpdir.path())
            .await
            .unwrap()
            .into(),
    );

    let data_dir = tmpdir.path().join("data");
    ensure(data_dir.join("dir/dir/file.txt"), "hello");
    ensure(data_dir.join("dir/name.txt"), "john doe");

    let manifest = crate::commit_dir(Arc::clone(&tmprepo), data_dir.as_path())
        .await
        .unwrap();
    let layer = tmprepo
        .create_layer(&graph::Manifest::from(&manifest))
        .await
        .unwrap();
    let _platform = tmprepo
        .create_platform(vec![layer.digest().unwrap()])
        .await
        .unwrap();

    // Safety: tmprepo was created as an FSRepository
    let tmprepo = match unsafe { &*Arc::into_raw(tmprepo) } {
        RepositoryHandle::FS(fs) => fs,
        _ => panic!("Unexpected tmprepo type!"),
    };

    tmprepo
        .render_manifest(&graph::Manifest::from(&manifest), None)
        .await
        .unwrap();

    let files = list_files(tmprepo.objects.root());
    assert!(!files.is_empty(), "should have stored data");

    clean_untagged_objects(Utc::now(), &tmprepo.clone().into(), false)
        .await
        .expect("failed to clean repo");

    let files = list_files(tmprepo.renders.as_ref().unwrap().renders.root());
    assert!(files.is_empty(), "should remove all created data files");
}

fn list_files<P: AsRef<std::path::Path>>(dirname: P) -> Vec<String> {
    let mut all_files = Vec::new();

    for entry in walkdir::WalkDir::new(dirname) {
        let entry = entry.expect("error while listing dir recursively");
        if entry.metadata().unwrap().is_dir() {
            continue;
        }
        all_files.push(entry.path().to_owned().to_string_lossy().to_string())
    }
    all_files
}
