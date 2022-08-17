// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use relative_path::RelativePathBuf;
use spfs::prelude::*;
use spk_env::data_path;
use spk_exec::resolve_runtime_layers;
use spk_foundation::ident_build::Build;
use spk_foundation::spec_ops::{ComponentOps, PackageOps};
use spk_ident::{Ident, PkgRequest, PreReleasePolicy, RangeIdent, RequestedBy};
use spk_ident_component::Component;
use spk_ident_ops::MetadataPath;
use spk_name::OptNameBuf;
use spk_option_map::OptionMap;
use spk_solver::{BoxedResolverCallback, DefaultResolver, ResolverCallback, Solver};
use spk_solver_graph::Graph;
use spk_solver_solution::Solution;
use spk_spec::{ComponentSpecList, Package};
use spk_storage::{self as storage};
use spk_version::VERSION_SEP;

use crate::{Error, Result};

#[cfg(test)]
#[path = "./binary_test.rs"]
mod binary_test;

/// Denotes an error during the build process.
#[derive(Debug, thiserror::Error)]
#[error("Build error: {message}")]
pub struct BuildError {
    pub message: String,
}

impl BuildError {
    pub fn new_error(format_args: std::fmt::Arguments) -> crate::Error {
        crate::Error::Build(Self {
            message: std::fmt::format(format_args),
        })
    }
}

/// Identifies the source files that should be used
/// in a binary package build
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildSource {
    /// Identifies an existing source package to be resolved
    SourcePackage(RangeIdent),
    /// Specifies that the binary package should be built
    /// against a set of local files.
    ///
    /// Source packages are preferred, but this variant
    /// is useful when rapidly modifying and testing against
    /// a local codebase
    LocalPath(PathBuf),
}

/// Builds a binary package.
///
/// ```no_run
/// # #[macro_use] extern crate spk_name;
/// # #[macro_use] extern crate spk_spec;
/// # async fn demo() {
/// spk_build::BinaryPackageBuilder::from_recipe(recipe!({
///         "pkg": "my-pkg",
///         "build": {"script": "echo hello, world"},
///      }))
///     .with_option(opt_name!("debug"), "true")
///     .build()
///     .await
///     .unwrap();
/// # }
/// ```
pub struct BinaryPackageBuilder<'a, Recipe> {
    prefix: PathBuf,
    recipe: Recipe,
    inputs: OptionMap,
    source: BuildSource,
    solver: Solver,
    environment: HashMap<String, String>,
    source_resolver: BoxedResolverCallback<'a>,
    build_resolver: BoxedResolverCallback<'a>,
    last_solve_graph: Arc<tokio::sync::RwLock<Graph>>,
    repos: Vec<Arc<storage::RepositoryHandle>>,
    interactive: bool,
}

