// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use miette::Diagnostic;
use thiserror::Error;

#[derive(Diagnostic, Debug, Error)]
#[diagnostic(
    url(
        "https://getspk.io/error_codes#{}",
        self.code().unwrap_or_else(|| Box::new("spfs::generic"))
    )
)]
pub enum ObjectError {
    #[error("Invalid object header, not enough data")]
    HeaderTooShort,

    #[error("Invalid object header, prefix was incorrect")]
    HeaderMissingPrefix,

    #[error("Invalid object data")]
    InvalidFlatbuffer(#[from] flatbuffers::InvalidFlatbuffer),

    #[error("Unexpected or unknown object kind {0:?}")]
    UnexpectedKind(u8),

    #[error("Unrecognized object encoding: {0}")]
    #[diagnostic(help("Your version of spfs may be too old to read this data"))]
    UnknownEncoding(u8),

    #[error("Unrecognized object digest strategy: {0}")]
    #[diagnostic(help("Your version of spfs may be too old to read this data"))]
    UnknownDigestStrategy(u8),
}

pub type ObjectResult<T> = std::result::Result<T, ObjectError>;
