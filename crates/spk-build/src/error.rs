// Copyright (c) Sony Pictures Imageworks, et al.
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
    #[error("Failed to create directory {0}")]
    DirectoryCreateError(std::path::PathBuf, #[source] std::io::Error),
    #[error("Failed to open file {0}")]
    FileOpenError(std::path::PathBuf, #[source] std::io::Error),
    #[error("Failed to write file {0}")]
    FileWriteError(std::path::PathBuf, #[source] std::io::Error),
    #[error(transparent)]
    ProcessSpawnError(spfs::Error),
    #[error(transparent)]
    SPFS(#[from] spfs::Error),
    #[error(transparent)]
    SpkExecError(#[from] spk_exec::Error),
    #[error(transparent)]
    SpkIdentError(#[from] spk_schema::ident::Error),
    #[error(transparent)]
    SpkSolverError(#[from] spk_solve::Error),
    #[error(transparent)]
    SpkSpecError(#[from] spk_schema::Error),
    #[error(transparent)]
    SpkStorageError(#[from] spk_storage::Error),
    #[error("Error: {0}")]
    String(String),
}