// SPDX-License-Identifier: Apache-2.0
//! The schema tree and table views from `information_schema` (docs/05 §6).
//!
//! A `MySQL` database is a schema: each opened database lists exactly one
//! schema, itself. Names reach the server as bound parameters.
pub(crate) mod detail;
mod diagram;
mod routine;
mod rows;

use rows::enum_labels;
pub(crate) use rows::{ColumnRow, IndexRow};

use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, IndexSummary, ObjectKind, ObjectSummary, SchemaSummary,
    TableStructure,
};
use openrows_driver_api::diagram::SchemaDiagram;
use openrows_driver_api::objects::RoutineKind;
use openrows_driver_api::schema::TableDetail;
use openrows_driver_api::{BoxFuture, Catalog, DriverError};

use crate::session::MySqlSession;

/// Databases the server itself maintains, hidden by default.
const SYSTEM_DATABASES: &[&str] = &["information_schema", "mysql", "performance_schema", "sys"];

/// Most names the quick-open index returns.
const SEARCH_LIMIT: usize = 50_000;

/// Order-independent checksums of the schemas, tables and views (with their
/// creation time, which a rebuilding ALTER changes) and routines.
const FINGERPRINT_SQL: &str = "SELECT CONCAT_WS(':', \
    (SELECT COUNT(*) FROM information_schema.SCHEMATA), \
    (SELECT COALESCE(SUM(CRC32(CONCAT_WS('.', TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE, \
        CREATE_TIME))), 0) FROM information_schema.TABLES), \
    (SELECT COALESCE(SUM(CRC32(CONCAT_WS('.', ROUTINE_SCHEMA, ROUTINE_NAME, ROUTINE_TYPE, \
        LAST_ALTERED))), 0) FROM information_schema.ROUTINES))";

const TABLES_SQL: &str = "SELECT TABLE_NAME, TABLE_TYPE, TABLE_ROWS, TABLE_COMMENT \
    FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? ORDER BY TABLE_NAME";
const ROUTINES_SQL: &str = "SELECT ROUTINE_NAME, ROUTINE_TYPE, ROUTINE_COMMENT \
    FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA = ? ORDER BY ROUTINE_NAME";

fn text(row: &[Option<String>], index: usize) -> String {
    row.get(index).cloned().flatten().unwrap_or_default()
}

fn optional(row: &[Option<String>], index: usize) -> Option<String> {
    row.get(index)
        .cloned()
        .flatten()
        .filter(|value| !value.is_empty())
}

fn bind(values: &[&str]) -> Vec<Option<String>> {
    values
        .iter()
        .map(|value| Some((*value).to_owned()))
        .collect()
}

impl MySqlSession {
    async fn databases(&mut self) -> Result<Vec<DatabaseSummary>, DriverError> {
        let rows = self
            .text_query(
                "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA ORDER BY SCHEMA_NAME",
                &[],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|row| {
                let name = text(row, 0);
                DatabaseSummary {
                    system: SYSTEM_DATABASES.contains(&name.to_ascii_lowercase().as_str()),
                    name,
                    comment: None,
                }
            })
            .collect())
    }

    async fn schemas(&mut self) -> Result<Vec<SchemaSummary>, DriverError> {
        let rows = self.text_query("SELECT DATABASE()", &[]).await?;
        Ok(rows
            .first()
            .and_then(|row| optional(row, 0))
            .map(|name| SchemaSummary {
                name,
                system: false,
                comment: None,
            })
            .into_iter()
            .collect())
    }

    async fn objects(
        &mut self,
        schema: &str,
        kinds: &[ObjectKind],
    ) -> Result<Vec<ObjectSummary>, DriverError> {
        let mut found = Vec::new();
        for row in self.text_query(TABLES_SQL, &bind(&[schema])).await? {
            let kind = match text(&row, 1).as_str() {
                "VIEW" | "SYSTEM VIEW" => ObjectKind::View,
                "SEQUENCE" => ObjectKind::Sequence,
                _ => ObjectKind::Table,
            };
            let estimate = optional(&row, 2).and_then(|rows| rows.parse().ok());
            found.push(object(
                schema,
                (text(&row, 0), kind),
                estimate,
                optional(&row, 3),
            ));
        }
        if kinds
            .iter()
            .any(|kind| matches!(kind, ObjectKind::Function | ObjectKind::Procedure))
        {
            for row in self.text_query(ROUTINES_SQL, &bind(&[schema])).await? {
                found.push(object(
                    schema,
                    (text(&row, 0), routine_kind(&row)),
                    None,
                    optional(&row, 2),
                ));
            }
        }
        found.retain(|object| kinds.contains(&object.kind));
        Ok(found)
    }

    async fn structure(
        &mut self,
        schema: &str,
        table: &str,
    ) -> Result<TableStructure, DriverError> {
        let columns = self.summaries(schema, table).await?;
        let indexes = self
            .index_rows(schema, table)
            .await?
            .into_iter()
            .map(|index| IndexSummary {
                primary: index.name == "PRIMARY",
                columns: index.columns.join(", "),
                unique: index.unique,
                method: index.method,
                predicate: None,
                name: index.name,
            })
            .collect();
        let estimate = self
            .text_query(
                "SELECT TABLE_ROWS FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?",
                &bind(&[schema, table]),
            )
            .await?
            .first()
            .and_then(|row| optional(row, 0))
            .and_then(|rows| rows.parse().ok());
        Ok(TableStructure {
            columns,
            indexes,
            row_estimate: estimate,
        })
    }

    async fn summaries(
        &mut self,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnSummary>, DriverError> {
        Ok(self
            .column_rows(schema, table)
            .await?
            .into_iter()
            .map(|row| ColumnSummary {
                enum_labels: enum_labels(&row.column_type),
                primary_key: row.key == "PRI",
                name: row.name,
                data_type: row.column_type,
                nullable: row.nullable,
                default: row.default,
                comment: row.comment,
            })
            .collect())
    }
}

