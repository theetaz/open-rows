// SPDX-License-Identifier: Apache-2.0
//! `MariaDB` sequence storage reads never call NEXTVAL or session-scoped LASTVAL.
use super::{bind, optional, text};
use crate::session::MySqlSession;
use openrows_driver_api::inspection::{ObjectDetail, ObjectIdentity, attributes};
use openrows_driver_api::{Connection, DriverError};
use openrows_sql::Dialect;

/// Inspect only a sequence confirmed by the server catalog.
pub(crate) async fn sequence(
    session: &mut MySqlSession,
    object: &ObjectIdentity,
) -> Result<ObjectDetail, DriverError> {
    if session.server().product != "MariaDB" {
        return Err(DriverError::Unsupported("MySQL sequences".to_owned()));
    }
    let listed = session
        .text_query(
            "SELECT t.TABLE_SCHEMA, t.TABLE_NAME, c.COLUMN_TYPE FROM information_schema.TABLES t \
         LEFT JOIN information_schema.COLUMNS c ON c.TABLE_SCHEMA = t.TABLE_SCHEMA \
         AND c.TABLE_NAME = t.TABLE_NAME AND c.COLUMN_NAME = 'start_value' \
         WHERE t.TABLE_SCHEMA = ? AND t.TABLE_NAME = ? AND t.TABLE_TYPE = 'SEQUENCE'",
            &bind(&[&object.schema, &object.name]),
        )
        .await?;
    let catalog = listed.first().ok_or_else(|| missing("Sequence"))?;
    let sql = detail_sql(&text(catalog, 0), &text(catalog, 1))?;
    let rows = session.text_query(&sql, &[]).await?;
    let row = rows.first().ok_or_else(|| missing("Sequence"))?;
    let mut fields = attributes(
        &[
            "Start",
            "Minimum",
            "Maximum",
            "Increment",
            "Cycle",
            "Cache",
            "Next uncached value",
        ],
        row,
    );
    fields.insert(
        0,
        attributes(&["Data type"], &[optional(catalog, 2)]).remove(0),
    );
    Ok(ObjectDetail {
        attributes: fields,
        ..ObjectDetail::default()
    })
}

fn detail_sql(schema: &str, name: &str) -> Result<String, DriverError> {
    let name = Dialect::MySql
        .qualified(schema, name)
        .map_err(|error| DriverError::Protocol(error.to_string()))?;
    Ok(format!(
        "SELECT start_value, minimum_value, maximum_value, increment, \
        cycle_option, cache_size, next_not_cached_value FROM {name}"
    ))
}

fn missing(kind: &str) -> DriverError {
    DriverError::Server {
        code: "42704".to_owned(),
        message: format!("{kind} no longer exists or is not visible"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_sequence_identifiers_escape_quotes_backslashes_and_unicode()
    -> Result<(), DriverError> {
        let sql = detail_sql("db`\\é", "seq`\\සි")?;
        assert!(sql.ends_with("FROM `db``\\é`.`seq``\\සි`"));
        assert!(!sql.contains("NEXTVAL"));
        assert!(detail_sql("db", "a\0").is_err());
        Ok(())
    }
}
