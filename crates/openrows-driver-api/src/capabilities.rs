// SPDX-License-Identifier: Apache-2.0
//! What a driver supports, known before it connects (docs/05 §2.1).
//!
//! The app reads [`Capabilities`] to decide which toolbar actions, tree
//! nodes and editors to offer instead of naming engines, and the
//! conformance suite (`openrows-driver-conformance`) runs exactly the test
//! groups a driver's capabilities claim, then checks the live connection
//! agrees with them (facets present, cancel handle offered, …).

use crate::catalog::ObjectKind;

/// Where an engine's databases live.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Location {
    /// A server reached over the network (or a local socket):
    /// `ConnectRequest::endpoint` is `Endpoint::Server`.
    Server,
    /// A local database file: `ConnectRequest::endpoint` is `Endpoint::File`.
    File,
}

/// One optional feature of a driver; absent features are hidden in the UI.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Feature {
    /// A running statement can be cancelled out of band:
    /// `Session::cancel_handle` returns a handle, and a cancelled statement
    /// ends early with the engine's cancel SQLSTATE while the session stays
    /// usable.
    Cancel,
    /// `Session::execute_bound` binds text parameters written with the
    /// dialect's placeholders; values never become SQL text.
    BindParameters,
    /// One server holds several databases and the catalog lists them.
    MultipleDatabases,
    /// Databases contain namespaces (schemas) besides the database itself.
    Schemas,
    /// `Connection::catalog` offers the [`crate::Catalog`] facet.
    Catalog,
    /// `Connection::editor` offers the [`crate::Editor`] facet: row keys,
    /// exact snapshots and guarded edits committed in one transaction.
    RowEdits,
    /// `Driver::alter_sql` generates structure changes.
    StructureChanges,
    /// `Editor::commit_ddl` runs its statements in one transaction, so a
    /// failing statement undoes the earlier ones.
    TransactionalDdl,
    /// The transport can be encrypted with TLS (`ConnectRequest::ssl_mode`).
    Tls,
    /// Connections can run through a route such as an SSH tunnel or proxy
    /// (`ConnectRequest::via`).
    Routes,
    /// `ConnectRequest::startup_commands` run after every connect, and a
    /// failing one fails the connect naming it (FR-CONN-10).
    StartupCommands,
    /// `Catalog::schema_fingerprint` returns a digest that DDL changes, so
    /// the app notices schema changes made outside it (M1-16).
    SchemaFingerprint,
    /// `Connection::admin` offers the [`crate::Admin`] facet: the server's
    /// process list, cancelling a process's statement and terminating it.
    ProcessList,
    /// `Connection::bulk_loader` offers the [`crate::BulkLoader`] facet: the
    /// engine's fast append path for imports (e.g. `COPY FROM STDIN`).
    BulkLoad,
    /// `Driver::object_sql` creates, renames, duplicates, truncates and
    /// drops tables (FR-SCHEMA-10).
    TableLifecycle,
    /// `Driver::object_sql` creates, replaces and drops views.
    ViewEditing,
    /// Materialized views can be created, replaced and dropped.
    MaterializedViews,
    /// `Driver::object_sql` saves and drops triggers from their definition.
    TriggerEditing,
    /// `Driver::object_sql` saves and drops functions and procedures, and
    /// `Catalog::routine_definition` reads their definition.
    RoutineEditing,
    /// Databases can be created (with encoding and collation) and dropped.
    DatabaseLifecycle,
    /// Databases can be renamed.
    DatabaseRename,
    /// Schemas can be created, renamed and dropped.
    SchemaLifecycle,
    /// Structure changes add and drop foreign keys.
    ForeignKeyEditing,
    /// Structure changes add and drop check constraints.
    CheckEditing,
    /// Tables and columns carry comments.
    Comments,
    /// Columns have a collation that structure changes can set.
    ColumnCollation,
    /// Columns have a character set that structure changes can set.
    ColumnCharset,
    /// Structure changes add and remove identity or auto-increment numbering.
    IdentityColumns,
    /// Indexes can have a `WHERE` predicate.
    PartialIndexes,
    /// Add primary and unique constraints to an existing table.
    KeyEditing,
    /// Explicit USING expressions for column type conversion.
    TypeUsing,
    /// Build indexes without blocking writes, outside a transaction.
    ConcurrentIndexes,
    /// Move columns with FIRST/AFTER, preserving their definition.
    ColumnReordering,
    /// User-defined types can be listed and inspected.
    Types,
    /// Installed extensions can be listed and inspected.
    Extensions,
    /// Users, roles, memberships and grants can be inspected read-only.
    Roles,
}

impl Feature {
    /// Every feature, in documentation order.
    pub const ALL: [Self; 36] = [
        Self::Cancel,
        Self::BindParameters,
        Self::MultipleDatabases,
        Self::Schemas,
        Self::Catalog,
        Self::RowEdits,
        Self::StructureChanges,
        Self::TransactionalDdl,
        Self::Tls,
        Self::Routes,
        Self::StartupCommands,
        Self::SchemaFingerprint,
        Self::ProcessList,
        Self::BulkLoad,
        Self::TableLifecycle,
        Self::ViewEditing,
        Self::MaterializedViews,
        Self::TriggerEditing,
        Self::RoutineEditing,
        Self::DatabaseLifecycle,
        Self::DatabaseRename,
        Self::SchemaLifecycle,
        Self::ForeignKeyEditing,
        Self::CheckEditing,
        Self::Comments,
        Self::ColumnCollation,
        Self::ColumnCharset,
        Self::IdentityColumns,
        Self::PartialIndexes,
        Self::KeyEditing,
        Self::TypeUsing,
        Self::ConcurrentIndexes,
        Self::ColumnReordering,
        Self::Types,
        Self::Extensions,
        Self::Roles,
    ];
}

/// A driver's static capabilities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    /// Server or file engine.
    pub location: Location,
    /// The optional features the driver implements.
    pub features: &'static [Feature],
    /// Object kinds the catalog lists below a schema, in tree order.
    pub object_kinds: &'static [ObjectKind],
}

impl Capabilities {
    /// True when the driver implements `feature`.
    #[must_use]
    pub fn supports(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }

    /// True when the catalog lists objects of `kind`.
    #[must_use]
    pub fn lists(&self, kind: ObjectKind) -> bool {
        self.supports(Feature::Catalog) && self.object_kinds.contains(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: Capabilities = Capabilities {
        location: Location::File,
        features: &[Feature::Catalog, Feature::Cancel],
        object_kinds: &[ObjectKind::Table],
    };

    #[test]
    fn features_and_kinds_are_looked_up() {
        assert!(FILE.supports(Feature::Cancel));
        assert!(!FILE.supports(Feature::Tls));
        assert!(FILE.lists(ObjectKind::Table));
        assert!(!FILE.lists(ObjectKind::View));
        let no_catalog = Capabilities {
            features: &[],
            ..FILE
        };
        assert!(!no_catalog.lists(ObjectKind::Table));
        assert_eq!(Feature::ALL.len(), 36);
    }
}
