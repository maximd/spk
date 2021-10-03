// Copyright (c) 2021 Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::collections::VecDeque;

use super::{AmbiguousReferenceError, Object, Result, UnknownReferenceError};
use crate::encoding;

/// Walks an object tree depth-first starting at some root digest
pub struct DatabaseWalker<'db> {
    db: &'db dyn DatabaseView,
    queue: VecDeque<encoding::Digest>,
}

impl<'db> DatabaseWalker<'db> {
    /// Create an iterator that yields all child objects starting at root
    /// from the given database.
    ///
    /// # Errors
    /// The same as [`DatabaseView::read_object`]
    pub fn new(db: &'db dyn DatabaseView, root: encoding::Digest) -> Self {
        let mut queue = VecDeque::new();
        queue.push_back(root);
        DatabaseWalker { db, queue }
    }
}

impl<'db> Iterator for DatabaseWalker<'db> {
    type Item = Result<(encoding::Digest, Object)>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.queue.pop_front();
        match &next {
            None => None,
            Some(next) => {
                let obj = self.db.read_object(next);
                match obj {
                    Ok(obj) => {
                        for digest in obj.child_objects() {
                            self.queue.push_back(digest);
                        }
                        Some(Ok((*next, obj)))
                    }
                    Err(err) => Some(Err(err)),
                }
            }
        }
    }
}

/// Iterates all objects in a database, in no particular order
pub struct DatabaseIterator<'db> {
    db: &'db dyn DatabaseView,
    inner: Box<dyn Iterator<Item = Result<encoding::Digest>>>,
}

impl<'db> DatabaseIterator<'db> {
    /// Create an iterator that yields all child objects starting at root
    /// from the given database.
    ///
    /// # Errors
    /// The same as [`DatabaseView::read_object`]
    pub fn new(db: &'db dyn DatabaseView) -> Self {
        let iter = db.iter_digests();
        DatabaseIterator { db, inner: iter }
    }
}

impl<'db> Iterator for DatabaseIterator<'db> {
    type Item = Result<(encoding::Digest, Object)>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.inner.next();
        match next {
            None => None,
            Some(next) => match next {
                Err(err) => Some(Err(err)),
                Ok(next) => {
                    let obj = self.db.read_object(&next);
                    match obj {
                        Ok(obj) => Some(Ok((next, obj))),
                        Err(err) => Some(Err(err)),
                    }
                }
            },
        }
    }
}

/// A read-only object database.
pub trait DatabaseView {
    /// Read information about the given object from the database.
    ///
    /// # Errors:
    /// - [`super::UnknownObjectError`]: if the object is not in this database
    fn read_object(&self, digest: &encoding::Digest) -> Result<Object>;

    /// Iterate all the object digests in this database.
    fn iter_digests(&self) -> Box<dyn Iterator<Item = Result<encoding::Digest>>>;

    /// Return true if this database contains the identified object
    fn has_object(&self, digest: &encoding::Digest) -> bool {
        self.read_object(digest).is_ok()
    }

    /// Iterate all the object in this database.
    fn iter_objects(&self) -> DatabaseIterator<'_>;

    /// Walk all objects connected to the given root object.
    fn walk_objects<'db>(&'db self, root: &encoding::Digest) -> DatabaseWalker<'db>;

    /// Return the shortened version of the given digest.
    ///
    /// By default this is an O(n) operation defined by the number of objects.
    /// Other implemntations may provide better results.
    fn get_shortened_digest(&self, digest: &encoding::Digest) -> String {
        const SIZE_STEP: usize = 5; // creates 8 char string at base 32
        let mut shortest_size: usize = SIZE_STEP;
        let mut shortest = &digest.as_bytes()[..shortest_size];
        for other in self.iter_digests() {
            match other {
                Err(_) => continue,
                Ok(other) => {
                    if &other.as_bytes()[0..shortest_size] != shortest {
                        continue;
                    }
                    if &other == digest {
                        continue;
                    }
                    while &other.as_bytes()[..shortest_size] == shortest {
                        shortest_size += SIZE_STEP;
                        shortest = &digest.as_bytes()[..shortest_size];
                    }
                }
            }
        }
        data_encoding::BASE32.encode(shortest)
    }

    /// Resolve the complete object digest from a shortened one.
    ///
    /// By default this is an O(n) operation defined by the number of objects.
    /// Other implemntations may provide better results.
    ///
    /// # Errors
    /// - UnknownReferenceError: if the digest cannot be resolved
    /// - AmbiguousReferenceError: if the digest could point to multiple objects
    fn resolve_full_digest(&self, partial: &encoding::PartialDigest) -> Result<encoding::Digest> {
        if let Some(digest) = partial.to_digest() {
            return Ok(digest);
        }
        let mut options = Vec::new();
        for digest in self.iter_digests() {
            let digest = digest?;
            if &digest.as_bytes()[..partial.len()] == partial.as_slice() {
                options.push(digest)
            }
        }

        match options.len() {
            0 => Err(UnknownReferenceError::new_err(partial.to_string())),
            1 => Ok(options.get(0).unwrap().to_owned()),
            _ => Err(AmbiguousReferenceError::new_err(partial.to_string())),
        }
    }
}

impl<T: DatabaseView> DatabaseView for &T {
    fn read_object(&self, digest: &encoding::Digest) -> Result<Object> {
        DatabaseView::read_object(&**self, digest)
    }

    fn iter_digests(&self) -> Box<dyn Iterator<Item = Result<encoding::Digest>>> {
        DatabaseView::iter_digests(&**self)
    }

    fn iter_objects(&self) -> DatabaseIterator<'_> {
        DatabaseView::iter_objects(&**self)
    }

    fn walk_objects<'db>(&'db self, root: &encoding::Digest) -> DatabaseWalker<'db> {
        DatabaseView::walk_objects(&**self, root)
    }
}

impl<T: DatabaseView> DatabaseView for &mut T {
    fn read_object(&self, digest: &encoding::Digest) -> Result<Object> {
        DatabaseView::read_object(&**self, digest)
    }

    fn iter_digests(&self) -> Box<dyn Iterator<Item = Result<encoding::Digest>>> {
        DatabaseView::iter_digests(&**self)
    }

    fn iter_objects(&self) -> DatabaseIterator<'_> {
        DatabaseView::iter_objects(&**self)
    }

    fn walk_objects<'db>(&'db self, root: &encoding::Digest) -> DatabaseWalker<'db> {
        DatabaseView::walk_objects(&**self, root)
    }
}

pub trait Database: DatabaseView {
    /// Write an object to the database, for later retrieval.
    fn write_object(&mut self, obj: &Object) -> Result<()>;

    /// Remove an object from the database.
    fn remove_object(&mut self, digest: &encoding::Digest) -> Result<()>;
}

impl<T: Database> Database for &mut T {
    fn write_object(&mut self, obj: &Object) -> Result<()> {
        Database::write_object(&mut **self, obj)
    }

    fn remove_object(&mut self, digest: &encoding::Digest) -> Result<()> {
        Database::remove_object(&mut **self, digest)
    }
}