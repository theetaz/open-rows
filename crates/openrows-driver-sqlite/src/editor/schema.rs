// SPDX-License-Identifier: Apache-2.0
//! Structure changes in one immediate transaction. Foreign keys are
//! switched off around it (the pragma is a no-op inside a transaction), so
//! a table rebuild's `DROP TABLE` neither cascades nor fails, and checked
//! with `foreign_key_check` before the commit, as the `SQLite` documentation
//! prescribes for the generalized `ALTER TABLE` procedure.
use openrows_driver_api::DriverError;
use rusqlite::{Connection, TransactionBehavior};

use crate::catalog::sql;

/// SQLSTATE `foreign_key_violation`.
const FOREIGN_KEY_VIOLATION: &str = "23503";

/// Run `statements` atomically; foreign-key enforcement is restored after.
pub(super) fn commit(
    connection: &mut Connection,
    statements: &[String],
) -> Result<(), DriverError> {
    let enforced: bool = sql(connection.query_row("PRAGMA foreign_keys", [], |row| row.get(0)))?;
    if enforced {
        sql(connection.execute_batch("PRAGMA foreign_keys = OFF"))?;
    }
    let result = run(connection, statements, enforced);
    // A failed rebuild may stop between its two legacy-rename pragmas.
    let legacy = sql(connection.execute_batch("PRAGMA legacy_alter_table = OFF"));
    let result = result.and(legacy);
    if enforced {
        let restored = sql(connection.execute_batch("PRAGMA foreign_keys = ON"));
        return result.and(restored);
    }
    result
}

fn run(connection: &mut Connection, statements: &[String], check: bool) -> Result<(), DriverError> {
    let transaction = sql(connection.transaction_with_behavior(TransactionBehavior::Immediate))?;
    for statement in statements {
        sql(transaction.execute_batch(statement))?;
    }
    if check {
        let mut violations = sql(transaction.prepare("PRAGMA foreign_key_check"))?;
        if sql(sql(violations.query([]))?.next())?.is_some() {
            // Dropping the transaction rolls every statement back.
            return Err(DriverError::Server {
                code: FOREIGN_KEY_VIOLATION.to_owned(),
                message: "The changes would break foreign keys; nothing was changed.".to_owned(),
            });
        }
    }
    sql(transaction.commit())
}
