// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use rstest::rstest;

use super::{PkgOpt, VarOpt};

#[rstest]
#[case("{pkg: my-pkg}", "1", false)]
#[case("{pkg: my-pkg}", "none", true)]
#[case("{pkg: my-pkg}", "", false)]
fn test_pkg_opt_validation(#[case] spec: &str, #[case] value: &str, #[case] expect_err: bool) {
    let mut opt: PkgOpt = serde_yaml::from_str(spec).unwrap();
    let res = opt.set_value(value.to_string());
    assert_eq!(res.is_err(), expect_err);
}

#[rstest]
#[case("{var: my-var, choices: [hello, world]}", "hello", false)]
#[case("{var: my-var, choices: [hello, world]}", "bad", true)]
#[case("{var: my-var, choices: [hello, world]}", "", false)]
fn test_var_opt_validation(#[case] spec: &str, #[case] value: &str, #[case] expect_err: bool) {
    let mut opt: VarOpt = serde_yaml::from_str(spec).unwrap();
    let res = opt.set_value(value.to_string());
    assert_eq!(res.is_err(), expect_err);
}