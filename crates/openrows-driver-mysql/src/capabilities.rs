// SPDX-License-Identifier: Apache-2.0
//! What the `MySQL`/`MariaDB` driver supports (docs/05 §2.1).
use openrows_driver_api::catalog::ObjectKind;
use openrows_driver_api::{Capabilities, Feature, Location};

/// A database is its own only schema and DDL commits implicitly per
/// statement; structure changes are one `ALTER TABLE` (docs/05 §13).
pub(crate) const CAPABILITIES: Capabilities = Capabilities {
    location: Location::Server,
    features: &[
        Feature::Cancel,
        Feature::BindParameters,
        Feature::MultipleDatabases,
        Feature::Catalog,
        Feature::RowEdits,
        Feature::StructureChanges,
        Feature::Tls,
        Feature::Routes,
        Feature::StartupCommands,
        Feature::SchemaFingerprint,
    ],
    // `SEQUENCE` objects exist in `MariaDB` only.
    object_kinds: &[
        ObjectKind::Table,
        ObjectKind::View,
        ObjectKind::Function,
        ObjectKind::Procedure,
        ObjectKind::Sequence,
    ],
};
