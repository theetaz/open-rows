// SPDX-License-Identifier: Apache-2.0
//! `SQLite` classification by the statement's leading keyword.

use super::{Classification, Sql, StatementKind, heads};
use crate::lexer::TokenKind;

use StatementKind::{Ddl, Read, TxControl, Unknown, Write};

/// Pragmas that act (checkpoint, optimise, free memory) even without a value.
const ACTING_PRAGMAS: &[&str] = &[
    "optimize",
    "wal_checkpoint",
    "shrink_memory",
    "incremental_vacuum",
];

/// Pragmas whose argument only selects what to read, e.g. `table_info(t)`.
const READ_WITH_ARGUMENT: &[&str] = &[
    "table_info",
    "table_xinfo",
    "table_list",
    "index_info",
    "index_xinfo",
    "index_list",
    "foreign_key_list",
    "foreign_key_check",
    "integrity_check",
    "quick_check",
];

/// Classify by the first keyword; `EXPLAIN` never runs its statement in
/// `SQLite`, so the caller classifies it as a read.
pub(super) fn dispatch(sql: Sql<'_>) -> Classification {
    let Some(head) = sql.word(0) else {
        return Classification::new(Unknown);
    };
    match head.as_str() {
        "SELECT" | "VALUES" | "DETACH" => Classification::new(Read),
        "INSERT" | "REPLACE" | "UPDATE" | "DELETE" | "VACUUM" | "REINDEX" | "ANALYZE"
        | "ATTACH" => Classification::new(Write),
        "DROP" => Classification::dangerous(Ddl),
        "ALTER" => heads::alter(sql),
        "CREATE" => Classification::new(Ddl),
        "WITH" => heads::with(sql),
        "PRAGMA" => Classification::new(pragma(sql)),
        "BEGIN" | "COMMIT" | "END" | "ROLLBACK" | "SAVEPOINT" | "RELEASE" => {
            Classification::new(TxControl)
        }
        _ => Classification::new(Unknown),
    }
}

/// `PRAGMA [schema.]name`, `PRAGMA name(arg)` or `PRAGMA name = value`:
/// assignments change settings (`Unknown`); reads are `Read`.
fn pragma(sql: Sql<'_>) -> StatementKind {
    let name_at = if sql.text(2) == "." { 3 } else { 1 };
    let Some(name) = sql.word(name_at).map(|name| name.to_ascii_lowercase()) else {
        return Unknown;
    };
    let after = name_at + 1;
    let read = match sql.kind(after) {
        None => !ACTING_PRAGMAS.contains(&name.as_str()),
        Some(TokenKind::OpenParen) => READ_WITH_ARGUMENT.contains(&name.as_str()),
        Some(_) => false,
    };
    if read { Read } else { Unknown }
}
