// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::io::Read;
use std::path::Path;
use std::str::FromStr;

use enum_dispatch::enum_dispatch;
use format_serde_error::SerdeError;
use serde::{Deserialize, Serialize};
use spk_schema_foundation::ident_component::Component;
use spk_schema_foundation::spec_ops::PackageMutOps;

use crate::foundation::name::{PkgName, PkgNameBuf};
use crate::foundation::option_map::OptionMap;
use crate::foundation::spec_ops::{Named, PackageOps, RecipeOps, Versioned};
use crate::foundation::version::{Compat, Compatibility, Version};
use crate::ident::{Ident, PkgRequest, RangeIdent, Request, VarRequest};
use crate::{test_spec::TestSpec, Deprecate, DeprecateMut, Error, Package, Result};
use crate::{BuildEnv, ComponentSpec, FromYaml, Recipe, Template, TemplateExt};

#[cfg(test)]
#[path = "./spec_test.rs"]
mod spec_test;

/// Create a spec recipe from a json structure.
///
/// This will panic if the given struct
/// cannot be deserialized into a recipe.
///
/// ```
/// # #[macro_use] extern crate spk_schema;
/// # fn main() {
/// recipe!({
///   "api": "v0/package",
///   "pkg": "my-pkg/1.0.0",
///   "build": {
///     "options": [
///       {"pkg": "dependency"}
///     ]
///   }
/// });
/// # }
/// ```
#[macro_export]
macro_rules! recipe {
    ($($spec:tt)+) => {{
        use $crate::FromYaml;
        let value = $crate::serde_json::json!($($spec)+);
        let spec = $crate::SpecRecipe::from_yaml(value.to_string()).expect("invalid recipe data");
        spec
    }};
}

/// Create a spec from a json structure.
///
/// This will panic if the given struct
/// cannot be deserialized into a spec.
///
/// ```
/// # #[macro_use] extern crate spk_schema;
/// # fn main() {
/// spec!({
///   "api": "v0/package",
///   "pkg": "my-pkg/1.0.0",
///   "build": {
///     "options": [
///       {"pkg": "dependency"}
///     ]
///   }
/// });
/// # }
/// ```
#[macro_export]
macro_rules! spec {
    ($($spec:tt)+) => {{
        use $crate::FromYaml;
        let value = $crate::serde_json::json!($($spec)+);
        let spec = $crate::Spec::from_yaml(value.to_string()).expect("invalid spec");
        spec
    }};
}

/// A generic, structured data object that can be turned into a recipe
/// when provided with the necessary option values
pub struct SpecTemplate {
    name: PkgNameBuf,
    file_path: std::path::PathBuf,
    template: String,
}

impl Named for SpecTemplate {
    fn name(&self) -> &PkgName {
        &self.name
    }
}

impl Template for SpecTemplate {
    type Output = SpecRecipe;

    fn file_path(&self) -> &Path {
        &self.file_path
    }

    fn render(&self, options: &OptionMap) -> Result<Self::Output> {
        let name = self.file_path.display().to_string();
        let rendered = spk_schema_handlebars::render_template(name, &self.template, &options)
            .map_err(|err| {
                let custom_err = format_serde_error::ErrorTypes::Custom {
                    line: err.line_no,
                    column: err.column_no,
                    error: err.into(),
                };
                Error::InvalidTemplate(format_serde_error::SerdeError::new(
                    self.template.clone(),
                    custom_err,
                ))
            })?;
        Ok(SpecRecipe::from_yaml(&rendered)?)
    }
}

impl TemplateExt for SpecTemplate {
    fn from_file(path: &Path) -> Result<Self> {
        let file_path = path
            .canonicalize()
            .map_err(|err| Error::InvalidPath(path.to_owned(), err))?;
        let file = std::fs::File::open(&file_path)
            .map_err(|err| Error::FileOpenError(file_path.to_owned(), err))?;
        let mut template = String::new();
        std::io::BufReader::new(file)
            .read_to_string(&mut template)
            .map_err(|err| Error::String(format!("Failed to read file {path:?}: {err}")))?;

        // validate that the template is still a valid yaml mapping even
        // though we will need to re-process it again later on
        let template_value: serde_yaml::Mapping = match serde_yaml::from_str(&template) {
            Err(err) => return Err(Error::InvalidYaml(SerdeError::new(template, err))),
            Ok(v) => v,
        };

        let pkg = template_value
            .get(&serde_yaml::Value::String("pkg".to_string()))
            .ok_or_else(|| {
                crate::Error::String(format!("Missing pkg field in spec file: {file_path:?}"))
            })?;
        let pkg = pkg.as_str().ok_or_else(|| {
            crate::Error::String(format!(
                "Invalid value for 'pkg' field: expected string, got {pkg:?} in {file_path:?}"
            ))
        })?;
        let name = PkgNameBuf::from_str(
            // it should never be possible for split to return 0 results
            // but this trick avoids the use of unwrap
            pkg.split('/').next().unwrap_or(pkg),
        )?;

        if template_value
            .get(&serde_yaml::Value::String("api".to_string()))
            .is_none()
        {
            tracing::warn!(
                "Spec file is missing the 'api' field, this may be an error in the future"
            );
            tracing::warn!(" > for specs in the original spk format, add 'api: v0/package'");
        }

        Ok(Self {
            file_path,
            name,
            template,
        })
    }
}

