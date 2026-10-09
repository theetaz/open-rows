// SPDX-License-Identifier: Apache-2.0
//! What the `SQLite` driver supports (docs/05 §2.1).
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{Capabilities, Feature, Location};

/// A local file: interrupt-based cancel, bound parameters, attached
/// databases as schemas, guarded edits, structure changes (rebuilding the
/// table where `ALTER TABLE` cannot), transactional DDL and prepared-insert
/// bulk loading; no TLS or routes. Tables, views and triggers have editors;
/// there are no stored routines, comments, character sets or identity
/// columns, and databases are files.
pub(crate) const CAPABILITIES: Capabilities = Capabilities {
    location: Location::File,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::Schemas,
        Feature::Catalog,
        Feature::RowEdits,
        Feature::StructureChanges,
        Feature::TransactionalDdl,
        Feature::StartupCommands,
        Feature::SchemaFingerprint,
        Feature::BulkLoad,
        Feature::TableLifecycle,
        Feature::ViewEditing,
        Feature::TriggerEditing,
        Feature::ForeignKeyEditing,
        Feature::CheckEditing,
        Feature::ColumnCollation,
        Feature::PartialIndexes,
    ],
    object_kinds: &[ObjectKind::Table, ObjectKind::View],
};
