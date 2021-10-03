// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use structopt::StructOpt;

use spfs::{self, prelude::*};

#[derive(Debug, StructOpt)]
pub struct CmdLsTags {
    #[structopt(
        default_value = "/",
        about = "The tag path to list under, defaults to the root ('/')"
    )]
    path: String,
}

impl CmdLsTags {
    pub fn run(&mut self, config: &spfs::Config) -> spfs::Result<i32> {
        let repo = config.get_repository()?;

        let path = relative_path::RelativePathBuf::from(&self.path);
        let names = repo.ls_tags(&path)?;
        for name in names {
            println!("{}", name);
        }
        Ok(0)
    }
}