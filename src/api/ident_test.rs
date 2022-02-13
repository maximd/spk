// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::str::FromStr;

use rstest::rstest;

use super::{parse_ident, Ident, RepositoryName};
use crate::api::{parse_version, Build};

#[rstest]
#[case("package")]
#[case("package/1.1.0")]
#[case("package/2.0.0.1")]
fn test_ident_to_str(#[case] input: &str) {
    let ident = parse_ident(input).unwrap();
    let out = ident.to_string();
    assert_eq!(out, input);
}

#[rstest]
fn test_ident_to_yaml() {
    let ident = Ident::from_str("package").unwrap();
    let out = serde_yaml::to_string(&ident).unwrap();
    assert_eq!(&out, "---\npackage\n");
}

#[rstest]
#[case(
    "local/hello/1.0.0/src",
    Ident{repository_name: Some(RepositoryName("local".to_string())), name: "hello".parse().unwrap(), version: parse_version("1.0.0").unwrap(), build: Some(Build::Source)}
)]
#[case(
    "hello/1.0.0/src",
    Ident{
        repository_name: None,
        name: "hello".parse().unwrap(),
        version: parse_version("1.0.0").unwrap(),
        build: Some(Build::Source)
    }
)]
#[case(
    "python/2.7",
    Ident{
        repository_name: None,
        name: "python".parse().unwrap(),
        version: parse_version("2.7").unwrap(),
        build: None
    }
)]
// pathological cases: package named "local"
#[case(
    "local/1.0.0/src",
    Ident{repository_name: None, name: "local".parse().unwrap(), version: parse_version("1.0.0").unwrap(), build: Some(Build::Source)}
)]
#[case(
    "local/1.0.0/DEADBEEF",
    Ident{repository_name: None, name: "local".parse().unwrap(), version: parse_version("1.0.0").unwrap(), build: Some(Build::from_str("DEADBEEF").unwrap())}
)]
#[case(
    "local/1.0.0",
    Ident{repository_name: None, name: "local".parse().unwrap(), version: parse_version("1.0.0").unwrap(), build: None}
)]
fn test_parse_ident(#[case] input: &str, #[case] expected: Ident) {
    let actual = parse_ident(input).unwrap();
    assert_eq!(actual, expected);
}
