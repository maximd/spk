// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::{collections::HashSet, iter::FromIterator, path::Path};

use indicatif::ParallelProgressIterator;
use rayon::prelude::*;

use super::config::load_config;
use crate::{encoding, graph, runtime, storage, tracking, Error, Result};
use encoding::Encodable;
use storage::{ManifestStorage, Repository};

/// Render the given environment in the local repository
///
/// All items in the spec will be merged and rendered, so
/// it's usually best to only include one thing in the spec if
/// building up layers for use in an spfs runtime
pub fn render(spec: &tracking::EnvSpec) -> Result<std::path::PathBuf> {
    use std::os::unix::ffi::OsStrExt;
    let render_cmd = match super::which_spfs("render") {
        Some(cmd) => cmd,
        None => return Err("'spfs-render' command not found in environment".into()),
    };
    let mut cmd = std::process::Command::new(render_cmd);
    cmd.arg(spec.to_string());
    tracing::debug!("{:?}", cmd);
    let output = cmd.output()?;
    let mut bytes = output.stdout.as_slice();
    loop {
        match bytes.strip_suffix(&[b'\n']) {
            Some(b) => bytes = b,
            None => break,
        }
    }
    match output.status.code() {
        Some(0) => Ok(std::path::PathBuf::from(std::ffi::OsStr::from_bytes(bytes))),
        _ => {
            let stderr = std::ffi::OsStr::from_bytes(output.stderr.as_slice());
            Err(format!("render failed:\n{}", stderr.to_string_lossy()).into())
        }
    }
}

/// Render a set of layers into an arbitrary target directory.
///
/// This method runs in the current thread and creates a copy
/// of the desired data in the target directory
pub fn render_into_directory(
    env_spec: &tracking::EnvSpec,
    target: impl AsRef<std::path::Path>,
) -> Result<()> {
    let repo = load_config()?.get_repository()?;
    let mut stack = Vec::new();
    for target in &env_spec.items {
        let target = target.to_string();
        let obj = repo.read_ref(target.as_str())?;
        stack.push(obj.digest()?);
    }
    let layers = resolve_stack_to_layers(stack.iter(), None)?;
    let manifests: Result<Vec<_>> = layers
        .into_iter()
        .map(|layer| repo.read_manifest(&layer.manifest))
        .collect();
    let manifests = manifests?;
    let mut manifest = tracking::Manifest::default();
    for next in manifests.into_iter() {
        manifest.update(&next.unlock());
    }
    let manifest = graph::Manifest::from(&manifest);
    repo.render_manifest_into_dir(&manifest, &target, storage::fs::RenderType::Copy)
}

/// Compute or load the spfs manifest representation for a saved reference.
pub fn compute_manifest<R: AsRef<str>>(reference: R) -> Result<tracking::Manifest> {
    let config = load_config()?;
    let mut repos: Vec<storage::RepositoryHandle> = vec![config.get_repository()?.into()];
    for name in config.list_remote_names() {
        match config.get_remote(&name) {
            Ok(repo) => repos.push(repo),
            Err(err) => {
                tracing::warn!(remote = ?name, "failed to load remote repository");
                tracing::debug!(" > {:?}", err);
            }
        }
    }

    let spec = tracking::TagSpec::parse(reference)?;
    for repo in repos {
        match repo.read_ref(spec.to_string().as_str()) {
            Ok(obj) => return compute_object_manifest(obj, &repo),
            Err(Error::UnknownObject(_)) => continue,
            Err(err) => return Err(err),
        }
    }
    Err(graph::UnknownReferenceError::new(spec.to_string()))
}

pub fn compute_object_manifest(
    obj: graph::Object,
    repo: &storage::RepositoryHandle,
) -> Result<tracking::Manifest> {
    match obj {
        graph::Object::Layer(obj) => Ok(repo.read_manifest(&obj.manifest)?.unlock()),
        graph::Object::Platform(obj) => {
            let layers = resolve_stack_to_layers(obj.stack.iter(), Some(&repo))?;
            let mut manifest = tracking::Manifest::default();
            for layer in layers.iter().rev() {
                let layer_manifest = repo.read_manifest(&layer.manifest)?;
                manifest.update(&layer_manifest.unlock());
            }
            Ok(manifest)
        }
        graph::Object::Manifest(obj) => Ok(obj.unlock()),
        obj => Err(format!("Resolve: Unhandled object of type {:?}", obj.kind()).into()),
    }
}

