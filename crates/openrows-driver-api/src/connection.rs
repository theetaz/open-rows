// SPDX-License-Identifier: Apache-2.0
//! A live connection and its facets (docs/05 §2, §6, §7).
//!
//! Every [`Connection`] is a [`Session`] (statement execution for the session
//! actor). Catalog reads ([`Catalog`]) and row and structure commits
//! ([`Editor`]) are facets a driver offers when its engine supports them; a
//! missing facet surfaces as `Unsupported`.
use openrows_sql::select::BoundSql;

use crate::admin::Admin;
use crate::bulk::BulkLoader;
use crate::catalog::TableStructure;
use crate::catalog::{ColumnSummary, DatabaseSummary, ObjectKind, ObjectSummary, SchemaSummary};
use crate::diagram::{SchemaDiagram, read_per_table};
use crate::objects::RoutineKind;
use crate::schema::TableDetail;
use crate::trace::TlsStatus;
use crate::{BoxFuture, DriverError, Session};

/// What the server reported about itself right after connecting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerSummary {
    /// Product name, e.g. `PostgreSQL`, `MySQL`, `MariaDB` or `SQLite`.
    pub product: String,
    /// Server or library version, e.g. `17.2` or `8.4.3`.
    pub version: String,
    /// Hosting or compatible-product flavour worth naming, e.g. `Neon`.
    pub flavour: Option<String>,
    /// The database the connection is on (`main` for `SQLite`).
    pub database: String,
    /// The authenticated user; empty for files.
    pub user: String,
    /// Transport encryption.
    pub tls: TlsStatus,
}

/// Catalog row identity and the columns the server generates.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RowEditInfo {
    /// Supported non-null unique key columns.
    pub key: Vec<String>,
    /// Generated or always-identity columns, never written.
    pub protected: Vec<String>,
    /// Hidden pseudo-column identifying the rows of a keyless table (a
    /// `SQLite` rowid table's `rowid`, `_rowid_` or `oid`). When set, `key`
    /// is exactly `[row_id]` and `protected` lists it too: browsing selects
    /// it as a trailing hidden column and edits guard on it but never write it.
    pub row_id: Option<String>,
}

/// One row as `(column, exact text)` pairs in column order; `None` is NULL.
pub type RowSnapshot = Vec<(String, Option<String>)>;

/// One live connection: a [`Session`] plus its facets.
pub trait Connection: Session {
    /// Product, version, flavour, current database, user and TLS state.
    fn server(&self) -> &ServerSummary;

    /// Catalog reads; `None` when the engine has no browsable catalog.
    fn catalog(&mut self) -> Option<&mut dyn Catalog> {
        None
    }

    /// Row and structure commits; `None` when the engine cannot edit.
    fn editor(&mut self) -> Option<&mut dyn Editor> {
        None
    }

    /// Process list and cancel/terminate; `None` when the engine has none.
    fn admin(&mut self) -> Option<&mut dyn Admin> {
        None
    }

    /// The engine's fast append path for imports; `None` when imports use
    /// batched parameterised `INSERT`s instead.
    fn bulk_loader(&mut self) -> Option<&mut dyn BulkLoader> {
        None
    }
}

/// Catalog reads for the schema tree and the table views (docs/05 §6).
pub trait Catalog: Send {
    /// Databases on the server.
    ///
    /// # Errors
    /// Server or transport failures.
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>>;

    /// Schemas (namespaces) of the current database, user schemas first.
    ///
    /// # Errors
    /// Server or transport failures.
    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>>;

    /// Objects of `kinds` in `schema`, ordered by name.
    ///
    /// # Errors
    /// Server or transport failures.
    fn list_objects<'a>(
        &'a mut self,
        schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>>;

    /// Bounded name index of the current database for quick open.
    ///
    /// # Errors
    /// Server or transport failures.
    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>>;

    /// A relation's columns in ordinal order; empty when it does not exist.
    ///
    /// # Errors
    /// Server or transport failures.
    fn table_columns<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>>;

    /// A relation's columns, indexes and row estimate.
    ///
    /// # Errors
    /// Server or transport failures.
    fn table_structure<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableStructure, DriverError>>;

    /// Columns, constraints, indexes, triggers, sizes, comment and (when the
    /// engine stores one) the server's own definition.
    ///
    /// # Errors
    /// Server or transport failures.
    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>>;

