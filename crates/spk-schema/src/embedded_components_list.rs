// Copyright (c) Sony Pictures Imageworks, et al.
// SPDX-License-Identifier: Apache-2.0
// https://github.com/imageworks/spk

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use spk_schema_foundation::ident_component::{Component, Components};
use spk_schema_foundation::ident_ops::parsing::range_ident_pkg_name;
use spk_schema_foundation::name::PkgNameBuf;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EmbeddedComponents {
    pub name: PkgNameBuf,
    pub components: BTreeSet<Component>,
}

impl std::fmt::Display for EmbeddedComponents {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.name.fmt(f)?;
        self.components.fmt_component_set(f)?;
        Ok(())
    }
}

impl<'de> Deserialize<'de> for EmbeddedComponents {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct EmbeddedComponentsVisitor;

        impl<'de> serde::de::Visitor<'de> for EmbeddedComponentsVisitor {
            type Value = EmbeddedComponents;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an embedded components")
            }

            fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                range_ident_pkg_name::<nom_supreme::error::ErrorTree<_>>(v)
                    .map(|(_, (name, components))| Self::Value {
                        name: name.to_owned(),
                        components,
                    })
                    .map_err(|err| match err {
                        nom::Err::Error(e) | nom::Err::Failure(e) => {
                            serde::de::Error::custom(e.to_string())
                        }
                        nom::Err::Incomplete(_) => unreachable!(),
                    })
            }
        }

        deserializer.deserialize_str(EmbeddedComponentsVisitor)
    }
}

impl Serialize for EmbeddedComponents {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{self}"))
    }
}

/// A set of packages that are embedded/provided by another.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct EmbeddedComponentsList(Vec<EmbeddedComponents>);

impl std::ops::Deref for EmbeddedComponentsList {
    type Target = Vec<EmbeddedComponents>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for EmbeddedComponentsList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
