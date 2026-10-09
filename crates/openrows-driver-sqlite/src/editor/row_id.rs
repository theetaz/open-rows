// SPDX-License-Identifier: Apache-2.0
//! Rowid identity for keyless ordinary tables (docs/05 §13): `WITHOUT ROWID`
//! tables, views, virtual and system tables never get one.
use openrows_driver_api::DriverError;
use rusqlite::{Connection, OptionalExtension, params};

use crate::catalog::{ColumnRow, sql};

/// `SQLite`'s names for the rowid, in preference order.
const ALIASES: [&str; 3] = ["rowid", "_rowid_", "oid"];

/// The first rowid alias no real column shadows, when `table` is an ordinary
/// rowid table; `None` otherwise (also when every alias is a column name).
pub(super) fn row_id(
    connection: &Connection,
    (schema, table): (&str, &str),
    columns: &[ColumnRow],
) -> Result<Option<String>, DriverError> {
    if !rowid_table(connection, schema, table)? {
        return Ok(None);
    }
    let shadowed = |alias: &str| {
        columns
            .iter()
            .any(|column| column.name.eq_ignore_ascii_case(alias))
    };
    Ok(ALIASES
        .into_iter()
        .find(|alias| !shadowed(alias))
        .map(str::to_owned))
}

/// An ordinary (`type = 'table'`), rowid (`wr = 0`), non-system table.
fn rowid_table(connection: &Connection, schema: &str, table: &str) -> Result<bool, DriverError> {
    if table
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("sqlite_"))
    {
        return Ok(false);
    }
    let found: Option<(String, i64)> = sql(connection
        .query_row(
            "SELECT type, wr FROM pragma_table_list WHERE schema = ?1 AND name = ?2",
            params![schema, table],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional())?;
    Ok(found.is_some_and(|(kind, without_rowid)| kind == "table" && without_rowid == 0))
}
