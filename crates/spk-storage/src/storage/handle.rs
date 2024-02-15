// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

#[cfg(feature = "legacy-spk-version-tags")]
use spk_schema::ident_ops::VerbatimTagStrategy;
use spk_schema::{Spec, SpecRecipe};

use super::Repository;

type Handle = dyn Repository<Recipe = SpecRecipe, Package = Spec>;

#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(clippy::large_enum_variant)]
pub enum RepositoryHandle {
    SPFS(super::SpfsRepository),
    #[cfg(feature = "legacy-spk-version-tags")]
    SPFSWithVerbatimTags(super::SpfsRepository<VerbatimTagStrategy>),
    Mem(super::MemRepository<SpecRecipe>),
    Runtime(super::RuntimeRepository),
}

impl RepositoryHandle {
    /// Create a repository handle to an empty, in-memory repository
    pub fn new_mem() -> Self {
        Self::Mem(Default::default())
    }

    /// Create a repository handle to the active runtime repository
    ///
    /// This is the repository that holds packages which have been
    /// installed into the current spfs runtime.
    pub fn new_runtime() -> Self {
        Self::Runtime(Default::default())
    }

    #[cfg(feature = "legacy-spk-version-tags")]
    pub fn is_spfs(&self) -> bool {
        matches!(self, Self::SPFS(_) | Self::SPFSWithVerbatimTags(_))
    }

    #[cfg(not(feature = "legacy-spk-version-tags"))]
    pub fn is_spfs(&self) -> bool {
        matches!(self, Self::SPFS(_))
    }

    pub fn is_mem(&self) -> bool {
        matches!(self, Self::Mem(_))
    }

    pub fn is_runtime(&self) -> bool {
        matches!(self, Self::Runtime(_))
    }

    pub fn to_repo(self) -> Box<Handle> {
        match self {
            Self::SPFS(repo) => Box::new(repo),
            #[cfg(feature = "legacy-spk-version-tags")]
            Self::SPFSWithVerbatimTags(repo) => Box::new(repo),
            Self::Mem(repo) => Box::new(repo),
            Self::Runtime(repo) => Box::new(repo),
        }
    }
}

impl std::ops::Deref for RepositoryHandle {
    type Target = Handle;

    fn deref(&self) -> &Self::Target {
        match self {
            RepositoryHandle::SPFS(repo) => repo,
            #[cfg(feature = "legacy-spk-version-tags")]
            RepositoryHandle::SPFSWithVerbatimTags(repo) => repo,
            RepositoryHandle::Mem(repo) => repo,
            RepositoryHandle::Runtime(repo) => repo,
        }
    }
}

impl std::ops::DerefMut for RepositoryHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            RepositoryHandle::SPFS(repo) => repo,
            #[cfg(feature = "legacy-spk-version-tags")]
            RepositoryHandle::SPFSWithVerbatimTags(repo) => repo,
            RepositoryHandle::Mem(repo) => repo,
            RepositoryHandle::Runtime(repo) => repo,
        }
    }
}

impl From<super::SpfsRepository> for RepositoryHandle {
    fn from(repo: super::SpfsRepository) -> Self {
        RepositoryHandle::SPFS(repo)
    }
}

impl From<super::MemRepository<SpecRecipe>> for RepositoryHandle {
    fn from(repo: super::MemRepository<SpecRecipe>) -> Self {
        RepositoryHandle::Mem(repo)
    }
}

impl From<super::RuntimeRepository> for RepositoryHandle {
    fn from(repo: super::RuntimeRepository) -> Self {
        RepositoryHandle::Runtime(repo)
    }
}