impl<'a, Recipe> BinaryPackageBuilder<'a, Recipe>
where
    Recipe: spk_spec::Recipe<Ident = Ident>,
    Recipe::Output: Package<Ident = Ident> + serde::Serialize,
    <Recipe::Output as PackageOps>::Ident: MetadataPath,
    <Recipe::Output as PackageOps>::Component: ComponentOps,
{
    /// Create a new builder that builds a binary package from the given recipe
    pub fn from_recipe(recipe: Recipe) -> Self {
        let source = BuildSource::SourcePackage(recipe.to_ident().into_build(Build::Source).into());
        Self {
            recipe,
            source,
            prefix: PathBuf::from("/spfs"),
            inputs: OptionMap::default(),
            solver: Solver::default(),
            environment: Default::default(),
            source_resolver: Box::new(DefaultResolver {}),
            build_resolver: Box::new(DefaultResolver {}),
            last_solve_graph: Arc::new(tokio::sync::RwLock::new(Graph::new())),
            repos: Default::default(),
            interactive: false,
        }
    }

    /// Use an alternate prefix when building (not /spfs).
    ///
    /// This is not something that can usually be done well in a
    /// production context, but can be valuable when testing and
    /// in abnormal circumstances.
    pub fn with_prefix(&mut self, prefix: PathBuf) -> &mut Self {
        self.prefix = prefix;
        self
    }

    /// Update a single build option value
    ///
    /// These options are used when computing the final options
    /// for the binary package, and may affect many aspect of the build
    /// environment and generated package.
    pub fn with_option<N, V>(&mut self, name: N, value: V) -> &mut Self
    where
        N: Into<OptNameBuf>,
        V: Into<String>,
    {
        self.inputs.insert(name.into(), value.into());
        self
    }

    /// Update the build options with all of the provided ones
    ///
    /// These options are used when computing the final options
    /// for the binary package, and may affect many aspect of the build
    /// environment and generated package.
    pub fn with_options(&mut self, options: OptionMap) -> &mut Self {
        self.inputs.extend(options.into_iter());
        self
    }

    /// Define the source files that this build should run against
    pub fn with_source(&mut self, source: BuildSource) -> &mut Self {
        self.source = source;
        self
    }

    /// Use the given repository when resolving source and build environment packages
    pub fn with_repository(&mut self, repo: Arc<storage::RepositoryHandle>) -> &mut Self {
        self.repos.push(repo);
        self
    }

    /// Use the given repositories when resolving source and build environment packages
    pub fn with_repositories(
        &mut self,
        repos: impl IntoIterator<Item = Arc<storage::RepositoryHandle>>,
    ) -> &mut Self {
        self.repos.extend(repos);
        self
    }

    /// Provide a function that will be called when resolving the source package.
    ///
    /// This function should run the provided solver runtime to
    /// completion, returning the final result. This function
    /// is useful for introspecting and reporting on the solve
    /// process as needed.
    pub fn with_source_resolver<F>(&mut self, resolver: F) -> &mut Self
    where
        F: ResolverCallback + 'a,
    {
        self.source_resolver = Box::new(resolver);
        self
    }

    /// Provide a function that will be called when resolving the build environment.
    ///
    /// This function should run the provided solver runtime to
    /// completion, returning the final result. This function
    /// is useful for introspecting and reporting on the solve
    /// process as needed.
    pub fn with_build_resolver<F>(&mut self, resolver: F) -> &mut Self
    where
        F: ResolverCallback + 'a,
    {
        self.build_resolver = Box::new(resolver);
        self
    }

    /// Interactive builds stop just before running the build
    /// script and attempt to spawn an interactive shell process
    /// for the user to inspect and debug the build
    pub fn set_interactive(&mut self, interactive: bool) -> &mut Self {
        self.interactive = interactive;
        self
    }

    /// Return the resolve graph from the build environment.
    ///
    /// This is most useful for debugging build environments that failed to resolve,
    /// and builds that failed with a SolverError.
    ///
    /// If the builder has not run, return an incomplete graph.
    pub fn get_solve_graph(&self) -> Arc<tokio::sync::RwLock<Graph>> {
        self.last_solve_graph.clone()
    }

    pub async fn build_and_publish<R, T>(
        &mut self,
        repo: &R,
    ) -> Result<(Recipe::Output, HashMap<Component, spfs::encoding::Digest>)>
    where
        R: std::ops::Deref<Target = T>,
        T: storage::Repository<Recipe = Recipe> + ?Sized,
    {
        let (package, components) = self.build().await?;
        repo.publish_package(&package, &components).await?;
        Ok((package, components))
    }

    /// Build the requested binary package.
    ///
    /// Returns the unpublished package definition and set of components
    /// layers collected in the local spfs repository.
    pub async fn build(
        &mut self,
    ) -> Result<(Recipe::Output, HashMap<Component, spfs::encoding::Digest>)> {
        self.environment.clear();
        let mut runtime = spfs::active_runtime().await?;
        runtime.reset_all()?;
        runtime.status.editable = true;
        runtime.status.stack.clear();

        tracing::debug!("input options: {}", self.inputs);
        let build_options = self.recipe.resolve_options(&self.inputs)?;
        tracing::debug!("build options: {build_options}");
        let mut all_options = self.inputs.clone();
        all_options.extend(build_options.into_iter());

        if let BuildSource::SourcePackage(ident) = self.source.clone() {
            tracing::debug!("Resolving source package for build");
            let solution = self.resolve_source_package(&all_options, ident).await?;
            runtime
                .status
                .stack
                .extend(resolve_runtime_layers(&solution).await?);
        };

        tracing::debug!("Resolving build environment");
        let solution = self.resolve_build_environment(&all_options).await?;
        self.environment
            .extend(solution.to_environment(Some(std::env::vars())));

        let solution = self.resolve_build_environment(&all_options).await?;
        {
            // original options to be reapplied. It feels like this
            // shouldn't be necessary but I've not been able to isolate what
            // goes wrong when this is removed.
            let mut opts = solution.options();
            std::mem::swap(&mut opts, &mut all_options);
            all_options.extend(opts);
        }

        runtime
            .status
            .stack
            .extend(resolve_runtime_layers(&solution).await?);
        runtime.save_state_to_storage().await?;
        spfs::remount_runtime(&runtime).await?;

        let package = self.recipe.generate_binary_build(&all_options, &solution)?;
        let components = self
            .build_and_commit_artifacts(&package, &all_options)
            .await?;
        Ok((package, components))
    }

    async fn resolve_source_package(
        &mut self,
        options: &OptionMap,
        package: RangeIdent,
    ) -> Result<Solution> {
        self.solver.reset();
        self.solver.update_options(options.clone());

        let local_repo =
            async { Ok::<_, crate::Error>(Arc::new(storage::local_repository().await?.into())) };

        // If `package` specifies a repository name, only add the
        // repository that matches.
        if let Some(repo_name) = &package.repository_name {
            if repo_name.is_local() {
                self.solver.add_repository(local_repo.await?);
            } else {
                let mut found = false;
                for repo in self.repos.iter() {
                    if repo_name == repo.name() {
                        self.solver.add_repository(repo.clone());
                        found = true;
                        break;
                    }
                }
                if !found {
                    return Err(Error::String(format!(
                        "Repository not found (or enabled) for {package}",
                    )));
                }
            }
        } else {
            // `package` has no opinion about what repo to use.
            let local_repo = local_repo.await?;
            self.solver.add_repository(local_repo.clone());
            for repo in self.repos.iter() {
                if **repo == *local_repo {
                    // local repo is always injected first, and duplicates are redundant
                    continue;
                }
                self.solver.add_repository(repo.clone());
            }
        }

        let source_build = RequestedBy::SourceBuild(package.clone().try_into()?);
        let ident_range = package.with_components([Component::Source]);
        let request: PkgRequest = PkgRequest::new(ident_range, source_build)
            .with_prerelease(PreReleasePolicy::IncludeAll)
            .with_pin(None)
            .with_compat(None);

        self.solver.add_request(request.into());

        let mut runtime = self.solver.run();
        let solution = self.source_resolver.solve(&mut runtime).await;
        self.last_solve_graph = runtime.graph();
        Ok(solution?)
    }

    async fn resolve_build_environment(&mut self, options: &OptionMap) -> Result<Solution> {
        self.solver.reset();
        self.solver.update_options(options.clone());
        self.solver.set_binary_only(true);
        for repo in self.repos.iter().cloned() {
            self.solver.add_repository(repo);
        }

        for request in self.recipe.get_build_requirements(options)? {
            self.solver.add_request(request);
        }

        let mut runtime = self.solver.run();
        let solution = self.build_resolver.solve(&mut runtime).await;
        self.last_solve_graph = runtime.graph();
        Ok(solution?)
    }

    async fn build_and_commit_artifacts(
        &mut self,
        package: &Recipe::Output,
        options: &OptionMap,
    ) -> Result<HashMap<Component, spfs::encoding::Digest>> {
        self.build_artifacts(package, options).await?;

        let source_ident = Ident {
            name: self.recipe.name().to_owned(),
            version: self.recipe.version().clone(),
            build: Some(Build::Source),
        };
        let sources_dir = data_path(&source_ident);

        let mut runtime = spfs::active_runtime().await?;
        let pattern = sources_dir.join("**").to_string();
        tracing::info!(
            "Purging all changes made to source directory: {}",
            sources_dir.to_path(&self.prefix).display()
        );
        runtime.reset(&[pattern])?;
        runtime.save_state_to_storage().await?;
        spfs::remount_runtime(&runtime).await?;

        tracing::info!("Validating package contents...");
        package
            .validation()
            .validate_build_changeset(package)
            .await
            .map_err(|err| BuildError::new_error(format_args!("{}", err)))?;

        tracing::info!("Committing package contents...");
        commit_component_layers(package, &mut runtime).await
    }

    async fn build_artifacts(
        &mut self,
        package: &Recipe::Output,
        options: &OptionMap,
    ) -> Result<()> {
        let pkg = package.ident();
        let metadata_dir = data_path(pkg).to_path(&self.prefix);
        let build_spec = build_spec_path(pkg).to_path(&self.prefix);
        let build_options = build_options_path(pkg).to_path(&self.prefix);
        let build_script = build_script_path(pkg).to_path(&self.prefix);

        std::fs::create_dir_all(&metadata_dir)?;
        {
            let mut writer = std::fs::File::create(&build_spec)?;
            serde_yaml::to_writer(&mut writer, package)
                .map_err(|err| Error::String(format!("Failed to save build spec: {err}")))?;
            writer.sync_data()?;
        }
        {
            let mut writer = std::fs::File::create(&build_script)?;
            writer
                .write_all(package.build_script().as_bytes())
                .map_err(|err| Error::String(format!("Failed to save build script: {}", err)))?;
            writer.sync_data()?;
        }
        {
            let mut writer = std::fs::File::create(&build_options)?;
            serde_json::to_writer_pretty(&mut writer, &options)
                .map_err(|err| Error::String(format!("Failed to save build options: {}", err)))?;
            writer.sync_data()?;
        }
        for cmpt in package.components().iter() {
            let marker_path = component_marker_path(pkg, &cmpt.name).to_path(&self.prefix);
            std::fs::File::create(marker_path)?;
        }

        let source_dir = match &self.source {
            BuildSource::SourcePackage(source) => {
                source_package_path(&source.try_into()?).to_path(&self.prefix)
            }
            BuildSource::LocalPath(path) => path.clone(),
        };

        // force the base environment to be setup using bash, so that the
        // spfs startup and build environment are predictable and consistent
        // (eg in case the user's shell does not have startup scripts in
        //  the dependencies, is not supported by spfs, etc)
        std::env::set_var("SHELL", "bash");
        let runtime = spfs::active_runtime().await?;
        let cmd = if self.interactive {
            println!("\nNow entering an interactive build shell");
            println!(" - your current directory will be set to the sources area");
            println!(" - build and install your artifacts into /spfs");
            println!(
                " - this package's build script can be run from: {}",
                build_script.display()
            );
            println!(" - to cancel and discard this build, run `exit 1`");
            println!(" - to finalize and save the package, run `exit 0`");
            spfs::build_interactive_shell_command(&runtime)?
        } else {
            use std::ffi::OsString;
            spfs::build_shell_initialized_command(
                &runtime,
                OsString::from("bash"),
                &[OsString::from("-ex"), build_script.into_os_string()],
            )?
        };

        let mut cmd = cmd.into_std();
        cmd.envs(self.environment.drain());
        cmd.envs(options.to_environment());
        cmd.envs(get_package_build_env(package));
        cmd.env("PREFIX", &self.prefix);
        cmd.current_dir(&source_dir);

        match cmd.status()?.code() {
            Some(0) => (),
            Some(code) => {
                return Err(BuildError::new_error(format_args!(
                    "Build script returned non-zero exit status: {}",
                    code
                )))
            }
            None => {
                return Err(BuildError::new_error(format_args!(
                    "Build script failed unexpectedly"
                )))
            }
        }
        self.generate_startup_scripts(package)
    }

    fn generate_startup_scripts(&self, package: &impl Package) -> Result<()> {
        let ops = package.runtime_environment();
        if ops.is_empty() {
            return Ok(());
        }

        let startup_dir = self.prefix.join("etc").join("spfs").join("startup.d");
        if let Err(err) = std::fs::create_dir_all(&startup_dir) {
            match err.kind() {
                std::io::ErrorKind::AlreadyExists => (),
                _ => return Err(err.into()),
            }
        }

        let startup_file_csh = startup_dir.join(format!("spk_{}.csh", package.name()));
        let startup_file_sh = startup_dir.join(format!("spk_{}.sh", package.name()));
        let mut csh_file = std::fs::File::create(startup_file_csh)?;
        let mut sh_file = std::fs::File::create(startup_file_sh)?;
        for op in ops {
            csh_file.write_fmt(format_args!("{}\n", op.tcsh_source()))?;
            sh_file.write_fmt(format_args!("{}\n", op.bash_source()))?;
        }
        Ok(())
    }
}

