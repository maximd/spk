// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;

use relative_path::{RelativePath, RelativePathBuf};
use spfs::prelude::Encodable;

use super::env::data_path;
use crate::{
    api::{self, Package},
    storage, Result,
};

#[cfg(test)]
#[path = "./sources_test.rs"]
mod sources_test;

/// Denotes an error during the build process.
#[derive(Debug, Error)]
#[error("Collection error: {message}")]
pub struct CollectionError {
    pub message: String,
}

impl CollectionError {
    pub fn new_error(format_args: std::fmt::Arguments) -> crate::Error {
        crate::Error::Collection(Self {
            message: std::fmt::format(format_args),
        })
    }
}

/// Builds a source package.
///
/// ```no_run
/// # #[macro_use] extern crate spk;
/// # async fn demo() {
/// spk::build::SourcePackageBuilder::from_spec(spk::spec!({
///        "pkg": "my-pkg",
///     }))
///    .build()
///    .await
///    .unwrap();
/// # }
/// ```
pub struct SourcePackageBuilder {
    spec: api::Spec,
    repo: Option<Arc<storage::RepositoryHandle>>,
    prefix: PathBuf,
}

impl SourcePackageBuilder {
    pub fn from_spec(mut spec: api::Spec) -> Self {
        match &mut spec {
            // TODO: wrap this in a nicer api
            api::Spec::V0Package(spec) => {
                spec.pkg = spec.pkg.with_build(Some(api::Build::Source));
            }
        }
        Self {
            spec,
            repo: None,
            prefix: PathBuf::from("/spfs"),
        }
    }

    /// Set the repository that the created package should be published to.
    pub fn with_target_repository(
        &mut self,
        repo: impl Into<Arc<storage::RepositoryHandle>>,
    ) -> &mut Self {
        self.repo = Some(repo.into());
        self
    }

    /// Build the requested source package.
    pub async fn build(&mut self) -> Result<api::BuildIdent> {
        let layer = self.collect_and_commit_sources().await?;
        let repo = match &mut self.repo {
            Some(r) => r,
            None => {
                let repo = storage::local_repository().await?;
                self.repo.insert(Arc::new(repo.into()))
            }
        };
        // Capture the repository name we published the source package to into
        // the BuildIdent so it will be resolved later from the same repo and not
        // unexpectedly from some other repo.
        let pkg = self
            .spec
            .ident()
            .clone()
            .try_into_build_ident(repo.name().to_owned())?;
        let mut components = std::collections::HashMap::with_capacity(1);
        components.insert(api::Component::Source, layer.digest()?);
        repo.publish_package(&self.spec, components).await?;
        Ok(pkg)
    }

    /// Collect sources for the given spec and commit them into an spfs layer.
    async fn collect_and_commit_sources(&self) -> Result<spfs::graph::Layer> {
        let mut runtime = spfs::active_runtime().await?;
        let config = spfs::get_config()?;
        let repo = config.get_local_repository_handle().await?;
        runtime.reset_all()?;
        runtime.status.editable = true;
        runtime.status.stack.clear();
        runtime.save_state_to_storage().await?;
        spfs::remount_runtime(&runtime).await?;

        let source_dir = data_path(self.spec.ident()).to_path(&self.prefix);
        collect_sources(&self.spec, &source_dir)?;

        tracing::info!("Validating source package contents...");
        let diffs = spfs::diff(None, None).await?;
        validate_source_changeset(
            diffs,
            RelativePathBuf::from(source_dir.to_string_lossy().to_string()),
        )?;

        tracing::info!("Committing source package contents...");
        Ok(spfs::commit_layer(&mut runtime, repo.into()).await?)
    }
}

/// Collect the sources for a spec in the given directory.
pub(super) fn collect_sources<P: AsRef<Path>>(spec: &api::Spec, source_dir: P) -> Result<()> {
    let source_dir = source_dir.as_ref();
    std::fs::create_dir_all(&source_dir)?;

    let env = super::binary::get_package_build_env(spec);
    for source in spec.sources().iter() {
        let target_dir = match source.subdir() {
            Some(subdir) => subdir.to_path(source_dir),
            None => source_dir.into(),
        };
        std::fs::create_dir_all(&target_dir)?;
        source.collect(&target_dir, &env).map_err(|err| {
            CollectionError::new_error(format_args!(
                "Failed to collect source: {}\n{:?}",
                err, source
            ))
        })?;
    }
    Ok(())
}

/// Validate the set of diffs for a source package build.
///
/// # Errors:
///   - CollectionError: if any issues are identified in the changeset
pub fn validate_source_changeset<P: AsRef<RelativePath>>(
    diffs: Vec<spfs::tracking::Diff>,
    source_dir: P,
) -> Result<()> {
    if diffs.is_empty() {
        return Err(CollectionError::new_error(format_args!(
            "No source files collected, source package would be empty"
        )));
    }

    let mut source_dir = source_dir.as_ref();
    source_dir = source_dir.strip_prefix("/spfs").unwrap_or(source_dir);
    for diff in diffs.into_iter() {
        if diff.mode.is_unchanged() {
            continue;
        }
        if diff.path.starts_with(&source_dir) {
            // the change is within the source directory
            continue;
        }
        if source_dir.starts_with(&diff.path) {
            // the path is to a parent directory of the source path
            continue;
        }
        return Err(CollectionError::new_error(format_args!(
            "Invalid source file path found: {} (not under {})",
            &diff.path, source_dir
        )));
    }
    Ok(())
}
