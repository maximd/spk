// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use crate::proto::{
    database_service_client::DatabaseServiceClient, repository_client::RepositoryClient,
    tag_service_client::TagServiceClient,
};
use crate::{storage, Error, Result};

#[derive(Debug)]
pub struct RpcRepository {
    address: url::Url,
    pub(super) repo_client: RepositoryClient<tonic::transport::Channel>,
    pub(super) tag_client: TagServiceClient<tonic::transport::Channel>,
    pub(super) db_client: DatabaseServiceClient<tonic::transport::Channel>,
}

impl RpcRepository {
    pub async fn connect(address: url::Url) -> Result<Self> {
        let endpoint =
            tonic::transport::Endpoint::from_shared(address.to_string()).map_err(|err| {
                Error::String(format!("invalid address for rpc repository: {:?}", err))
            })?;
        let repo_client = RepositoryClient::connect(endpoint.clone())
            .await
            .map_err(|err| {
                Error::String(format!("failed to connect to rpc repository: {:?}", err))
            })?;
        let tag_client = TagServiceClient::connect(endpoint.clone())
            .await
            .map_err(|err| {
                Error::String(format!("failed to connect to rpc repository: {:?}", err))
            })?;
        let db_client = DatabaseServiceClient::connect(endpoint)
            .await
            .map_err(|err| {
                Error::String(format!("failed to connect to rpc repository: {:?}", err))
            })?;
        Ok(Self {
            address,
            repo_client,
            tag_client,
            db_client,
        })
    }
}

impl storage::Repository for RpcRepository {
    fn address(&self) -> url::Url {
        self.address.clone()
    }
}
