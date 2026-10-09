// SPDX-License-Identifier: Apache-2.0
//! The schema tree and table views for a database file (docs/05 §6).
//!
//! A file is one database, `main`; its schemas are the attached databases
//! (`main`, `temp` and any `ATTACH`ed file). Names reach `SQLite` as bound
//! parameters of the table-valued pragma functions, never as SQL text.
pub(crate) mod detail;

use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, IndexSummary, ObjectKind, ObjectSummary, SchemaSummary,
    TableStructure,
};
use openrows_driver_api::schema::TableDetail;
use openrows_driver_api::{BoxFuture, Catalog, DriverError};
use openrows_sql::Dialect;
use rusqlite::{Connection, params};

use crate::error::driver_error;
use crate::session::SqliteSession;

/// Most names the quick-open index returns.
const SEARCH_LIMIT: usize = 50_000;

/// rusqlite results as driver errors.
pub(crate) fn sql<T>(result: rusqlite::Result<T>) -> Result<T, DriverError> {
    result.map_err(|error| driver_error(&error))
}

fn schemas(connection: &Connection) -> Result<Vec<SchemaSummary>, DriverError> {
    let mut statement =
        sql(connection.prepare("SELECT name FROM pragma_database_list ORDER BY seq"))?;
    let names = sql(statement.query_map([], |row| row.get::<_, String>(0)))?;
    names
        .map(|name| {
            sql(name).map(|name| SchemaSummary {
                system: name == "temp",
                name,
                comment: None,
            })
        })
        .collect()
}

fn kind_name(kind: ObjectKind) -> Option<&'static str> {
    match kind {
        ObjectKind::Table => Some("table"),
        ObjectKind::View => Some("view"),
        ObjectKind::MaterializedView
        | ObjectKind::ForeignTable
        | ObjectKind::Function
        | ObjectKind::Procedure
        | ObjectKind::Sequence
        | ObjectKind::Type => None,
    }
}

fn objects(
    connection: &Connection,
    schema: &str,
    kinds: &[ObjectKind],
) -> Result<Vec<ObjectSummary>, DriverError> {
    let master = Dialect::Sqlite
        .qualified(schema, "sqlite_master")
        .map_err(|error| invalid(&error))?;
    let query = format!(
        "SELECT type, name FROM {master} WHERE type IN ('table', 'view') \
         AND name NOT LIKE 'sqlite\\_%' ESCAPE '\\' ORDER BY name"
    );
    let mut statement = sql(connection.prepare(&query))?;
    let rows = sql(statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }))?;
    let mut found = Vec::new();
    for row in rows {
        let (kind, name) = sql(row)?;
        let kind = if kind == "view" {
            ObjectKind::View
        } else {
            ObjectKind::Table
        };
        if kinds
            .iter()
            .any(|wanted| kind_name(*wanted) == kind_name(kind))
        {
            found.push(object(schema, name, kind));
        }
    }
    Ok(found)
}

fn object(schema: &str, name: String, kind: ObjectKind) -> ObjectSummary {
    ObjectSummary {
        schema: schema.to_owned(),
        name,
        kind,
        arguments: None,
        row_estimate: None,
        comment: None,
    }
}

fn search(connection: &Connection) -> Result<Vec<ObjectSummary>, DriverError> {
    let mut found = Vec::new();
    for schema in schemas(connection)? {
        found.extend(objects(
            connection,
            &schema.name,
            &[ObjectKind::Table, ObjectKind::View],
        )?);
    }
    found.truncate(SEARCH_LIMIT);
    Ok(found)
}

pub(crate) fn invalid(error: &openrows_sql::select::SelectError) -> DriverError {
    DriverError::Unsupported(error.to_string())
}

/// `PRAGMA table_xinfo` rows: name, type, not null, default, pk position, hidden.
pub(crate) struct ColumnRow {
    pub(crate) name: String,
    pub(crate) data_type: String,
    pub(crate) not_null: bool,
    pub(crate) default: Option<String>,
    pub(crate) primary_key: i64,
    pub(crate) hidden: i64,
}

