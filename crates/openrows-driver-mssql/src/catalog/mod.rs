// SPDX-License-Identifier: Apache-2.0
//! The schema tree and table views from the `sys` catalog views (docs/05
//! §6). Names reach the server as bound parameters; the catalog reads the
//! connection's current database.
pub(crate) mod detail;
mod diagram;
mod inspection;
mod roles;
pub(crate) mod rows;

use openrows_driver_api::catalog::{
    ColumnSummary, DatabaseSummary, IndexSummary, ObjectKind, ObjectSummary, SchemaSummary,
    TableStructure,
};
use openrows_driver_api::diagram::SchemaDiagram;
use openrows_driver_api::objects::RoutineKind;
use openrows_driver_api::schema::TableDetail;
use openrows_driver_api::{BoxFuture, Catalog, DriverError};

use rows::{bind, flag, optional, text};

use crate::session::SqlServerSession;

/// Databases the server maintains itself, hidden by default.
const SYSTEM_DATABASES: &[&str] = &["master", "model", "msdb", "tempdb"];
/// Most names the quick-open index returns.
const SEARCH_LIMIT: usize = 50_000;

const DATABASES_SQL: &str = "SELECT name FROM sys.databases \
    WHERE HAS_DBACCESS(name) = 1 ORDER BY name";
/// User schemas first; `sys`, `INFORMATION_SCHEMA`, `guest` and the fixed
/// database-role schemas (ids 16384–16399) are system schemas.
const SCHEMAS_SQL: &str = "SELECT name, CASE WHEN name IN (N'sys', N'INFORMATION_SCHEMA', \
    N'guest') OR schema_id BETWEEN 16384 AND 16399 THEN 1 ELSE 0 END AS system \
    FROM sys.schemas ORDER BY system, name";
const OBJECTS_SQL: &str = include_str!("sql/objects.sql");
const FINGERPRINT_SQL: &str = include_str!("sql/fingerprint.sql");
/// The stored `CREATE` text of a procedure or function (`sys.sql_modules`;
/// NULL for encrypted modules).
const ROUTINE_SQL: &str = "SELECT m.definition FROM sys.sql_modules AS m \
    JOIN sys.objects AS o ON o.object_id = m.object_id \
    JOIN sys.schemas AS s ON s.schema_id = o.schema_id \
    WHERE s.name = @P1 AND o.name = @P2 AND o.type IN ({types})";

/// The tree kind of a `sys.objects` type.
fn object_kind(code: &str) -> Option<ObjectKind> {
    match code.trim() {
        "U" | "S" => Some(ObjectKind::Table),
        "V" => Some(ObjectKind::View),
        "P" | "PC" => Some(ObjectKind::Procedure),
        "FN" | "IF" | "TF" | "FS" | "FT" => Some(ObjectKind::Function),
        "SO" => Some(ObjectKind::Sequence),
        "TY" => Some(ObjectKind::Type),
        _ => None,
    }
}

fn object(row: &[Option<String>]) -> Option<ObjectSummary> {
    Some(ObjectSummary {
        schema: text(row, 0),
        name: text(row, 1),
        kind: object_kind(&text(row, 2))?,
        arguments: None,
        row_estimate: optional(row, 3).and_then(|rows| rows.parse().ok()),
        comment: optional(row, 4),
    })
}

impl SqlServerSession {
    async fn databases(&mut self) -> Result<Vec<DatabaseSummary>, DriverError> {
        let rows = self.text_query(DATABASES_SQL, &[]).await?;
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
        let rows = self.text_query(SCHEMAS_SQL, &[]).await?;
        Ok(rows
            .iter()
            .map(|row| SchemaSummary {
                name: text(row, 0),
                system: flag(row, 1),
                comment: None,
            })
            .collect())
    }

    async fn objects(
        &mut self,
        schema: Option<&str>,
        kinds: &[ObjectKind],
    ) -> Result<Vec<ObjectSummary>, DriverError> {
        let (top, filter, params) = match schema {
            Some(schema) => (String::new(), "AND s.name = @P1", bind(&[schema])),
            None => (format!("TOP ({SEARCH_LIMIT})"), "", Vec::new()),
        };
        let sql = OBJECTS_SQL
            .replace("{top}", &top)
            .replace("{schema}", filter)
            .replace(
                "{system}",
                if schema.is_some() {
                    ""
                } else {
                    "AND o.is_ms_shipped = 0"
                },
            )
            .replace(
                "{types}",
                if schema.is_some() {
                    "(t.is_user_defined = 1 OR s.name = N'sys')"
                } else {
                    "t.is_user_defined = 1"
                },
            );
        let rows = self.text_query(&sql, &params).await?;
        Ok(rows
            .iter()
            .filter_map(|row| object(row))
            .filter(|object| kinds.contains(&object.kind))
            .collect())
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
                columns: index.key_list(),
                unique: index.unique,
                primary: index.primary,
                method: index.method,
                predicate: index.filter,
                name: index.name,
            })
            .collect();
        let estimate = self
            .objects(Some(schema), &[ObjectKind::Table])
            .await?
            .into_iter()
            .find(|object| object.name == table)
            .and_then(|object| object.row_estimate);
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
                name: row.name,
                data_type: row.data_type,
                nullable: row.nullable,
                default: row.default.or(row.computed),
                comment: row.comment,
                primary_key: row.primary_key,
                enum_labels: None,
            })
            .collect())
    }
}

impl Catalog for SqlServerSession {
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
        Box::pin(self.objects(Some(schema), kinds))
    }

    fn search_index(&mut self) -> BoxFuture<'_, Result<Vec<ObjectSummary>, DriverError>> {
        Box::pin(self.objects(None, &ObjectKind::ALL))
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

    fn list_roles(
        &mut self,
    ) -> BoxFuture<'_, Result<Vec<openrows_driver_api::inspection::RoleSummary>, DriverError>> {
        Box::pin(roles::roles(self))
    }

    fn role_grants<'a>(
        &'a mut self,
        role: (&'a str, &'a str),
    ) -> BoxFuture<'a, Result<Vec<openrows_driver_api::inspection::GrantSummary>, DriverError>>
    {
        Box::pin(roles::grants(self, role))
    }

    fn object_detail<'a>(
        &'a mut self,
        object: &'a openrows_driver_api::inspection::ObjectIdentity,
    ) -> BoxFuture<'a, Result<openrows_driver_api::inspection::ObjectDetail, DriverError>> {
        Box::pin(inspection::object_detail(self, object))
    }

    fn routine_definition<'a>(
        &'a mut self,
        (schema, name): (&'a str, &'a str),
        kind: RoutineKind,
        _arguments: Option<&'a str>,
    ) -> BoxFuture<'a, Result<Option<String>, DriverError>> {
        let types = match kind {
            RoutineKind::Procedure => "'P'",
            RoutineKind::Function => "'FN', 'IF', 'TF'",
        };
        Box::pin(async move {
            let sql = ROUTINE_SQL.replace("{types}", types);
            let rows = self.text_query(&sql, &bind(&[schema, name])).await?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_types_map_to_tree_kinds() {
        assert_eq!(object_kind("U "), Some(ObjectKind::Table));
        assert_eq!(object_kind("IF"), Some(ObjectKind::Function));
        assert_eq!(object_kind("PC"), Some(ObjectKind::Procedure));
        assert_eq!(object_kind("TR"), None);
        assert!(OBJECTS_SQL.contains("{top}") && OBJECTS_SQL.contains("{schema}"));
    }
}
