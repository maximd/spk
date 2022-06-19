// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use std::collections::HashSet;

use rstest::rstest;
use serde_yaml;

use super::{parse_ident_range, InclusionPolicy, PkgRequest, PreReleasePolicy, VarRequest};
use crate::api;

#[rstest]
#[case("python/3.1.0", &[])]
#[case("python:lib/3.1.0", &["lib"])]
#[case("python:{lib}/3.1.0", &["lib"])]
#[case("python:{lib,bin}/3.1.0", &["lib", "bin"])]
#[case("python:{lib,bin,dev}/3.1.0", &["lib", "bin", "dev"])]
#[should_panic]
#[case("python.Invalid/3.1.0", &[""])]
#[should_panic]
#[case("python.lib,bin/3.1.0", &[""])]
fn test_parse_ident_range_components(#[case] source: &str, #[case] expected: &[&str]) {
    let actual = parse_ident_range(source).unwrap();
    let expected: HashSet<_> = expected
        .iter()
        .map(api::Component::parse)
        .map(Result::unwrap)
        .collect();
    assert_eq!(actual.components, expected);
}

#[rstest]
fn test_range_ident_restrict_components() {
    let mut first = parse_ident_range("python:lib").unwrap();
    let second = parse_ident_range("python:bin").unwrap();
    let expected = parse_ident_range("python:{bin,lib}").unwrap();
    first.restrict(&second).unwrap();
    assert_eq!(first.components, expected.components);
}

#[rstest]
fn test_prerelease_policy() {
    let mut a: PkgRequest =
        serde_yaml::from_str("{pkg: something, prereleasePolicy: IncludeAll}").unwrap();
    let b: PkgRequest =
        serde_yaml::from_str("{pkg: something, prereleasePolicy: ExcludeAll}").unwrap();

    a.restrict(&b).unwrap();
    match a.prerelease_policy {
        PreReleasePolicy::ExcludeAll => (),
        _ => panic!("expected restricted prerelease policy"),
    }
}

#[rstest]
fn test_inclusion_policy() {
    let mut a: PkgRequest =
        serde_yaml::from_str("{pkg: something, include: IfAlreadyPresent}").unwrap();
    let b: PkgRequest = serde_yaml::from_str("{pkg: something, include: Always}").unwrap();

    a.restrict(&b).unwrap();
    match a.inclusion_policy {
        InclusionPolicy::Always => (),
        _ => panic!("expected restricted inclusion policy"),
    }
}

#[rstest]
fn test_deserialize_value_or_pin() {
    let res = serde_yaml::from_str::<VarRequest>("{var: python.abi/cp27m}");
    assert!(res.is_ok(), "should allow regular name/value");

    let res = serde_yaml::from_str::<VarRequest>("{var: python.abi, fromBuildEnv: true}");
    assert!(res.is_ok(), "should allow no value when pinning build env");

    let res = serde_yaml::from_str::<VarRequest>("{var: python.abi/cp27m, fromBuildEnv: true}");
    assert!(res.is_err(), "should not allow value and pin");

    let res = serde_yaml::from_str::<VarRequest>("{var: python.abi}");
    assert!(res.is_err(), "should not allow omitting value without pin");
}

#[rstest]
fn test_var_request_empty_value_roundtrip() {
    let req = serde_yaml::from_str::<VarRequest>("{var: python.abi/}").unwrap();
    let yaml = serde_yaml::to_string(&req).unwrap();
    let res = serde_yaml::from_str::<VarRequest>(&yaml);
    assert!(
        res.is_ok(),
        "should be able to round-trip serialize a var request with empty string value"
    );
}

#[rstest]
fn test_var_request_pinned_roundtrip() {
    let req = serde_yaml::from_str::<VarRequest>("{var: python.abi, fromBuildEnv: true}").unwrap();
    let yaml = serde_yaml::to_string(&req).unwrap();
    let res = serde_yaml::from_str::<VarRequest>(&yaml);
    assert!(
        res.is_ok(),
        "should be able to round-trip serialize a var request with pin"
    );
    assert!(res.unwrap().pin, "should preserve pin value");
}

#[rstest]
#[case("1.2.3", "x.x.x", "1.2.3")]
#[case("1.2.3", "x", "1")]
#[case("1.2.3", "x.x", "1.2")]
#[case("1.2.3", "~x.x.x.x", "~1.2.3.0")]
#[case("1.2.3", "Binary", "Binary:1.2.3")]
#[case("1.2.3", "API", "API:1.2.3")]
#[case("1.2.3.4.5", "API", "API:1.2.3.4.5")]
#[case("1.2.3", "API:x.x", "API:1.2")]
#[case("1.2.3", "true", "Binary:1.2.3")]
fn test_pkg_request_pin_rendering(
    #[case] version: &str,
    #[case] pin: &str,
    #[case] expected: &str,
) {
    let req = serde_yaml::from_str::<PkgRequest>(&format!("{{pkg: test, fromBuildEnv: {}}}", pin))
        .unwrap();
    let version = api::parse_ident(format!("test/{}", version)).unwrap();
    let res = req
        .render_pin(&version)
        .expect("should not fail to render pin");
    assert_eq!(&res.pkg.version.to_string(), expected);
}

#[rstest]
fn test_var_request_base_name() {
    let req = VarRequest {
        var: "pkg.var".to_string(),
        value: String::new(),
        pin: false,
    };
    assert_eq!(req.base_name(), "var");

    let req = VarRequest {
        var: "var".to_string(),
        value: String::new(),
        pin: false,
    };
    assert_eq!(req.base_name(), "var");
}