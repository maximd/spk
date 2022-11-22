// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::borrow::Cow;
use std::path::Path;

use serde::{Deserialize, Serialize};
use spk_schema_ident::VersionIdent;

use super::{RecipeBuildSpec, RecipeOptionList, RecipePackagingSpec, RecipeSourceSpec};
use crate::foundation::name::PkgName;
use crate::foundation::option_map::OptionMap;
use crate::foundation::spec_ops::prelude::*;
use crate::foundation::version::{Compat, Compatibility, Version};
use crate::ident::{is_false, PkgRequest, Satisfy, VarRequest};
use crate::meta::Meta;
use crate::{BuildEnv, Deprecate, DeprecateMut, Package, RequirementsList, Result, TestStage};

#[cfg(test)]
#[path = "./recipe_test.rs"]
mod recipe_test;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(test, serde(deny_unknown_fields))]
pub struct Recipe {
    pub pkg: VersionIdent,
    #[serde(default, skip_serializing_if = "Meta::is_default")]
    pub meta: Meta,
    #[serde(default, skip_serializing_if = "Compat::is_default")]
    pub compat: Compat,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deprecated: bool,
    #[serde(default, skip_serializing_if = "RecipeOptionList::is_empty")]
    pub options: RecipeOptionList,
    #[serde(default, skip_serializing_if = "RecipeSourceSpec::is_empty")]
    pub source: RecipeSourceSpec,
    #[serde(default)]
    pub build: RecipeBuildSpec,
    #[serde(default)]
    pub package: RecipePackagingSpec,
}

impl Recipe {
    /// Create an empty spec for the identified package
    pub fn new(ident: VersionIdent) -> Self {
        Self {
            pkg: ident,
            meta: Meta::default(),
            compat: Compat::default(),
            deprecated: bool::default(),
            options: Default::default(),
            source: Default::default(),
            build: Default::default(),
            package: Default::default(),
        }
    }
}

impl Named for Recipe {
    fn name(&self) -> &PkgName {
        self.pkg.name()
    }
}

impl HasVersion for Recipe {
    fn version(&self) -> &Version {
        self.pkg.version()
    }
}

impl Versioned for Recipe {
    fn compat(&self) -> &Compat {
        &self.compat
    }
}

impl Deprecate for Recipe {
    fn is_deprecated(&self) -> bool {
        self.deprecated
    }
}

impl DeprecateMut for Recipe {
    fn deprecate(&mut self) -> Result<()> {
        self.deprecated = true;
        Ok(())
    }

    fn undeprecate(&mut self) -> Result<()> {
        self.deprecated = false;
        Ok(())
    }
}

impl crate::Recipe for Recipe {
    type Output = super::Package;
    type Test = super::TestScript;
    type Variant = super::VariantSpec;

    fn ident(&self) -> &VersionIdent {
        &self.pkg
    }

    fn default_variants(&self) -> Cow<'_, Vec<Self::Variant>> {
        Cow::Borrowed(&self.build.variants)
    }

    fn resolve_options(&self, _given: &OptionMap) -> Result<OptionMap> {
        todo!()
    }

    fn get_build_requirements(&self, _options: &OptionMap) -> Result<Cow<'_, RequirementsList>> {
        todo!()
    }

    fn get_tests(&self, _stage: TestStage, _options: &OptionMap) -> Result<Vec<super::TestScript>> {
        todo!()
    }

    fn generate_source_build(&self, _root: &Path) -> Result<Self::Output> {
        todo!()
    }

    fn generate_binary_build<E, P>(&self, _build_env: &E) -> Result<Self::Output>
    where
        E: BuildEnv<Package = P>,
        P: Package,
    {
        todo!()
    }
}

impl Satisfy<PkgRequest> for Recipe {
    fn check_satisfies_request(&self, _pkg_request: &PkgRequest) -> Compatibility {
        todo!()
    }
}

impl Satisfy<VarRequest> for Recipe {
    fn check_satisfies_request(&self, _var_request: &VarRequest) -> Compatibility {
        todo!()
    }
}
