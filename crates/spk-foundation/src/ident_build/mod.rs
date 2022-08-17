// Copyright (c) 2022 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

mod build;
mod error;
mod format;
pub mod parsing;

pub use build::{parse_build, Build, InvalidBuildError, EMBEDDED, SRC};
pub use error::{Error, Result};