fn routine_kind(row: &[Option<String>]) -> ObjectKind {
    if text(row, 1) == "PROCEDURE" {
        ObjectKind::Procedure
    } else {
        ObjectKind::Function
    }
}

fn object(
    schema: &str,
    (name, kind): (String, ObjectKind),
    row_estimate: Option<u64>,
    comment: Option<String>,
) -> ObjectSummary {
    ObjectSummary {
        schema: schema.to_owned(),
        name,
        kind,
        arguments: None,
        row_estimate,
        comment,
    }
}

impl Catalog for MySqlSession {
    fn list_databases(&mut self) -> BoxFuture<'_, Result<Vec<DatabaseSummary>, DriverError>> {
        Box::pin(self.databases())
    }

    fn list_schemas(&mut self) -> BoxFuture<'_, Result<Vec<SchemaSummary>, DriverError>> {
        Box::pin(self.schemas())
    }

    fn list_objects<'a>(
        &'a mut self,
        schema: &'a str,
        kinds: &'a [ObjectKind],
    ) -> BoxFuture<'a, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(self.objects(schema, kinds))
    }

    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(async move {
            let Some(schema) = self.schemas().await?.into_iter().next() else {
                return Ok(Vec::new());
            };
            let mut found = self.objects(&schema.name, &ObjectKind::ALL).await?;
            found.truncate(SEARCH_LIMIT);
            Ok(found)
        })
    }

    fn table_columns<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<Vec<ColumnSummary>, DriverError>> {
        Box::pin(self.summaries(schema, table))
    }

    fn table_structure<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableStructure, DriverError>> {
        Box::pin(self.structure(schema, table))
    }

    fn table_detail<'a>(
        &'a mut self,
        schema: &'a str,
        table: &'a str,
    ) -> BoxFuture<'a, Result<TableDetail, DriverError>> {
        Box::pin(detail::table_detail(self, schema, table))
    }

    fn schema_fingerprint(&mut self) -> BoxFuture<'_, Result<Option<String>, DriverError>> {
        Box::pin(async move {
            let rows = self.text_query(FINGERPRINT_SQL, &[]).await?;
            Ok(rows.first().and_then(|row| optional(row, 0)))
        })
    }

    fn schema_diagram<'a>(
        &'a mut self,
        schema: &'a str,
        max_tables: usize,
    ) -> BoxFuture<'a, Result<SchemaDiagram, DriverError>> {
        Box::pin(diagram::schema_diagram(self, schema, max_tables))
    }

    fn routine_definition<'a>(
        &'a mut self,
        routine: (&'a str, &'a str),
        kind: RoutineKind,
        _arguments: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<String>, DriverError>> {
        Box::pin(routine::routine_definition(self, routine, kind))
    }
}
