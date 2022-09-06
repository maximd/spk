// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use super::FileMatcher;

pub trait ComponentOps {
    fn files(&self) -> &FileMatcher;
}