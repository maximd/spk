// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use super::{must_install_something, must_not_alter_existing_files};
use crate::api::validators::must_collect_all_files;

#[test]
fn test_validate_build_changeset_nothing() {
    let spec = Default::default();
    let res = must_install_something(&spec, &[], "/spfs");
    assert!(res.is_some())
}

#[test]
fn test_validate_build_changeset_modified() {
    let spec = Default::default();
    let res = must_not_alter_existing_files(
        &spec,
        &vec![spfs::tracking::Diff {
            path: "/spfs/file.txt".into(),
            mode: spfs::tracking::DiffMode::Changed,
            entries: None,
        }],
        "/spfs",
    );
    assert!(res.is_some())
}

#[test]
fn test_validate_build_changeset_collected() {
    let mut spec = crate::api::Spec::default();
    // the default components are added and collect all files,
    // so we remove them to ensure nothing is collected
    let _ = spec.install.components.drain(..);
    let res = must_collect_all_files(
        &spec,
        &vec![spfs::tracking::Diff {
            path: "/spfs/file.txt".into(),
            mode: spfs::tracking::DiffMode::Changed,
            entries: None,
        }],
        "/spfs",
    );
    assert!(
        res.is_some(),
        "should get error when a file is created that was not in a component spec"
    )
}