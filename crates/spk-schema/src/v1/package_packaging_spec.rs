// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use serde::{Deserialize, Serialize};

use super::TestScript;
use crate::{ComponentSpecList, EnvOp};

#[derive(Debug, Default, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub struct PackagePackagingSpec {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment: Vec<EnvOp>,
    #[serde(default)]
    pub components: ComponentSpecList<super::Package>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test: Vec<TestScript>,
}