/// Specifies some buildable object within the spk ecosystem.
///
/// All build-able types have a recipe representation
/// that can be serialized and deserialized from a human-written
/// file or machine-managed persistent storage.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(tag = "api")]
#[enum_dispatch(Deprecate, DeprecateMut)]
pub enum SpecRecipe {
    #[serde(rename = "v0/package")]
    V0Package(super::v0::Spec),
}

impl RecipeOps for SpecRecipe {
    type Ident = Ident;
    type PkgRequest = PkgRequest;
    type RangeIdent = RangeIdent;

    fn is_api_compatible(&self, base: &Version) -> Compatibility {
        match self {
            SpecRecipe::V0Package(r) => r.is_api_compatible(base),
        }
    }

    fn is_binary_compatible(&self, base: &Version) -> Compatibility {
        match self {
            SpecRecipe::V0Package(r) => r.is_binary_compatible(base),
        }
    }

    fn is_satisfied_by_range_ident(
        &self,
        range_ident: &Self::RangeIdent,
        required: crate::foundation::version::CompatRule,
    ) -> Compatibility {
        match self {
            SpecRecipe::V0Package(r) => r.is_satisfied_by_range_ident(range_ident, required),
        }
    }

    fn is_satisfied_by_pkg_request(&self, pkg_request: &Self::PkgRequest) -> Compatibility {
        match self {
            SpecRecipe::V0Package(r) => r.is_satisfied_by_pkg_request(pkg_request),
        }
    }

    fn to_ident(&self) -> Self::Ident {
        match self {
            SpecRecipe::V0Package(r) => r.to_ident(),
        }
    }
}

impl Recipe for SpecRecipe {
    type Output = Spec;

    fn default_variants(&self) -> &Vec<OptionMap> {
        match self {
            SpecRecipe::V0Package(r) => r.default_variants(),
        }
    }

    fn resolve_options(&self, inputs: &OptionMap) -> Result<OptionMap> {
        match self {
            SpecRecipe::V0Package(r) => r.resolve_options(inputs),
        }
    }

    fn get_build_requirements(&self, options: &OptionMap) -> Result<Vec<Request>> {
        match self {
            SpecRecipe::V0Package(r) => r.get_build_requirements(options),
        }
    }

    fn get_tests(&self, options: &OptionMap) -> Result<Vec<TestSpec>> {
        match self {
            SpecRecipe::V0Package(r) => r.get_tests(options),
        }
    }

    fn generate_source_build(&self, root: &Path) -> Result<Self::Output> {
        match self {
            SpecRecipe::V0Package(r) => r.generate_source_build(root).map(Spec::V0Package),
        }
    }

    fn generate_binary_build<E, P>(
        &self,
        options: &OptionMap,
        build_env: &E,
    ) -> Result<Self::Output>
    where
        E: BuildEnv<Package = P>,
        P: Package<Ident = Ident>,
    {
        match self {
            SpecRecipe::V0Package(r) => r
                .generate_binary_build(options, build_env)
                .map(Spec::V0Package),
        }
    }
}

impl PackageOps for SpecRecipe {
    type Ident = Ident;
    type Component = ComponentSpec;
    type VarRequest = VarRequest;

    fn components_iter(&self) -> std::slice::Iter<'_, Self::Component> {
        match self {
            SpecRecipe::V0Package(r) => r.components_iter(),
        }
    }

    fn ident(&self) -> &Self::Ident {
        match self {
            SpecRecipe::V0Package(r) => r.ident(),
        }
    }

    fn is_satisfied_by_var_request(&self, var_request: &Self::VarRequest) -> Compatibility {
        match self {
            SpecRecipe::V0Package(r) => r.is_satisfied_by_var_request(var_request),
        }
    }
}

impl Named for SpecRecipe {
    fn name(&self) -> &PkgName {
        match self {
            SpecRecipe::V0Package(r) => r.name(),
        }
    }
}

