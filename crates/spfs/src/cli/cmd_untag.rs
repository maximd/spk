// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct CmdUntag {
    #[structopt(
        long = "remote",
        short = "r",
        about = "Remove tags in a remote repository instead of the local one"
    )]
    remote: Option<String>,
    #[structopt(long = "latest", help = "only remove the latest version of this tag")]
    latest: bool,
    #[structopt(
        short = "a",
        long = "all",
        help = "only remove the latest version of this tag"
    )]
    all: bool,
    #[structopt(value_name = "TAG", required = true, help = "The tag to remove")]
    tag: String,
}

impl CmdUntag {
    pub fn run(&mut self, config: &spfs::Config) -> spfs::Result<i32> {
        let mut repo = match &self.remote {
            Some(remote) => config.get_remote(remote)?,
            None => config.get_repository()?.into(),
        };

        let has_version = self.tag.contains("~") || self.latest;
        let mut tag = spfs::tracking::TagSpec::parse(&self.tag)?;
        if self.latest {
            tag = tag.with_version(0);
        }
        if !self.all && !has_version {
            tracing::error!("You must specify one of --all, --latest or provide a tag with an explicit version number");
        }

        if self.all {
            repo.remove_tag_stream(&tag)?;
        } else {
            let resolved = repo.resolve_tag(&tag)?;
            repo.remove_tag(&resolved)?;
        }
        tracing::info!(?tag, "removed");
        Ok(0)
    }
}