/// Return the environment variables to be set for a build of the given package spec.
pub fn get_package_build_env<P>(spec: &P) -> HashMap<String, String>
where
    P: Package<Ident = Ident>,
{
    let mut env = HashMap::with_capacity(8);
    env.insert("SPK_PKG".to_string(), spec.ident().to_string());
    env.insert("SPK_PKG_NAME".to_string(), spec.name().to_string());
    env.insert("SPK_PKG_VERSION".to_string(), spec.version().to_string());
    env.insert(
        "SPK_PKG_BUILD".to_string(),
        spec.ident()
            .build
            .as_ref()
            .map(Build::to_string)
            .unwrap_or_default(),
    );
    env.insert(
        "SPK_PKG_VERSION_MAJOR".to_string(),
        spec.version().major().to_string(),
    );
    env.insert(
        "SPK_PKG_VERSION_MINOR".to_string(),
        spec.version().minor().to_string(),
    );
    env.insert(
        "SPK_PKG_VERSION_PATCH".to_string(),
        spec.version().patch().to_string(),
    );
    env.insert(
        "SPK_PKG_VERSION_BASE".to_string(),
        spec.version()
            .parts
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(VERSION_SEP),
    );
    env
}

pub async fn commit_component_layers<P>(
    package: &P,
    runtime: &mut spfs::runtime::Runtime,
) -> Result<HashMap<Component, spfs::encoding::Digest>>
where
    P: Package<Ident = Ident>,
{
    let config = spfs::get_config()?;
    let repo = Arc::new(config.get_local_repository_handle().await?);
    let layer = spfs::commit_layer(runtime, Arc::clone(&repo)).await?;
    let manifest = repo.read_manifest(layer.manifest).await?.unlock();
    let manifests = split_manifest_by_component(package.ident(), &manifest, package.components())?;
    let mut committed = HashMap::with_capacity(manifests.len());
    for (component, manifest) in manifests {
        let manifest = spfs::graph::Manifest::from(&manifest);
        let layer = spfs::graph::Layer {
            manifest: manifest.digest().unwrap(),
        };
        let layer_digest = layer.digest().unwrap();
        #[rustfmt::skip]
        tokio::try_join!(
            async { repo.write_object(&manifest.into()).await },
            async { repo.write_object(&layer.into()).await }
        )?;
        committed.insert(component, layer_digest);
    }
    Ok(committed)
}

