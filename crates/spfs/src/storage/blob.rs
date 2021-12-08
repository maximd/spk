// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use crate::{encoding, graph, Result};

pub trait BlobStorage: graph::Database {
    /// Iterate the objects in this storage which are blobs."""
    fn iter_blobs<'db>(
        &'db self,
    ) -> Box<dyn Iterator<Item = Result<(encoding::Digest, graph::Blob)>> + 'db>
    where
        Self: Sized,
    {
        use graph::Object;
        Box::new(self.iter_objects().filter_map(|res| match res {
            Ok((digest, obj)) => match obj {
                Object::Blob(manifest) => Some(Ok((digest, manifest))),
                _ => None,
            },
            Err(err) => Some(Err(err)),
        }))
    }

    /// Return true if the identified blob exists in this storage.
    fn has_blob(&self, digest: &encoding::Digest) -> bool {
        self.read_blob(digest).is_ok()
    }

    /// Return the blob identified by the given digest.
    fn read_blob(&self, digest: &encoding::Digest) -> Result<graph::Blob> {
        use graph::Object;
        match self.read_object(digest) {
            Err(err) => Err(err),
            Ok(Object::Blob(blob)) => Ok(blob),
            Ok(_) => Err(format!("Object is not a blob: {:?}", digest).into()),
        }
    }

    /// Store the given blob
    fn write_blob(&mut self, blob: graph::Blob) -> Result<()> {
        self.write_object(&graph::Object::Blob(blob))
    }
}

impl<T: BlobStorage> BlobStorage for &mut T {}
