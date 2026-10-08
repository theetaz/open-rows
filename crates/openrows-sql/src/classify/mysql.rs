// SPDX-License-Identifier: Apache-2.0
//! `MySQL` and `MariaDB` classification by the statement's leading keyword.

use super::{Classification, Sql, StatementKind, heads, mysql_set};

use StatementKind::{Ddl, Read, TxControl, Unknown, Write};

/// Classify by the first keyword; `EXPLAIN` is handled by [`explain`].
pub(super) fn dispatch(sql: Sql<'_>) -> Classification {
    let Some(head) = sql.word(0) else {
        return Classification::new(Unknown);
    };
    match head.as_str() {
        "SELECT" | "VALUES" | "TABLE" | "SHOW" | "DESCRIBE" | "DESC" | "HELP" | "USE" | "CHECK"
        | "CHECKSUM" | "DEALLOCATE" => Classification::new(Read),
        "INSERT" | "REPLACE" | "UPDATE" | "DELETE" | "LOAD" | "OPTIMIZE" | "REPAIR" | "FLUSH"
        | "KILL" | "RESET" | "PURGE" | "CACHE" | "LOCK" | "BINLOG" => Classification::new(Write),
        "TRUNCATE" | "SHUTDOWN" | "RESTART" | "CLONE" => Classification::dangerous(Write),
        "DROP" => Classification::dangerous(Ddl),
        "ALTER" => heads::alter(sql),
        "CREATE" | "RENAME" | "GRANT" | "REVOKE" | "INSTALL" | "UNINSTALL" | "IMPORT" => {
            Classification::new(Ddl)
        }
        "ANALYZE" => analyze(sql),
        "WITH" => heads::with(sql),
        "SET" => Classification::new(mysql_set::set(sql)),
        _ => Classification::new(transaction(sql, &head)),
    }
}

/// Transaction control; `READ WRITE` transactions and `MariaDB` compound
/// `BEGIN NOT ATOMIC` blocks are `Unknown`. Unrecognised heads (`CALL`,
/// `DO`, `EXECUTE`, `HANDLER`, …) are `Unknown` too.
fn transaction(sql: Sql<'_>, head: &str) -> StatementKind {
    match head {
        "BEGIN" if sql.is_word(1, "NOT") => Unknown,
        "BEGIN" | "START" | "XA" if sql.has_word_pair("READ", "WRITE") => Unknown,
        "BEGIN" | "START" | "COMMIT" | "ROLLBACK" | "SAVEPOINT" | "RELEASE" | "XA" | "UNLOCK" => {
            TxControl
        }
        _ => Unknown,
    }
}

/// `ANALYZE TABLE` updates statistics; `MariaDB` `ANALYZE <statement>` runs
/// the statement, so it takes that statement's class.
fn analyze(sql: Sql<'_>) -> Classification {
    let table = ["TABLE", "TABLES", "NO_WRITE_TO_BINLOG", "LOCAL"]
        .iter()
        .any(|word| sql.is_word(1, word));
    if table {
        Classification::new(Write)
    } else {
        sql.from(skip_format(sql, 1)).classify()
    }
}

/// `EXPLAIN` plans without running, so it reads; `EXPLAIN ANALYZE` runs its
/// statement and takes that statement's class.
pub(super) fn explain(sql: Sql<'_>) -> Classification {
    if sql.is_word(1, "ANALYZE") {
        sql.from(skip_format(sql, 2)).classify()
    } else {
        Classification::new(Read)
    }
}

/// Index after an optional `FORMAT = name` starting at `index`.
fn skip_format(sql: Sql<'_>, index: usize) -> usize {
    if sql.is_word(index, "FORMAT") && sql.text(index + 1) == "=" {
        index + 3
    } else {
        index
    }
}
