// SPDX-License-Identifier: Apache-2.0
//! Explicit catalog/editor operations and bounded transport replies.
use crate::{
    WireColumnSummary, WireDatabaseSummary, WireObjectDetail, WireObjectKind, WireObjectSummary,
    WireSchemaSummary, WireTableDetail, WireTableStructure,
};
use serde::{Deserialize, Serialize};

/// Read-only catalog operation; unsupported native methods stay unsupported.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CatalogRequest {
    /// Database list.
    Databases,
    /// Schema list.
    Schemas,
    /// Bounded object list.
    Objects {
        /// Namespace.
        schema: String,
        /// Requested kinds.
        #[serde(deserialize_with = "crate::bounded::list")]
        kinds: Vec<WireObjectKind>,
    },
    /// Bounded search index.
    SearchIndex,
    /// Column list.
    Columns {
        /// Namespace.
        schema: String,
        /// Relation.
        table: String,
    },
    /// Relation structure.
    Structure {
        /// Namespace.
        schema: String,
        /// Relation.
        table: String,
    },
    /// Full relation detail.
    Detail {
        /// Namespace.
        schema: String,
        /// Relation.
        table: String,
    },
    /// Schema digest.
    Fingerprint,
    /// View inspection.
    ObjectDetail {
        /// Namespace.
        schema: String,
        /// Name.
        name: String,
        /// Kind.
        kind: WireObjectKind,
        /// Routine overload.
        arguments: Option<String>,
    },
}
/// SQL and bound values remain separate on the wire.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireEdit {
    /// Generated SQL.
    pub sql: String,
    /// Exact text/NULL parameters.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub params: Vec<Option<String>>,
}
/// Atomic edit operation owned by the host worker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EditorRequest {
    /// Key discovery.
    Info {
        /// Namespace.
        schema: String,
        /// Relation.
        table: String,
    },
    /// Exact row snapshot.
    Snapshot {
        /// Namespace.
        schema: String,
        /// Relation.
        table: String,
        /// Key names and values.
        #[serde(deserialize_with = "crate::bounded::list")]
        keys: Vec<(String, String)>,
    },
    /// Guarded edits in one transaction.
    Commit(#[serde(deserialize_with = "crate::bounded::list")] Vec<WireEdit>),
    /// Atomic DDL where supported.
    Ddl(#[serde(deserialize_with = "crate::bounded::list")] Vec<String>),
}
/// Row identity independent of the driver's native representation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WireRowEditInfo {
    /// Non-null unique key.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub key: Vec<String>,
    /// Generated columns.
    #[serde(deserialize_with = "crate::bounded::list")]
    pub protected: Vec<String>,
    /// Hidden row identity.
    pub row_id: Option<String>,
}
/// Bounded facet response, sent in acknowledged chunks when larger than one frame.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum FacetReply {
    /// Database list.
    Databases(#[serde(deserialize_with = "crate::bounded::catalog")] Vec<WireDatabaseSummary>),
    /// Schema list.
    Schemas(#[serde(deserialize_with = "crate::bounded::catalog")] Vec<WireSchemaSummary>),
    /// Object list or search index.
    Objects(#[serde(deserialize_with = "crate::bounded::catalog")] Vec<WireObjectSummary>),
    /// Columns.
    Columns(#[serde(deserialize_with = "crate::bounded::catalog")] Vec<WireColumnSummary>),
    /// Structure.
    Structure(WireTableStructure),
    /// Full detail.
    Detail(WireTableDetail),
    /// Schema digest.
    Fingerprint(Option<String>),
    /// View detail.
    ObjectDetail(WireObjectDetail),
    /// Row identity.
    EditInfo(WireRowEditInfo),
    /// Exact row snapshot.
    Snapshot(#[serde(deserialize_with = "crate::bounded::list")] Vec<(String, Option<String>)>),
    /// Successful commit.
    Committed,
}
