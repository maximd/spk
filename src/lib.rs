pub mod build;
mod error;
pub mod storage;

pub use error::{Error, Result};

// -- begin python wrappers --

use pyo3::prelude::*;
use spfs::{self, prelude::*};

#[pyclass]
#[derive(Clone)]
pub struct Digest {
    inner: spfs::encoding::Digest,
}

#[pyproto]
impl pyo3::PyObjectProtocol for Digest {
    fn __str__(&self) -> Result<String> {
        Ok(self.inner.to_string())
    }
    fn __repr__(&self) -> Result<String> {
        Ok(self.inner.to_string())
    }
}

impl From<spfs::encoding::Digest> for Digest {
    fn from(inner: spfs::encoding::Digest) -> Self {
        Self { inner: inner }
    }
}

#[pyclass]
pub struct Runtime {
    inner: spfs::runtime::Runtime,
}

#[pymodule]
fn spkrs(py: Python, m: &PyModule) -> PyResult<()> {
    use self::{build, storage};

    #[pyfn(m, "configure_logging")]
    fn configure_logging(_py: Python, verbosity: u64) -> Result<()> {
        match verbosity {
            0 => {
                if std::env::var("SPFS_DEBUG").is_ok() {
                    std::env::set_var("RUST_LOG", "spfs=debug");
                } else if std::env::var("RUST_LOG").is_err() {
                    std::env::set_var("RUST_LOG", "spfs=info");
                }
            }
            1 => std::env::set_var("RUST_LOG", "spfs=debug"),
            _ => std::env::set_var("RUST_LOG", "spfs=trace"),
        }
        use tracing_subscriber::layer::SubscriberExt;
        let filter = tracing_subscriber::filter::EnvFilter::from_default_env();
        let registry = tracing_subscriber::Registry::default().with(filter);
        let mut fmt_layer = tracing_subscriber::fmt::layer().without_time();
        if verbosity < 3 {
            fmt_layer = fmt_layer.with_target(false);
        }
        let sub = registry.with(fmt_layer);
        tracing::subscriber::set_global_default(sub).unwrap();
        Ok(())
    }
    #[pyfn(m, "active_runtime")]
    fn active_runtime(_py: Python) -> Result<Runtime> {
        let rt = spfs::active_runtime()?;
        Ok(Runtime { inner: rt })
    }
    #[pyfn(m, "local_repository")]
    fn local_repository(_py: Python) -> Result<storage::SpFSRepository> {
        Ok(storage::local_repository()?)
    }
    #[pyfn(m, "remote_repository")]
    fn remote_repository(_py: Python, path: &str) -> Result<storage::SpFSRepository> {
        Ok(storage::remote_repository(path)?)
    }
    #[pyfn(m, "open_tar_repository")]
    fn open_tar_repository(
        _py: Python,
        path: &str,
        create: Option<bool>,
    ) -> Result<storage::SpFSRepository> {
        let repo = match create {
            Some(true) => spfs::storage::tar::TarRepository::create(path)?,
            _ => spfs::storage::tar::TarRepository::open(path)?,
        };
        let handle: spfs::storage::RepositoryHandle = repo.into();
        Ok(storage::SpFSRepository::from(handle))
    }
    #[pyfn(m, "validate_build_changeset")]
    fn validate_build_changeset() -> Result<()> {
        let diffs = spfs::diff(None, None)?;
        build::validate_build_changeset(diffs, "/spfs")?;
        Ok(())
    }
    #[pyfn(m, "validate_source_changeset")]
    fn validate_source_changeset() -> Result<()> {
        let diffs = spfs::diff(None, None)?;
        build::validate_source_changeset(diffs, "/spfs")?;
        Ok(())
    }
    #[pyfn(m, "reconfigure_runtime")]
    fn reconfigure_runtime(
        editable: Option<bool>,
        reset: Option<Vec<String>>,
        stack: Option<Vec<Digest>>,
    ) -> Result<()> {
        let mut runtime = spfs::active_runtime()?;

        // make editable first before trying to make any changes
        runtime.set_editable(true)?;
        spfs::remount_runtime(&runtime)?;

        if let Some(editable) = editable {
            runtime.set_editable(editable)?;
        }
        match reset {
            Some(reset) => runtime.reset(reset.as_slice())?,
            None => runtime.reset_all()?,
        }
        runtime.reset_stack()?;
        if let Some(stack) = stack {
            for digest in stack.iter() {
                runtime.push_digest(&digest.inner)?;
            }
        }
        spfs::remount_runtime(&runtime)?;
        Ok(())
    }
    #[pyfn(m, "build_shell_initialized_command", args = "*")]
    fn build_shell_initialized_command(cmd: String, args: Vec<String>) -> Result<Vec<String>> {
        let cmd = std::ffi::OsString::from(cmd);
        let mut args = args
            .into_iter()
            .map(|a| std::ffi::OsString::from(a))
            .collect();
        let cmd = spfs::build_shell_initialized_command(cmd, &mut args)?;
        let cmd = cmd
            .into_iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        Ok(cmd)
    }
    #[pyfn(m, "build_interactive_shell_command")]
    fn build_interactive_shell_command() -> Result<Vec<String>> {
        let cmd = spfs::build_interactive_shell_cmd()?;
        let cmd = cmd
            .into_iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        Ok(cmd)
    }
    #[pyfn(m, "commit_layer")]
    fn commit_layer(runtime: &mut Runtime) -> Result<Digest> {
        let layer = spfs::commit_layer(&mut runtime.inner)?;
        Ok(Digest::from(layer.digest()?))
    }

    m.add_class::<Digest>()?;
    m.add_class::<Runtime>()?;
    m.add_class::<self::storage::SpFSRepository>()?;

    let empty_spfs: spfs::encoding::Digest = spfs::encoding::EMPTY_DIGEST.into();
    let empty_spk = Digest::from(empty_spfs);
    m.setattr::<&str, PyObject>("EMPTY_DIGEST", empty_spk.into_py(py))?;

    Ok(())
}