impl Versioned for SpecRecipe {
    fn version(&self) -> &Version {
        match self {
            SpecRecipe::V0Package(r) => r.version(),
        }
    }
}

impl FromYaml for SpecRecipe {
    fn from_yaml<S: Into<String>>(yaml: S) -> std::result::Result<Self, SerdeError> {
        let yaml = yaml.into();

        // unfortunately, serde does not have a derive mechanism which
        // would allow us to specify a default enum variant for when
        // the 'api' field does not exist in a spec. To do this properly
        // and still be able to maintain source location data for
        // yaml errors, we need to deserialize twice: once to get the
        // api version, and a second time to deserialize that version

        // the name of this struct appears in error messages when the
        // root of the yaml doc is not a mapping, so we use something
        // fairly generic, eg: 'expected struct YamlMapping'
        #[derive(Deserialize)]
        struct YamlMapping {
            #[serde(default = "ApiVersion::default")]
            api: ApiVersion,
        }

        let with_version = match serde_yaml::from_str::<YamlMapping>(&yaml) {
            // we cannot simply use map_err because we need the compiler
            // to understand that we only pass ownership of 'yaml' if
            // the function is returning
            Err(err) => return Err(SerdeError::new(yaml, err)),
            Ok(m) => m,
        };

        match with_version.api {
            ApiVersion::V0Package => {
                let inner =
                    serde_yaml::from_str(&yaml).map_err(|err| SerdeError::new(yaml, err))?;
                Ok(Self::V0Package(inner))
            }
        }
    }
}

/// Specifies some data object within the spk ecosystem.
///
/// All resolve-able types have a spec representation that can be serialized
/// and deserialized from a `Repository`.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(tag = "api")]
#[enum_dispatch(Deprecate, DeprecateMut)]
pub enum Spec {
    #[serde(rename = "v0/package")]
    V0Package(super::v0::Spec),
}

impl RecipeOps for Spec {
    type Ident = Ident;
    type PkgRequest = PkgRequest;
    type RangeIdent = RangeIdent;

    fn is_api_compatible(&self, base: &Version) -> Compatibility {
        match self {
            Spec::V0Package(r) => RecipeOps::is_api_compatible(r, base),
        }
    }

    fn is_binary_compatible(&self, base: &Version) -> Compatibility {
        match self {
            Spec::V0Package(r) => RecipeOps::is_binary_compatible(r, base),
        }
    }

    fn is_satisfied_by_range_ident(
        &self,
        range_ident: &Self::RangeIdent,
        required: crate::foundation::version::CompatRule,
    ) -> Compatibility {
        match self {
            Spec::V0Package(r) => RecipeOps::is_satisfied_by_range_ident(r, range_ident, required),
        }
    }

    fn is_satisfied_by_pkg_request(&self, pkg_request: &Self::PkgRequest) -> Compatibility {
        match self {
            Spec::V0Package(r) => RecipeOps::is_satisfied_by_pkg_request(r, pkg_request),
        }
    }

    fn to_ident(&self) -> Self::Ident {
        match self {
            Spec::V0Package(r) => RecipeOps::to_ident(r),
        }
    }
}

impl Recipe for Spec {
    type Output = Spec;

    fn default_variants(&self) -> &Vec<OptionMap> {
        match self {
            Spec::V0Package(r) => r.default_variants(),
        }
    }

    fn resolve_options(&self, inputs: &OptionMap) -> Result<OptionMap> {
        match self {
            Spec::V0Package(r) => r.resolve_options(inputs),
        }
    }

    fn get_build_requirements(&self, options: &OptionMap) -> Result<Vec<Request>> {
        match self {
            Spec::V0Package(r) => r.get_build_requirements(options),
        }
    }

    fn get_tests(&self, options: &OptionMap) -> Result<Vec<TestSpec>> {
        match self {
            Spec::V0Package(r) => r.get_tests(options),
        }
    }

    fn generate_source_build(&self, root: &Path) -> Result<Self::Output> {
        match self {
            Spec::V0Package(r) => r.generate_source_build(root).map(Spec::V0Package),
        }
    }

    fn generate_binary_build<E, P>(
        &self,
        options: &OptionMap,
        build_env: &E,
    ) -> Result<Self::Output>
    where
        E: BuildEnv<Package = P>,
        P: Package<Ident = Ident>,
    {
        match self {
            Spec::V0Package(r) => r
                .generate_binary_build(options, build_env)
                .map(Spec::V0Package),
        }
    }
}

impl PackageOps for Spec {
    type Ident = Ident;
    type Component = ComponentSpec;
    type VarRequest = VarRequest;

