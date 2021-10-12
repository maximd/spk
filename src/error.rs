// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use pyo3::{exceptions, prelude::*};

use super::api;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    SPFS(spfs::Error),
    Serde(serde_yaml::Error),
    Collection(crate::build::CollectionError),
    Build(crate::build::BuildError),
    Solve(crate::solve::Error),
    String(String),
    PyErr(PyErr),

    // API Errors
    InvalidVersionError(api::InvalidVersionError),
    InvalidNameError(api::InvalidNameError),
    InvalidBuildError(api::InvalidBuildError),
}

impl Error {
    /// Wraps an error message with a prefix, creating a contextual but generic error
    pub fn wrap<S: AsRef<str>>(prefix: S, err: Self) -> Self {
        // preserve PyErr types
        match err {
            Error::PyErr(pyerr) => Error::PyErr(Python::with_gil(|py| {
                PyErr::from_type(
                    pyerr.ptype(py),
                    format!("{}: {}", prefix.as_ref(), pyerr.pvalue(py).to_string()),
                )
            })),
            err => Error::String(format!("{}: {:?}", prefix.as_ref(), err)),
        }
    }

    /// Wraps an error message with a prefix, creating a contextual error
    pub fn wrap_io<S: AsRef<str>>(prefix: S, err: std::io::Error) -> Error {
        Error::String(format!("{}: {:?}", prefix.as_ref(), err))
    }
}

impl std::error::Error for Error {}

impl From<PyErr> for Error {
    fn from(err: PyErr) -> Error {
        Error::PyErr(err)
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<spfs::Error> for Error {
    fn from(err: spfs::Error) -> Error {
        Error::SPFS(err)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(err: serde_yaml::Error) -> Error {
        Error::Serde(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self))
    }
}

impl From<Error> for PyErr {
    fn from(err: Error) -> PyErr {
        match err {
            Error::IO(err) => err.into(),
            Error::SPFS(spfs::Error::IO(err)) => err.into(),
            Error::SPFS(err) => exceptions::PyRuntimeError::new_err(spfs::io::format_error(&err)),
            Error::Serde(err) => exceptions::PyRuntimeError::new_err(err.to_string()),
            Error::Build(err) => exceptions::PyRuntimeError::new_err(err.message),
            Error::Collection(err) => exceptions::PyRuntimeError::new_err(err.message),
            Error::Solve(err) => err.into(),
            Error::String(msg) => exceptions::PyRuntimeError::new_err(msg),
            Error::InvalidBuildError(err) => exceptions::PyValueError::new_err(err.message),
            Error::InvalidVersionError(err) => exceptions::PyValueError::new_err(err.message),
            Error::InvalidNameError(err) => exceptions::PyValueError::new_err(err.message),
            Error::PyErr(err) => err,
        }
    }
}
