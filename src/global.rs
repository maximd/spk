// Copyright (c) 2022 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::{convert::TryInto, sync::Arc};

use crate::{
    api,
    storage::{self, Repository},
    Error, Result,
};

#[cfg(test)]
#[path = "./global_test.rs"]
mod global_test;

/// Load a package spec from the default repository.
pub fn load_spec<S: TryInto<api::Ident, Error = crate::Error>>(pkg: S) -> Result<Arc<api::Spec>> {
    let pkg = pkg.try_into()?;

    match crate::HANDLE
        .block_on(storage::remote_repository("origin"))?
        .read_spec(&pkg)
    {
        Err(Error::PackageNotFoundError(_)) => crate::HANDLE
            .block_on(storage::local_repository())?
            .read_spec(&pkg),
        res => res,
    }
}

/// Save a package spec to the local repository.
pub fn save_spec(spec: &api::Spec) -> Result<()> {
    let repo = crate::HANDLE.block_on(storage::local_repository())?;
    repo.force_publish_spec(spec)
}