    /// A cheap digest of the objects the tree lists (names, kinds and their
    /// catalog row versions); it changes when DDL ran anywhere, including
    /// outside the app. `None` when the engine cannot tell.
    ///
    /// # Errors
    /// Server or transport failures.
    fn schema_fingerprint(&mut self) -> BoxFuture<'_, Result<Option<String>, DriverError>> {
        Box::pin(async { Ok(None) })
    }

    /// Tables (at most `max_tables`, by name), their columns and the foreign
    /// keys between them, for the ER diagram. Defaults to per-table reads;
    /// server engines override it with bulk catalog queries.
    ///
    /// # Errors
    /// Server or transport failures.
    fn schema_diagram<'a>(
        &'a mut self,
        schema: &'a str,
        max_tables: usize,
    ) -> BoxFuture<'a, Result<SchemaDiagram, DriverError>> {
        Box::pin(read_per_table(self, schema, max_tables))
    }

    /// The `CREATE` statement of a function or procedure (with `arguments`,
    /// its identity arguments, picking one overload), as the server prints
    /// it; `None` when it does not exist.
    ///
    /// # Errors
    /// `Unsupported` when the engine has no stored routines; server or
    /// transport failures.
    fn routine_definition<'a>(
        &'a mut self,
        _routine: (&'a str, &'a str),
        _kind: RoutineKind,
        _arguments: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<String>, DriverError>> {
        Box::pin(async { Err(DriverError::Unsupported("stored routines".to_owned())) })
    }
}

/// Exact-row snapshots and atomic commits on a dedicated connection (docs/05 §7).
pub trait Editor: Send {
    /// The table's row key and server-generated columns.
    ///
    /// # Errors
    /// Server or transport failures.
    fn row_edit_info<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<RowEditInfo, DriverError>>;

    /// One complete row as exact text, identified by `keys` (name, value);
    /// the text form is the dialect's `text_of` so commit guards compare it.
    ///
    /// # Errors
    /// Missing or ambiguous row identity, invalid names, server failures.
    fn edit_snapshot<'a>(
        &'a mut self,
        target: (&'a str, &'a str),
        keys: &'a [(String, String)],
    ) -> BoxFuture<'a, Result<RowSnapshot, DriverError>>;

    /// Run row edits in one transaction; each must affect exactly one row.
    ///
    /// # Errors
    /// Server or transport failures, a row conflict (SQLSTATE `40001`) or an
    /// uncertain commit (`08007`); everything else is rolled back.
    fn commit_edits<'a>(
        &'a mut self,
        edits: &'a [BoundSql],
    ) -> BoxFuture<'a, Result<(), DriverError>>;

    /// Run DDL statements in one transaction where the engine allows it.
    ///
    /// # Errors
    /// Server or transport failures, or an uncertain commit (`08007`).
    fn commit_ddl<'a>(
        &'a mut self,
        statements: &'a [String],
    ) -> BoxFuture<'a, Result<(), DriverError>>;
}

/// The connection's catalog facet, or `Unsupported` naming the product.
///
/// # Errors
/// `Unsupported` when the driver offers no catalog.
pub fn catalog(connection: &mut dyn Connection) -> Result<&mut dyn Catalog, DriverError> {
    let product = connection.server().product.clone();
    connection
        .catalog()
        .ok_or_else(|| DriverError::Unsupported(format!("browsing {product} catalogs")))
}

/// The connection's edit facet, or `Unsupported` naming the product.
///
/// # Errors
/// `Unsupported` when the driver cannot edit.
pub fn editor(connection: &mut dyn Connection) -> Result<&mut dyn Editor, DriverError> {
    let product = connection.server().product.clone();
    connection
        .editor()
        .ok_or_else(|| DriverError::Unsupported(format!("editing {product} data")))
}

/// The connection's admin facet, or `Unsupported` naming the product.
///
/// # Errors
/// `Unsupported` when the driver has no process list.
pub fn admin(connection: &mut dyn Connection) -> Result<&mut dyn Admin, DriverError> {
    let product = connection.server().product.clone();
    connection
        .admin()
        .ok_or_else(|| DriverError::Unsupported(format!("{product} process lists")))
}
