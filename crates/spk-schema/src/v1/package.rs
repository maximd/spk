// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use serde::{Deserialize, Serialize};
use spk_schema_ident::BuildIdent;

use crate::foundation::ident_build::Build;
use crate::foundation::ident_component::Component;
use crate::foundation::name::PkgName;
use crate::foundation::option_map::OptionMap;
use crate::foundation::spec_ops::prelude::*;
use crate::foundation::version::{Compat, Compatibility, Version};
use crate::ident::{is_false, PkgRequest, Satisfy, VarRequest};
use crate::meta::Meta;
use crate::{
    ComponentSpecList,
    Deprecate,
    DeprecateMut,
    EmbeddedPackagesList,
    EnvOp,
    Opt,
    PackageMut,
    RequirementsList,
    Result,
    SourceSpec,
    ValidationSpec,
};

#[cfg(test)]
#[path = "./package_test.rs"]
mod package_test;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct Package {
    pub pkg: BuildIdent,
    #[serde(default, skip_serializing_if = "Meta::is_default")]
    pub meta: Meta,
    #[serde(default, skip_serializing_if = "Compat::is_default")]
    pub compat: Compat,
    #[serde(default, skip_serializing_if = "is_false")]
    pub deprecated: bool,
}

impl Package {
    /// Create an empty spec for the identified package
    pub fn new(ident: BuildIdent) -> Self {
        Self {
            pkg: ident,
            meta: Meta::default(),
            compat: Compat::default(),
            deprecated: bool::default(),
        }
    }
}

impl Named for Package {
    fn name(&self) -> &PkgName {
        self.pkg.name()
    }
}

impl HasVersion for Package {
    fn version(&self) -> &Version {
        self.pkg.version()
    }
}

impl Versioned for Package {
    fn compat(&self) -> &Compat {
        &self.compat
    }
}

impl HasBuild for Package {
    fn build(&self) -> &Build {
        self.pkg.build()
    }
}

impl Deprecate for Package {
    fn is_deprecated(&self) -> bool {
        self.deprecated
    }
}

impl DeprecateMut for Package {
    fn deprecate(&mut self) -> Result<()> {
        self.deprecated = true;
        Ok(())
    }

    fn undeprecate(&mut self) -> Result<()> {
        self.deprecated = false;
        Ok(())
    }
}

impl crate::Package for Package {
    type Package = Self;

    fn ident(&self) -> &BuildIdent {
        &self.pkg
    }

    fn option_values(&self) -> OptionMap {
        todo!()
    }

    fn options(&self) -> &Vec<Opt> {
        todo!()
    }

    fn sources(&self) -> &Vec<SourceSpec> {
        todo!()
    }

    fn embedded(&self) -> &EmbeddedPackagesList {
        todo!()
    }

    fn embedded_as_packages(
        &self,
    ) -> std::result::Result<Vec<(Self::Package, Option<Component>)>, &str> {
        todo!()
    }

    fn components(&self) -> &ComponentSpecList {
        todo!()
    }

    fn runtime_environment(&self) -> &Vec<EnvOp> {
        todo!()
    }

    fn runtime_requirements(&self) -> &RequirementsList {
        todo!()
    }

    fn validation(&self) -> &ValidationSpec {
        todo!()
    }

    fn build_script(&self) -> String {
        todo!()
    }
}

impl PackageMut for Package {
    fn set_build(&mut self, build: Build) {
        self.pkg.set_target(build);
    }
}

impl Satisfy<PkgRequest> for Package {
    fn check_satisfies_request(&self, _pkg_request: &PkgRequest) -> Compatibility {
        todo!()
    }
}

impl Satisfy<VarRequest> for Package {
    fn check_satisfies_request(&self, _var_request: &VarRequest) -> Compatibility {
        todo!()
    }
}