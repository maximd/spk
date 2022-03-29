// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use clap::Args;

/// Migrate the data from and older repository format to the latest one
#[derive(Debug, Args)]
pub struct CmdMigrate {
    /// Replace old data with migrated data one complete
    #[clap(long)]
    upgrade: bool,

    /// The path to the filesystem repository to migrate
    path: String,
}

impl CmdMigrate {
    pub async fn run(&mut self, _config: &spfs::Config) -> spfs::Result<i32> {
        let repo_root = std::path::PathBuf::from(&self.path).canonicalize()?;
        let result = if self.upgrade {
            spfs::storage::fs::migrations::upgrade_repo(repo_root).await?
        } else {
            spfs::storage::fs::migrations::migrate_repo(repo_root).await?
        };
        tracing::info!(path = ?result, "migrated");
        Ok(0)
    }
}