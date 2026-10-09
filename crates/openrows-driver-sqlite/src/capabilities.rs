// SPDX-License-Identifier: Apache-2.0
//! What the `SQLite` driver supports (docs/05 §2.1).
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{Capabilities, Feature, Location};

/// A local file: interrupt-based cancel, bound parameters, attached
/// databases as schemas, guarded edits, structure changes (rebuilding the
/// table where `ALTER TABLE` cannot) and transactional DDL; no TLS or routes.
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
    ],
    object_kinds: &[ObjectKind::Table, ObjectKind::View],
};
