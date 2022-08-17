// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::convert::{TryFrom, TryInto};
use std::pin::Pin;

use futures::{Stream, TryStreamExt};
use relative_path::RelativePath;

use crate::proto::{self, tag_service_client::TagServiceClient, RpcResult};
use crate::storage::EntryType;
use crate::{
    encoding,
    storage::{self, tag::TagSpecAndTagStream},
    tracking, Result,
};

#[async_trait::async_trait]
impl storage::TagStorage for super::RpcRepository {
    async fn resolve_tag(
        &self,
        tag_spec: &crate::tracking::TagSpec,
    ) -> Result<crate::tracking::Tag> {
        let request = proto::ResolveTagRequest {
            tag_spec: tag_spec.to_string(),
        };
        let response = self
            .tag_client
            .clone()
            .resolve_tag(request)
            .await?
            .into_inner();
        response.to_result()?.try_into()
    }

    fn ls_tags(
        &self,
        path: &RelativePath,
    ) -> Pin<Box<dyn Stream<Item = Result<EntryType>> + Send>> {
        let request = proto::LsTagsRequest {
            path: path.to_string(),
        };
        let mut client = self.tag_client.clone();
        let stream = futures::stream::once(async move { client.ls_tags(request).await })
            .map_err(crate::Error::from)
            .and_then(|r| async { r.into_inner().to_result() })
            .map_ok(|resp| futures::stream::iter(resp.entries.into_iter().map(TryInto::try_into)))
            .try_flatten();
        Box::pin(stream)
    }

    fn find_tags(
        &self,
        digest: &encoding::Digest,
    ) -> Pin<Box<dyn Stream<Item = Result<tracking::TagSpec>> + Send>> {
        let request = proto::FindTagsRequest {
            digest: Some(digest.into()),
        };
        let mut client = self.tag_client.clone();
        let stream = futures::stream::once(async move { client.find_tags(request).await })
            .map_err(crate::Error::from)
            .and_then(|r| async { r.into_inner().to_result() })
            .map_ok(|tag_list| {
                futures::stream::iter(tag_list.tags.into_iter().map(tracking::TagSpec::parse))
            })
            .try_flatten();
        Box::pin(stream)
    }

    fn iter_tag_streams(&self) -> Pin<Box<dyn Stream<Item = Result<TagSpecAndTagStream>> + Send>> {
        let request = proto::IterTagSpecsRequest {};
        let mut client = self.tag_client.clone();
        let stream = futures::stream::once(async move { client.iter_tag_specs(request).await })
            .map_err(crate::Error::from)
            .and_then(|r| async { r.into_inner().to_result() })
            .map_ok(|response| {
                futures::stream::iter(response.tag_specs.into_iter().map(tracking::TagSpec::parse))
            })
            .try_flatten();
        let client = self.tag_client.clone();
        let stream = stream.and_then(move |spec| {
            let client = client.clone();
            async move {
                match read_tag(client, &spec).await {
                    Ok(tags) => Ok((spec, tags)),
                    Err(err) => Err(err),
                }
            }
        });

        Box::pin(stream)
    }

    async fn read_tag(
        &self,
        tag: &tracking::TagSpec,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<tracking::Tag>> + Send>>> {
        read_tag(self.tag_client.clone(), tag).await
    }

    async fn insert_tag(&self, tag: &tracking::Tag) -> Result<()> {
        let request = proto::InsertTagRequest {
            tag: Some(tag.into()),
        };
        let _response = self
            .tag_client
            .clone()
            .insert_tag(request)
            .await?
            .into_inner()
            .to_result()?;
        Ok(())
    }

    async fn remove_tag_stream(&self, tag: &tracking::TagSpec) -> Result<()> {
        let request = proto::RemoveTagStreamRequest {
            tag_spec: tag.to_string(),
        };
        let _response = self
            .tag_client
            .clone()
            .remove_tag_stream(request)
            .await?
            .into_inner()
            .to_result()?;
        Ok(())
    }

    async fn remove_tag(&self, tag: &tracking::Tag) -> Result<()> {
        let request = proto::RemoveTagRequest {
            tag: Some(tag.into()),
        };
        let _response = self
            .tag_client
            .clone()
            .remove_tag(request)
            .await?
            .into_inner()
            .to_result()?;
        Ok(())
    }
}

async fn read_tag(
    mut client: TagServiceClient<tonic::transport::Channel>,
    tag: &tracking::TagSpec,
) -> Result<Pin<Box<dyn Stream<Item = Result<tracking::Tag>> + Send>>> {
    let request = proto::ReadTagRequest {
        tag_spec: tag.to_string(),
    };
    let response = client.read_tag(request).await?.into_inner().to_result()?;
    let items: Result<Vec<_>> = response
        .tags
        .into_iter()
        .map(tracking::Tag::try_from)
        .collect();
    Ok(Box::pin(futures::stream::iter(items?.into_iter().map(Ok))))
}