    fn components_iter(&self) -> std::slice::Iter<'_, Self::Component> {
        match self {
            Spec::V0Package(r) => PackageOps::components_iter(r),
        }
    }

    fn ident(&self) -> &Self::Ident {
        match self {
            Spec::V0Package(r) => PackageOps::ident(r),
        }
    }

    fn is_satisfied_by_var_request(&self, var_request: &Self::VarRequest) -> Compatibility {
        match self {
            Spec::V0Package(r) => PackageOps::is_satisfied_by_var_request(r, var_request),
        }
    }
}

impl PackageMutOps for Spec {
    type Ident = Ident;

    fn ident_mut(&mut self) -> &mut Self::Ident {
        match self {
            Spec::V0Package(r) => PackageMutOps::ident_mut(r),
        }
    }
}

impl Named for Spec {
    fn name(&self) -> &PkgName {
        match self {
            Spec::V0Package(r) => r.name(),
        }
    }
}

impl Versioned for Spec {
    fn version(&self) -> &Version {
        match self {
            Spec::V0Package(r) => r.version(),
        }
    }
}

// enum_dispatch does not support associated types.
impl Package for Spec {
    type Package = Self;

    fn compat(&self) -> &Compat {
        match self {
            Spec::V0Package(spec) => spec.compat(),
        }
    }

    fn option_values(&self) -> OptionMap {
        match self {
            Spec::V0Package(spec) => spec.option_values(),
        }
    }

    fn options(&self) -> &Vec<super::Opt> {
        match self {
            Spec::V0Package(spec) => spec.options(),
        }
    }

    fn sources(&self) -> &Vec<super::SourceSpec> {
        match self {
            Spec::V0Package(spec) => spec.sources(),
        }
    }

    fn embedded(&self) -> &super::EmbeddedPackagesList {
        match self {
            Spec::V0Package(spec) => spec.embedded(),
        }
    }

    fn embedded_as_packages(
        &self,
    ) -> std::result::Result<Vec<(Self::Package, Option<Component>)>, &str> {
        match self {
            Spec::V0Package(spec) => spec
                .embedded_as_packages()
                .map(|vec| vec.into_iter().map(|(r, c)| (r.into(), c)).collect()),
        }
    }

    fn components(&self) -> &super::ComponentSpecList {
        match self {
            Spec::V0Package(spec) => spec.components(),
        }
    }

    fn runtime_environment(&self) -> &Vec<super::EnvOp> {
        match self {
            Spec::V0Package(spec) => spec.runtime_environment(),
        }
    }

    fn runtime_requirements(&self) -> &super::RequirementsList {
        match self {
            Spec::V0Package(spec) => spec.runtime_requirements(),
        }
    }

    fn validation(&self) -> &super::ValidationSpec {
        match self {
            Spec::V0Package(spec) => spec.validation(),
        }
    }

    fn build_script(&self) -> String {
        match self {
            Spec::V0Package(spec) => spec.build_script(),
        }
    }
}

impl FromYaml for Spec {
    fn from_yaml<S: Into<String>>(yaml: S) -> std::result::Result<Self, SerdeError> {
        let yaml = yaml.into();

        // unfortunately, serde does not have a derive mechanism which
        // would allow us to specify a default enum variant for when
        // the 'api' field does not exist in a spec. To do this properly
        // and still be able to maintain source location data for
        // yaml errors, we need to deserialize twice: once to get the
        // api version, and a second time to deserialize that version

        // the name of this struct appears in error messages when the
        // root of the yaml doc is not a mapping, so we use something
        // fairly generic, eg: 'expected struct YamlMapping'
        #[derive(Deserialize)]
        struct YamlMapping {
            #[serde(default = "ApiVersion::default")]
            api: ApiVersion,
        }

        let with_version = match serde_yaml::from_str::<YamlMapping>(&yaml) {
            // we cannot simply use map_err because we need the compiler
            // to understand that we only pass ownership of 'yaml' if
            // the function is returning
            Err(err) => return Err(SerdeError::new(yaml, err)),
            Ok(m) => m,
        };

        match with_version.api {
            ApiVersion::V0Package => {
                let inner =
                    serde_yaml::from_str(&yaml).map_err(|err| SerdeError::new(yaml, err))?;
                Ok(Self::V0Package(inner))
            }
        }
    }
}

impl AsRef<Spec> for Spec {
    fn as_ref(&self) -> &Spec {
        self
    }
}

#[derive(Deserialize, Serialize, Copy, Clone)]
pub enum ApiVersion {
    #[serde(rename = "v0/package")]
    V0Package,
}

impl Default for ApiVersion {
    fn default() -> Self {
        Self::V0Package
    }
}