pub(crate) fn column_rows(
    connection: &Connection,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnRow>, DriverError> {
    let mut statement = sql(connection.prepare(
        "SELECT name, type, \"notnull\", dflt_value, pk, hidden \
         FROM pragma_table_xinfo(?1, ?2) WHERE hidden <> 1 ORDER BY cid",
    ))?;
    let rows = sql(statement.query_map(params![table, schema], |row| {
        Ok(ColumnRow {
            name: row.get(0)?,
            data_type: row.get(1)?,
            not_null: row.get(2)?,
            default: row.get(3)?,
            primary_key: row.get(4)?,
            hidden: row.get(5)?,
        })
    }))?;
    rows.map(sql).collect()
}

fn columns(
    connection: &Connection,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnSummary>, DriverError> {
    Ok(column_rows(connection, schema, table)?
        .into_iter()
        .map(|row| ColumnSummary {
            nullable: !row.not_null && row.primary_key == 0,
            primary_key: row.primary_key > 0,
            name: row.name,
            data_type: row.data_type,
            default: row.default,
            comment: None,
            enum_labels: None,
        })
        .collect())
}

fn structure(
    connection: &Connection,
    schema: &str,
    table: &str,
) -> Result<TableStructure, DriverError> {
    let indexes = detail::indexes(connection, schema, table)?
        .into_iter()
        .map(|index| IndexSummary {
            name: index.name,
            columns: index.columns,
            unique: index.unique,
            primary: index.primary,
            method: index.method,
            predicate: index.predicate,
        })
        .collect();
    Ok(TableStructure {
        columns: columns(connection, schema, table)?,
        indexes,
        row_estimate: None,
    })
}

impl Catalog for SqliteSession {
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>> {
        Box::pin(async {
            Ok(vec![DatabaseSummary {
                name: "main".to_owned(),
                system: false,
                comment: None,
            }])
        })
    }

    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>> {
        Box::pin(async { self.worker.call(|connection| schemas(connection)).await? })
    }

    fn list_objects<'a>(
        &'a mut self,
        schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>> {
        let (schema, kinds) = (schema.to_owned(), kinds.to_vec());
        Box::pin(async move {
            self.worker
                .call(move |connection| objects(connection, &schema, &kinds))
                .await?
        })
    }

    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(async { self.worker.call(|connection| search(connection)).await? })
    }

    fn table_columns<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>> {
        let (schema, table) = (schema.to_owned(), table.to_owned());
        Box::pin(async move {
            self.worker
                .call(move |connection| columns(connection, &schema, &table))
                .await?
        })
    }

    fn table_structure<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableStructure, DriverError>> {
        let (schema, table) = (schema.to_owned(), table.to_owned());
        Box::pin(async move {
            self.worker
                .call(move |connection| structure(connection, &schema, &table))
                .await?
        })
    }

    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>> {
        let (schema, table) = (schema.to_owned(), table.to_owned());
        Box::pin(async move {
            self.worker
                .call(move |connection| detail::table_detail(connection, &schema, &table))
                .await?
        })
    }

    fn object_detail<'a>(
        &'a mut self,
        object: &'a openrows_driver_api::inspection::ObjectIdentity,
    ) -> BoxFuture<'a, Result<openrows_driver_api::inspection::ObjectDetail, DriverError>> {
        Box::pin(async move {
            if object.kind != ObjectKind::View {
                return Err(DriverError::Unsupported(
                    "this SQLite object viewer".to_owned(),
                ));
            }
            let detail = self.table_detail(&object.schema, &object.name).await?;
            openrows_driver_api::inspection::view_detail(detail)
        })
    }

    fn schema_fingerprint(&mut self) -> BoxFuture<'_, Result<Option<String>, DriverError>> {
        Box::pin(async move { self.worker.call(fingerprint).await? })
    }
}

/// Every attached database's `schema_version`, which each DDL statement bumps.
fn fingerprint(connection: &mut Connection) -> Result<Option<String>, DriverError> {
    let mut versions = Vec::new();
    for schema in schemas(connection)? {
        let quoted = Dialect::Sqlite
            .quote_ident(&schema.name)
            .map_err(|error| invalid(&error))?;
        let version: i64 = sql(connection.query_row(
            &format!("PRAGMA {quoted}.schema_version"),
            [],
            |row| row.get(0),
        ))?;
        versions.push(format!("{}={version}", schema.name));
    }
    Ok(Some(versions.join(",")))
}
