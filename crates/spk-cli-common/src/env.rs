// Copyright (c) 2022 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::{
    ffi::{OsStr, OsString},
    sync::Arc,
};

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use spk_ident::{parse_ident, PkgRequest, PreReleasePolicy, RangeIdent, RequestedBy};
use spk_solver::{PackageSource, Solution};
use spk_spec_ops::PackageOps;
use spk_storage::{self as storage};

use crate::Error;

/// Load the current environment from the spfs file system.
pub async fn current_env() -> crate::Result<Solution> {
    match spfs::active_runtime().await {
        Err(spfs::Error::NoActiveRuntime) => {
            return Err(Error::NoEnvironment);
        }
        Err(err) => return Err(err.into()),
        Ok(_) => {}
    }

    let repo = Arc::new(storage::RepositoryHandle::Runtime(Default::default()));
    let mut solution = Solution::new(None);
    for name in repo.list_packages().await? {
        for version in repo.list_package_versions(&name).await?.iter() {
            let pkg = parse_ident(format!("{name}/{version}"))?;
            for pkg in repo.list_package_builds(&pkg).await? {
                let spec = repo.read_package(&pkg).await?;
                let components = match repo.read_components(spec.ident()).await {
                    Ok(c) => c,
                    Err(spk_storage::Error::SpkValidatorsError(
                        spk_validators::Error::PackageNotFoundError(_),
                    )) => {
                        tracing::info!("Skipping missing build {pkg}; currently being built?");
                        continue;
                    }
                    Err(err) => return Err(err.into()),
                };
                let range_ident = RangeIdent::equals(spec.ident(), components.keys().cloned());
                let mut request = PkgRequest::new(range_ident, RequestedBy::CurrentEnvironment);
                request.prerelease_policy = PreReleasePolicy::IncludeAll;
                let repo = repo.clone();
                solution.add(
                    &request,
                    spec,
                    PackageSource::Repository { repo, components },
                );
            }
        }
    }

    Ok(solution)
}

#[cfg(feature = "sentry")]
pub fn configure_sentry() -> sentry::ClientInitGuard {
    use serde_json::json;
    use std::collections::BTreeMap;

    // Call this before `sentry::init` to avoid potential `SIGSEGV`.
    let username = whoami::username();

    // When using the sentry feature it is expected that the DSN
    // and other configuration is provided at *compile* time.
    let guard = sentry::init((
        option_env!("SENTRY_DSN"),
        sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: option_env!("SENTRY_ENVIRONMENT")
                .map(ToString::to_string)
                .map(std::borrow::Cow::Owned),
            ..Default::default()
        },
    ));

    let args: Vec<_> = std::env::args().collect();
    let program = args[0].clone();
    let command = args[1].clone();

    let mut data = BTreeMap::new();
    data.insert(String::from("program"), json!(program));
    data.insert(String::from("command"), json!(command));
    data.insert(String::from("args"), json!(args));

    sentry::configure_scope(|scope| {
        scope.set_user(Some(sentry::User {
            username: Some(username),
            ..Default::default()
        }));

        // Tags are searchable
        scope.set_tag("command", command);
        // Contexts are not searchable
        scope.set_context("SPK", sentry::protocol::Context::Other(data));

        // Okay for captured errors/anyhow, not good for direct
        // messages because they have no error value
        scope.set_fingerprint(Some(["{{ error.value }}"].as_ref()));
    });

    guard
}

pub fn configure_logging(verbosity: u32) -> Result<()> {
    use tracing_subscriber::layer::SubscriberExt;
    let mut directives = match verbosity {
        0 => "spk=info,spfs=warn",
        1 => "spk=debug,spfs=info",
        2 => "spk=trace,spfs=debug",
        3..=6 => "spk=trace,spfs=trace,build_sort=info",
        _ => "spk=trace,spfs=trace,build_sort=debug",
    }
    .to_string();
    if let Ok(overrides) = std::env::var("SPK_LOG") {
        // this is a common scenario because spk often calls itself
        if directives != overrides {
            directives = format!("{},{}", directives, overrides);
        }
    }
    std::env::set_var("SPK_LOG", &directives);
    if let Ok(overrides) = std::env::var("RUST_LOG") {
        // we also allow a full override via the RUST_LOG variable for debugging
        directives = overrides;
    }
    // this is not ideal, because it can propagate annoyingly into
    // created environments, but without it the spfs logging configuration
    // takes over in it's current setup/state.
    std::env::set_var("RUST_LOG", &directives);
    let env_filter = tracing_subscriber::filter::EnvFilter::new(directives);
    let registry = tracing_subscriber::Registry::default().with(env_filter);
    let mut fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .without_time();
    if verbosity < 3 {
        fmt_layer = fmt_layer.with_target(false);
    }

    #[cfg(not(feature = "sentry"))]
    let sub = registry.with(fmt_layer);

    #[cfg(feature = "sentry")]
    let sub = registry.with(fmt_layer).with(sentry_tracing::layer());

    tracing::subscriber::set_global_default(sub).context("Failed to set default logger")
}

static SPK_EXE: Lazy<&OsStr> = Lazy::new(|| match std::env::var_os("SPK_BIN_PATH") {
    Some(p) => Box::leak(Box::new(p)),
    None => Box::leak(Box::new(OsString::from("spk"))),
});

/// Return the spk executable to use when launching spk as a subprocess.
pub fn spk_exe() -> &'static OsStr {
    *SPK_EXE
}
