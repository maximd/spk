// Copyright (c) 2022 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Build(#[from] crate::build::BuildError),
    #[error(transparent)]
    Collection(#[from] crate::build::CollectionError),
    #[error(transparent)]
    IO(#[from] std::io::Error),
    #[error(transparent)]
    SPFS(#[from] spfs::Error),
    #[error(transparent)]
    SpkExecError(#[from] spk_exec::Error),
    #[error(transparent)]
    SpkIdentError(#[from] spk_ident::Error),
    #[error(transparent)]
    SpkSolverError(#[from] spk_solver::Error),
    #[error(transparent)]
    SpkSpecError(#[from] spk_spec::Error),
    #[error(transparent)]
    SpkStorageError(#[from] spk_storage::Error),
    #[error("Error: {0}")]
    String(String),
}