fn split_manifest_by_component(
    pkg: &Ident,
    manifest: &spfs::tracking::Manifest,
    components: &ComponentSpecList,
) -> Result<HashMap<Component, spfs::tracking::Manifest>> {
    let mut manifests = HashMap::with_capacity(components.len());
    for component in components.iter() {
        let mut component_manifest = spfs::tracking::Manifest::default();

        // identify all the file paths that we will replicate
        // first so that we can also identify necessary
        // parent directories in a second iteration
        let mut relevant_paths: HashSet<relative_path::RelativePathBuf> = Default::default();
        // all components must include the package metadata
        // as well as the marker file for itself
        relevant_paths.insert(build_spec_path(pkg));
        relevant_paths.insert(build_options_path(pkg));
        relevant_paths.insert(build_script_path(pkg));
        relevant_paths.insert(component_marker_path(pkg, &component.name));
        relevant_paths.extend(path_and_parents(data_path(pkg)));
        for node in manifest.walk() {
            if node.path.strip_prefix(data_path(pkg)).is_ok() {
                // paths within the metadata directory are controlled
                // separately and cannot be included by the component spec
                continue;
            }
            if component
                .files
                .matches(&node.path.to_path("/"), node.entry.is_dir())
            {
                relevant_paths.extend(path_and_parents(node.path.to_owned()));
            }
        }
        for node in manifest.walk() {
            if relevant_paths.contains(&node.path) {
                tracing::debug!("{}:{} collecting {:?}", pkg.name, component.name, node.path);
                let mut entry = node.entry.clone();
                if entry.is_dir() {
                    // we will be building back up any directory with
                    // only the children that is should have, so start
                    // with an empty one
                    entry.entries.clear();
                }
                component_manifest.mknod(&node.path, entry)?;
            }
        }

        manifests.insert(component.name.clone(), component_manifest);
    }
    Ok(manifests)
}

