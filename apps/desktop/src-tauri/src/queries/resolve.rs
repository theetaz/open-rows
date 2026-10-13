// SPDX-License-Identifier: Apache-2.0
//! Resolve unqualified result sources on the query's own session, never history.
#[cfg(test)]
mod mysql_tests;
mod rows;
#[cfg(test)]
mod tests;

use crate::connections::{Connections, IpcError, IpcErrorKind};
use openrows_driver_api::{Connection, DriverError, catalog};
use openrows_store::profile::Driver;
use openrows_types::SessionId;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::str::FromStr;
use tauri::State;

const POSTGRES: &str = "SELECT n.nspname::text, c.relname::text, c.relkind::text, \
    c.relpersistence::text FROM pg_catalog.pg_class c \
    JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace \
    WHERE c.oid = pg_catalog.to_regclass(pg_catalog.quote_ident($1::text))";
const MYSQL: &str = "SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE, 'permanent' \
    FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? LIMIT 2";
const SQLITE: &str = "SELECT schema, name, type, 'permanent' FROM pragma_table_list() \
    WHERE name = ?1 COLLATE NOCASE LIMIT 2";

/// A verified permanent base table, or an explanation for keeping the result read-only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum SourceResolution {
    Table { schema: String, table: String },
    Readonly { reason: String },
}
impl SourceResolution {
    fn readonly(reason: &str) -> Self {
        Self::Readonly {
            reason: reason.into(),
        }
    }
}

/// Fixed, bounded metadata reads on the owning actor; no user SQL is accepted.
#[tauri::command]
#[specta::specta]
pub(crate) async fn query_resolve_source(
    connections: State<'_, Connections>,
    session_id: String,
    table: String,
) -> Result<SourceResolution, IpcError> {
    validate_name(&table)?;
    let session =
        SessionId::from_str(&session_id).map_err(|_| IpcError::invalid("Invalid session id"))?;
    let owner = connections
        .owners
        .owner(session)
        .ok_or_else(|| IpcError::new(IpcErrorKind::NotFound, "The session is not open"))?;
    let driver = connections.profiles.get(owner.profile)?.driver;
    connections
        .sessions
        .facet_port(session)?
        .call(move |connection| Box::pin(async move { resolve(connection, driver, &table).await }))
        .await
        .map_err(|error| IpcError::from(&error))
}

fn validate_name(table: &str) -> Result<(), IpcError> {
    if table.is_empty() || table.len() > 1024 || table.contains('\0') {
        return Err(IpcError::invalid("Invalid source table name"));
    }
    Ok(())
}

async fn resolve(
    connection: &mut dyn Connection,
    driver: Driver,
    table: &str,
) -> Result<SourceResolution, DriverError> {
    let sql = match driver {
        Driver::Postgres => POSTGRES,
        Driver::Mysql | Driver::Mariadb => MYSQL,
        Driver::Sqlite | Driver::LibsqlFile => SQLITE,
        _ => {
            return Ok(SourceResolution::readonly(
                "This engine cannot authoritatively resolve unqualified result tables; qualify the source schema.",
            ));
        }
    };
    let rows = rows::query(connection, sql, table).await?;
    let resolved = resolution(&rows, driver);
    if matches!(driver, Driver::Mysql | Driver::Mariadb)
        && let SourceResolution::Table { schema, table } = &resolved
    {
        // INFORMATION_SCHEMA omits temporary shadows. Reuse the driver's escaped
        // SHOW CREATE catalog path with the canonical names returned by the server.
        let detail = catalog(connection)?.table_detail(schema, table).await?;
        let definition = detail.info.and_then(|info| info.definition);
        if let Some(reason) = mysql_definition_reason(definition.as_deref()) {
            return Ok(SourceResolution::readonly(reason));
        }
    }
    Ok(resolved)
}

fn resolution(rows: &[[String; 4]], driver: Driver) -> SourceResolution {
    if rows.iter().any(|row| temporary(row, driver)) {
        return SourceResolution::readonly("Temporary tables cannot be edited from query results.");
    }
    let [row] = rows else {
        return SourceResolution::readonly(if rows.is_empty() {
            "The source table was not found in the query session's current namespace."
        } else {
            "The unqualified source name is ambiguous across schemas; qualify the table."
        });
    };
    let [schema, table, kind, _] = row;
    if !matches!(kind.as_str(), "r" | "p" | "BASE TABLE" | "table") {
        return SourceResolution::readonly(
            "The source is a view or another non-base relation; only base tables are editable.",
        );
    }
    if matches!(driver, Driver::Sqlite | Driver::LibsqlFile) && schema != "main" {
        return SourceResolution::readonly(
            "The source is in an attached database; qualify its schema to verify the edit target.",
        );
    }
    SourceResolution::Table {
        schema: schema.clone(),
        table: table.clone(),
    }
}

fn temporary([schema, _, kind, persistence]: &[String; 4], driver: Driver) -> bool {
    match driver {
        Driver::Postgres => persistence == "t",
        Driver::Sqlite | Driver::LibsqlFile => schema == "temp",
        Driver::Mysql | Driver::Mariadb => kind == "TEMPORARY",
        _ => false,
    }
}

fn mysql_definition_reason(definition: Option<&str>) -> Option<&'static str> {
    let words: Vec<_> = definition
        .unwrap_or_default()
        .split_whitespace()
        .take(3)
        .collect();
    if words
        .get(1)
        .is_some_and(|word| word.eq_ignore_ascii_case("TEMPORARY"))
    {
        Some("Temporary tables cannot be edited from query results.")
    } else if words
        .first()
        .is_some_and(|word| word.eq_ignore_ascii_case("CREATE"))
        && words
            .get(1)
            .is_some_and(|word| word.eq_ignore_ascii_case("TABLE"))
    {
        None
    } else {
        Some("Could not verify a permanent base table in the query session.")
    }
}
