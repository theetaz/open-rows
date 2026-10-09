// SPDX-License-Identifier: Apache-2.0
//! A function's or procedure's `SHOW CREATE` definition for the routine
//! editor, and the table's check constraints.
use openrows_driver_api::DriverError;
use openrows_driver_api::objects::RoutineKind;
use openrows_driver_api::schema::{ConstraintDetail, ConstraintKind};
use openrows_sql::Dialect;

use super::{bind, optional, text};
use crate::session::MySqlSession;

/// `CHECK_CONSTRAINTS` exists from `MySQL` 8.0.16 and `MariaDB` 10.2.
const CHECKS_SQL: &str = "SELECT tc.CONSTRAINT_NAME, cc.CHECK_CLAUSE \
    FROM information_schema.TABLE_CONSTRAINTS tc \
    JOIN information_schema.CHECK_CONSTRAINTS cc \
    ON cc.CONSTRAINT_SCHEMA = tc.CONSTRAINT_SCHEMA AND cc.CONSTRAINT_NAME = tc.CONSTRAINT_NAME \
    WHERE tc.TABLE_SCHEMA = ? AND tc.TABLE_NAME = ? AND tc.CONSTRAINT_TYPE = 'CHECK' \
    ORDER BY tc.CONSTRAINT_NAME";

/// `SHOW CREATE FUNCTION|PROCEDURE`'s statement; `None` when it is missing.
pub(super) async fn routine_definition(
    session: &mut MySqlSession,
    (schema, name): (&str, &str),
    kind: RoutineKind,
) -> Result<Option<String>, DriverError> {
    let routine = Dialect::MySql
        .qualified(schema, name)
        .map_err(|error| DriverError::Unsupported(error.to_string()))?;
    let sql = format!("SHOW CREATE {} {routine}", kind.keyword());
    match session.text_query(&sql, &[]).await {
        // Column 2 is the statement (NULL without the privilege to see it).
        Ok(rows) => Ok(rows.first().and_then(|row| optional(row, 2))),
        // 1305: the routine does not exist.
        Err(DriverError::Server { code, .. }) if code == "42000" => Ok(None),
        Err(error) => Err(error),
    }
}

/// Check constraints; none on servers without `CHECK_CONSTRAINTS`.
pub(super) async fn checks(
    session: &mut MySqlSession,
    schema: &str,
    table: &str,
) -> Result<Vec<ConstraintDetail>, DriverError> {
    let rows = match session
        .text_query(CHECKS_SQL, &bind(&[schema, table]))
        .await
    {
        Ok(rows) => rows,
        Err(DriverError::Server { .. }) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    Ok(rows
        .iter()
        .map(|row| {
            let clause = text(row, 1);
            ConstraintDetail {
                name: text(row, 0),
                kind: ConstraintKind::Check,
                definition: format!("CHECK ({clause})"),
                columns: Vec::new(),
                references: None,
            }
        })
        .collect())
}
