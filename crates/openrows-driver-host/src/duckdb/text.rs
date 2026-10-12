// SPDX-License-Identifier: Apache-2.0
//! Bounded all-text catalog reads on the shared native connection.
use super::error;
use openrows_driver_api::DriverError;

/// One catalog row; every column is cast to VARCHAR in the catalog SQL.
pub(super) type Row = Vec<Option<String>>;

/// Matches the host's catalog reply bound.
const MAX_ROWS: usize = 65_536;

/// Run one catalog statement with text parameters and read `width` text columns.
pub(super) fn rows(
    native: &duckdb::Connection,
    sql: &str,
    params: &[Option<&str>],
    width: usize,
) -> Result<Vec<Row>, DriverError> {
    let mut statement = native.prepare(sql).map_err(error::map)?;
    let mut rows = statement
        .query(duckdb::params_from_iter(params.iter().copied()))
        .map_err(error::map)?;
    let mut read = Vec::new();
    while let Some(row) = rows.next().map_err(error::map)? {
        if read.len() == MAX_ROWS {
            return Err(DriverError::Unsupported(
                "DuckDB catalog response exceeds 65536 rows".into(),
            ));
        }
        let values = (0..width)
            .map(|index| row.get::<_, Option<String>>(index))
            .collect::<Result<Row, _>>()
            .map_err(error::map)?;
        read.push(values);
    }
    Ok(read)
}

/// A required column of a catalog row.
pub(super) fn value(row: &Row, index: usize) -> Result<String, DriverError> {
    optional(row, index)
        .ok_or_else(|| DriverError::Protocol("DuckDB catalog row is missing a value".into()))
}

/// A nullable column of a catalog row.
pub(super) fn optional(row: &Row, index: usize) -> Option<String> {
    row.get(index).cloned().flatten()
}

/// A BOOLEAN column rendered as text.
pub(super) fn flag(row: &Row, index: usize) -> bool {
    row.get(index).and_then(Option::as_deref) == Some("true")
}

/// Comma-joined names from `array_to_string(list, ', ')`.
pub(super) fn names(row: &Row, index: usize) -> Vec<String> {
    optional(row, index)
        .filter(|joined| !joined.is_empty())
        .map(|joined| joined.split(", ").map(str::to_owned).collect())
        .unwrap_or_default()
}
