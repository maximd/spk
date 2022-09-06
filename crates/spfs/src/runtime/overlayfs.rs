// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::MetadataExt;

use crate::{Error, Result};

#[cfg(test)]
#[path = "./overlayfs_test.rs"]
mod overlayfs_test;

pub fn is_removed_entry(meta: &std::fs::Metadata) -> bool {
    // overlayfs uses character device files to denote
    // a file that was removed, using this special file
    // as a whiteout file of the same name.
    if meta.mode() & libc::S_IFCHR == 0 {
        return false;
    }
    // - the device is always 0/0 for a whiteout file
    meta.rdev() == 0
}

#[cfg(target_os = "linux")]
pub fn overlayfs_available_options() -> crate::Result<HashSet<String>> {
    let output = std::process::Command::new("/sbin/modinfo")
        .arg("overlay")
        .output()
        .map_err(|err| Error::process_spawn_error("/sbin/modinfo".into(), err, None))?;

    if output.status.code().unwrap_or(1) != 0 {
        return Err(Error::OverlayFSNotInstalled);
    }

    parse_modinfo_params(&mut BufReader::new(output.stdout.as_slice()))
}

/// Parses the available parameters from the output of `modinfo` for a kernel module
#[cfg(target_os = "linux")]
fn parse_modinfo_params<R: BufRead>(reader: &mut R) -> Result<HashSet<String>> {
    let mut params = HashSet::new();
    for line in reader.lines() {
        let line = line.map_err(|err| {
            Error::String(format!("Failed to read kernel module information: {err}"))
        })?;
        let param = match line.strip_prefix("parm:") {
            Some(remainder) => remainder.trim(),
            None => continue,
        };
        let name = match param.split_once(':') {
            Some((name, _remainder)) => name,
            None => param,
        };
        params.insert(name.to_owned());
    }

    Ok(params)
}