/// Compile the set of directories to be overlayed for a runtime.
///
/// These are returned as a list, from bottom to top.
pub fn resolve_overlay_dirs(runtime: &runtime::Runtime) -> Result<Vec<std::path::PathBuf>> {
    let config = load_config()?;
    let mut repo = config.get_repository()?.into();
    let mut overlay_dirs = Vec::new();
    let layers = resolve_stack_to_layers(runtime.get_stack().into_iter(), Some(&repo))?;
    let manifests: Result<Vec<_>> = layers
        .into_par_iter()
        .map(|layer| repo.read_manifest(&layer.manifest))
        .collect();
    let mut manifests = manifests?;
    if manifests.len() > config.filesystem.max_layers {
        let to_flatten = manifests.len() - config.filesystem.max_layers as usize;
        tracing::debug!("flattening {} layers into one...", to_flatten);
        let mut manifest = tracking::Manifest::default();
        for next in manifests.drain(0..to_flatten) {
            manifest.update(&next.unlock());
        }
        let manifest = graph::Manifest::from(&manifest);
        // store the newly created manifest so that the render process can read it back
        repo.write_object(&manifest.clone().into())?;
        manifests.insert(0, manifest);
    }

    let renders = repo.renders()?;
    let to_render: HashSet<encoding::Digest> = HashSet::from_iter(
        manifests
            .iter()
            .map(|m| m.digest().unwrap())
            .filter(|digest| !renders.has_rendered_manifest(&digest)),
    );
    if to_render.len() > 0 {
        tracing::info!("{} layers require rendering", to_render.len());

        let style = indicatif::ProgressStyle::default_bar()
            .template("       {msg} [{bar:40}] {pos:>7}/{len:7}")
            .progress_chars("=>-");
        let bar = indicatif::ProgressBar::new(to_render.len() as u64).with_style(style.clone());
        bar.set_message("rendering layers");
        let results: Result<Vec<_>> = to_render
            .into_par_iter()
            .progress_with(bar)
            .map(|manifest| render(&manifest.into()))
            .collect();
        results?;
    }
    for manifest in manifests {
        let rendered_dir = renders.render_manifest(&manifest)?;
        overlay_dirs.push(rendered_dir);
    }

    Ok(overlay_dirs)
}

/// Given a sequence of tags and digests, resolve to the set of underlying layers.
pub fn resolve_stack_to_layers<D: AsRef<encoding::Digest>>(
    stack: impl Iterator<Item = D>,
    mut repo: Option<&storage::RepositoryHandle>,
) -> Result<Vec<graph::Layer>> {
    let owned_handle;
    let repo = match repo.take() {
        Some(repo) => repo,
        None => {
            let config = load_config()?;
            owned_handle = storage::RepositoryHandle::from(config.get_repository()?);
            &owned_handle
        }
    };

    let mut layers = Vec::new();
    for reference in stack {
        let reference = reference.as_ref();
        let entry = repo.read_ref(reference.to_string().as_str())?;
        match entry {
            graph::Object::Layer(layer) => layers.push(layer),
            graph::Object::Platform(platform) => {
                let mut expanded =
                    resolve_stack_to_layers(platform.stack.clone().into_iter(), Some(repo))?;
                layers.append(&mut expanded);
            }
            graph::Object::Manifest(manifest) => {
                layers.push(graph::Layer::new(manifest.digest().unwrap()))
            }
            obj => {
                return Err(format!(
                    "Cannot resolve object into a mountable filesystem layer: {:?}",
                    obj.kind()
                )
                .into())
            }
        }
    }

    Ok(layers)
}

/// Find an spfs-* subcommand in the current environment
pub fn which_spfs<S: AsRef<str>>(subcommand: S) -> Option<std::path::PathBuf> {
    let command = format!("spfs-{}", subcommand.as_ref());
    if let Some(path) = which(&command) {
        return Some(path);
    }
    if let Ok(mut path) = std::env::current_exe() {
        path.set_file_name(&command);
        if is_exe(&path) {
            return Some(path);
        }
    }
    None
}

/// Find a command
pub fn which<S: AsRef<str>>(name: S) -> Option<std::path::PathBuf> {
    let path = std::env::var("PATH").unwrap_or_else(|_| "".to_string());
    let search_paths = path.split(":");
    for path in search_paths {
        let filepath = Path::new(path).join(name.as_ref());
        if is_exe(&filepath) {
            return Some(filepath);
        }
    }
    None
}

fn is_exe<P: AsRef<Path>>(filepath: P) -> bool {
    use faccess::PathExt;

    if !filepath.as_ref().is_file() {
        false
    } else if filepath.as_ref().executable() {
        true
    } else {
        false
    }
}