/// Return the file path for the given source package's files.
pub fn source_package_path(pkg: &Ident) -> RelativePathBuf {
    data_path(pkg)
}

/// Return the file path for the given build's spec.yaml file.
///
/// This file is created during a build and stores the full
/// package spec of what was built.
pub fn build_spec_path(pkg: &Ident) -> RelativePathBuf {
    data_path(pkg).join("spec.yaml")
}

/// Return the file path for the given build's options.json file.
///
/// This file is created during a build and stores the set
/// of build options used when creating the package
pub fn build_options_path(pkg: &Ident) -> RelativePathBuf {
    data_path(pkg).join("options.json")
}

/// Return the file path for the given build's build.sh file.
///
/// This file is created during a build and stores the bash
/// script used to build the package contents
pub fn build_script_path(pkg: &Ident) -> RelativePathBuf {
    data_path(pkg).join("build.sh")
}

/// Return the file path for the given build's build.sh file.
///
/// This file is created during a build and stores the bash
/// script used to build the package contents
pub fn component_marker_path(pkg: &Ident, name: &Component) -> RelativePathBuf {
    data_path(pkg).join(format!("{}.cmpt", name))
}

/// Expand a path to a list of itself and all of its parents
fn path_and_parents(mut path: RelativePathBuf) -> Vec<RelativePathBuf> {
    let mut hierarchy = Vec::new();
    loop {
        let parent = path.parent().map(ToOwned::to_owned);
        hierarchy.push(path);
        match parent {
            Some(parent) if !parent.as_str().is_empty() => {
                path = parent;
            }
            _ => break,
        }
    }
    hierarchy
}
