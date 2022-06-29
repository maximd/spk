// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk
use std::collections::HashMap;

use crate::{api, Result};

#[cfg(test)]
#[path = "./repository_test.rs"]
mod repository_test;

#[async_trait::async_trait]
pub trait Repository: Sync {
    /// A repository's address should identify it uniquely. It's
    /// expected that two handles to the same logical repository
    /// share an address
    fn address(&self) -> url::Url;

    /// Return the set of known packages in this repo.
    async fn list_packages(&self) -> Result<Vec<api::PkgName>>;

    /// Return the set of versions available for the named package.
    async fn list_package_versions(&self, name: &api::PkgName) -> Result<Vec<api::Version>>;

    /// Return the set of builds for the given package name and version.
    async fn list_package_builds(&self, pkg: &api::Ident) -> Result<Vec<api::Ident>>;

    /// Returns the set of components published for a package build
    async fn list_build_components(&self, pkg: &api::Ident) -> Result<Vec<api::Component>>;

    /// Read a package spec file for the given package, version and optional build.
    ///
    /// # Errors:
    /// - PackageNotFoundError: If the package, version, or build does not exist
    async fn read_spec(&self, pkg: &api::Ident) -> Result<api::Spec>;

    /// Identify the payloads for the identified package's components.
    async fn get_package(
        &self,
        pkg: &api::Ident,
    ) -> Result<HashMap<api::Component, spfs::encoding::Digest>>;

    /// Publish a package spec to this repository.
    ///
    /// The published spec represents all builds of a single version.
    /// The source package, or at least one binary package should be
    /// published as well in order to make the spec usable in environments.
    ///
    /// # Errors:
    /// - VersionExistsError: if the spec version is already present
    async fn publish_spec(&self, spec: api::Spec) -> Result<()>;

    /// Remove a package version from this repository.
    ///
    /// This will not untag builds for this package, but make it unresolvable
    /// and unsearchable. It's recommended that you remove all existing builds
    /// before removing the spec in order to keep the repository clean.
    async fn remove_spec(&self, pkg: &api::Ident) -> Result<()>;

    /// Publish a package spec to this repository.
    ///
    /// Same as 'publish_spec' except that it clobbers any existing
    /// spec at this version
    async fn force_publish_spec(&self, spec: api::Spec) -> Result<()>;

    /// Publish a package to this repository.
    ///
    /// The provided component digests are expected to each identify an spfs
    /// layer which contains properly constructed binary package files and metadata.
    async fn publish_package(
        &self,
        spec: api::Spec,
        components: HashMap<api::Component, spfs::encoding::Digest>,
    ) -> Result<()>;

    /// Remove a package from this repository.
    ///
    /// The given package identifier must identify a full package build.
    async fn remove_package(&self, pkg: &api::Ident) -> Result<()>;

    /// Perform any upgrades that are pending on this repository.
    ///
    /// This will bring the repository up-to-date for the current
    /// spk library version, but may also make it incompatible with
    /// older ones. Upgrades can also take time depending on their
    /// nature and the size of the repository so. Please, take time to
    /// read any release and upgrade notes before invoking this.
    async fn upgrade(&self) -> Result<String> {
        Ok("Nothing to do.".to_string())
    }
}
