// SPDX-License-Identifier: Apache-2.0
//! The data one cached scope holds.

use openrows_driver_api::catalog::{DatabaseSummary, ObjectSummary, SchemaSummary};

use crate::index::NameIndex;

/// A cached listing; its variant follows from its [`crate::Scope`].
#[derive(Debug)]
pub enum Listing {
    /// [`crate::Scope::Databases`].
    Databases(Vec<DatabaseSummary>),
    /// [`crate::Scope::Schemas`].
    Schemas(Vec<SchemaSummary>),
    /// [`crate::Scope::Objects`].
    Objects(Vec<ObjectSummary>),
    /// [`crate::Scope::Index`], ready to search.
    Index(NameIndex),
}

impl Listing {
    /// The databases, for a databases listing.
    #[must_use]
    pub fn databases(&self) -> Option<&[DatabaseSummary]> {
        match self {
            Self::Databases(databases) => Some(databases),
            _ => None,
        }
    }

    /// The schemas, for a schemas listing.
    #[must_use]
    pub fn schemas(&self) -> Option<&[SchemaSummary]> {
        match self {
            Self::Schemas(schemas) => Some(schemas),
            _ => None,
        }
    }

    /// The objects, for an objects listing or the name index.
    #[must_use]
    pub fn objects(&self) -> Option<&[ObjectSummary]> {
        match self {
            Self::Objects(objects) => Some(objects),
            Self::Index(index) => Some(index.objects()),
            _ => None,
        }
    }

    /// The searchable name index, for an index listing.
    #[must_use]
    pub fn index(&self) -> Option<&NameIndex> {
        match self {
            Self::Index(index) => Some(index),
            _ => None,
        }
    }
}
