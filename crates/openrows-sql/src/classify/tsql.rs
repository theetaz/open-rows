// SPDX-License-Identifier: Apache-2.0
//! SQL Server (T-SQL) classification by the statement's leading keyword.
//!
//! T-SQL needs no `;` between statements, so one split statement may be a
//! whole batch (`SELECT 1 DELETE FROM t`). A `Read` or transaction-control
//! head is therefore escalated to `Unknown` when a writing keyword appears
//! anywhere in it (see [`hides_writes`]).

use super::{Classification, Sql, StatementKind, heads};
use crate::lexer::TokenKind;

use StatementKind::{Ddl, Read, TxControl, Unknown, Write};

/// System procedures that only report (`EXEC sp_help 't'`).
const READING_PROCEDURES: &[&str] = &[
    "sp_help",
    "sp_helptext",
    "sp_helpindex",
    "sp_helpconstraint",
    "sp_columns",
    "sp_tables",
    "sp_who",
    "sp_who2",
    "sp_databases",
    "sp_server_info",
    "sp_lock",
    "sp_stored_procedures",
];

/// System procedures that change the schema: renames and the
/// `MS_Description` extended properties the structure editor writes.
const DDL_PROCEDURES: &[&str] = &[
    "sp_rename",
    "sp_addextendedproperty",
    "sp_updateextendedproperty",
    "sp_dropextendedproperty",
];

/// `SET` options that cannot change data or lift a read-only guard.
const HARMLESS_OPTIONS: &[&str] = &[
    "ANSI_DEFAULTS",
    "ANSI_NULLS",
    "ANSI_NULL_DFLT_OFF",
    "ANSI_NULL_DFLT_ON",
    "ANSI_PADDING",
    "ANSI_WARNINGS",
    "ARITHABORT",
    "ARITHIGNORE",
    "CONCAT_NULL_YIELDS_NULL",
    "DATEFIRST",
    "DATEFORMAT",
    "DEADLOCK_PRIORITY",
    "FMTONLY",
    "LANGUAGE",
    "LOCK_TIMEOUT",
    "NOCOUNT",
    "NOEXEC",
    "NUMERIC_ROUNDABORT",
    "PARSEONLY",
    "QUOTED_IDENTIFIER",
    "ROWCOUNT",
    "SHOWPLAN_ALL",
    "SHOWPLAN_TEXT",
    "SHOWPLAN_XML",
    "STATISTICS",
    "TEXTSIZE",
    "XACT_ABORT",
];

/// `DBCC` commands that only report.
const READING_DBCC: &[&str] = &[
    "SHOW_STATISTICS",
    "USEROPTIONS",
    "INPUTBUFFER",
    "OPENTRAN",
    "SQLPERF",
    "TRACESTATUS",
];

/// Keywords that write wherever they start a statement of a batch.
const WRITING_KEYWORDS: &[&str] = &[
    "INSERT",
    "DELETE",
    "MERGE",
    "TRUNCATE",
    "DROP",
    "CREATE",
    "ALTER",
    "EXEC",
    "EXECUTE",
    "GRANT",
    "REVOKE",
    "DENY",
    "BACKUP",
    "RESTORE",
    "KILL",
    "SHUTDOWN",
    "DBCC",
    "BULK",
    "RECEIVE",
    "UPDATETEXT",
    "WRITETEXT",
    "RECONFIGURE",
    "CHECKPOINT",
];

/// Classify by the first keyword.
pub(super) fn dispatch(sql: Sql<'_>) -> Classification {
    let Some(head) = sql.word(0) else {
        return Classification::new(Unknown);
    };
    let base = match head.as_str() {
        "SELECT" | "PRINT" | "RAISERROR" | "THROW" | "USE" | "OPEN" | "FETCH" | "CLOSE"
        | "DEALLOCATE" | "WAITFOR" => Classification::new(Read),
        "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "BULK" | "UPDATETEXT" | "WRITETEXT" | "KILL"
        | "BACKUP" | "CHECKPOINT" | "RECONFIGURE" | "RECEIVE" | "SEND" => {
            Classification::new(Write)
        }
        "TRUNCATE" | "SHUTDOWN" | "RESTORE" => Classification::dangerous(Write),
        "DROP" => Classification::dangerous(Ddl),
        "ALTER" => heads::alter(sql),
        "CREATE" | "GRANT" | "REVOKE" | "DENY" | "ENABLE" | "DISABLE" => Classification::new(Ddl),
        "WITH" => heads::with(sql),
        "EXEC" | "EXECUTE" => Classification::new(execute(sql)),
        "DECLARE" => declare(sql),
        "SET" => Classification::new(set(sql)),
        "DBCC" => Classification::new(dbcc(sql)),
        "BEGIN" => Classification::new(begin(sql)),
        "COMMIT" | "ROLLBACK" | "SAVE" => Classification::new(TxControl),
        _ => Classification::new(Unknown),
    };
    escalate(sql, base)
}

/// Keywords that make a hidden statement need confirmation.
const DESTRUCTIVE_KEYWORDS: &[&str] = &["DROP", "TRUNCATE", "SHUTDOWN", "RESTORE"];

/// A reading or transaction-control head that hides writes is `Unknown`,
/// and dangerous when what it hides is destructive.
fn escalate(sql: Sql<'_>, base: Classification) -> Classification {
    if matches!(base.kind, Read | TxControl) && hides_writes(sql) {
        let destructive = DESTRUCTIVE_KEYWORDS.iter().any(|word| sql.has_word(word));
        return Classification {
            kind: Unknown,
            dangerous: base.dangerous || destructive,
        };
    }
    base
}

/// A writing keyword anywhere after the head, an `UPDATE` that starts a
/// statement, or `NEXT VALUE FOR` (which advances a sequence).
fn hides_writes(sql: Sql<'_>) -> bool {
    (1..sql.tokens.len()).any(|index| {
        WRITING_KEYWORDS.iter().any(|word| sql.is_word(index, word))
            || dml_head(sql, index, "UPDATE")
            || (sql.is_word(index, "NEXT")
                && sql.is_word(index + 1, "VALUE")
                && sql.is_word(index + 2, "FOR"))
    })
}

/// Words after `UPDATE`/`DELETE` that show it is not a data statement:
/// `ON DELETE CASCADE`, `GRANT UPDATE ON t`, `AFTER INSERT, UPDATE AS`,
/// `FOR UPDATE OF`, `UPDATE STATISTICS`.
const NOT_DML_AFTER: &[&str] = &["SET", "CASCADE", "NO", "ON", "AS", "OF", "STATISTICS", "TO"];

/// `UPDATE` or `DELETE` at `index` starts a data statement: it is followed
/// by a table (or `FROM`/`TOP`), not by a keyword of a constraint, grant or
/// trigger clause, and is not the `UPDATE(col)` trigger function.
pub(super) fn dml_head(sql: Sql<'_>, index: usize, verb: &str) -> bool {
    if !sql.is_word(index, verb) {
        return false;
    }
    match sql.kind(index + 1) {
        Some(TokenKind::Word) => !NOT_DML_AFTER
            .iter()
            .any(|word| sql.is_word(index + 1, word)),
        Some(TokenKind::QuotedIdent) => true,
        _ => false,
    }
}

/// `EXEC sp_rename …` is DDL, `EXEC sp_help …` reads; any other procedure
/// (or dynamic SQL) may do anything.
fn execute(sql: Sql<'_>) -> StatementKind {
    let Some(name) = procedure_name(sql) else {
        return Unknown;
    };
    if DDL_PROCEDURES.contains(&name.as_str()) {
        Ddl
    } else if READING_PROCEDURES.contains(&name.as_str()) {
        Read
    } else {
        Unknown
    }
}

/// The lower-case procedure name after `EXEC`, without a `sys.`/`dbo.`
/// prefix; `None` for `EXEC (@sql)` or `EXEC @status = proc`.
fn procedure_name(sql: Sql<'_>) -> Option<String> {
    let mut index = 1;
    if matches!(sql.word(1).as_deref(), Some("SYS" | "DBO")) && sql.text(2) == "." {
        index = 3;
    }
    let name = sql.word(index)?;
    (sql.text(index + 1) != "=" && !name.starts_with('@')).then(|| name.to_ascii_lowercase())
}

/// `DECLARE @v …` (variables, table variables) reads; `DECLARE c CURSOR …
/// FOR query` takes the class of its query.
fn declare(sql: Sql<'_>) -> Classification {
    if sql.text(1).starts_with('@') {
        return Classification::new(Read);
    }
    let query = sql
        .find_top_level(1, &["FOR"])
        .map_or(Classification::new(Unknown), |index| {
            sql.from(index + 1).classify()
        });
    if sql.has_word("UPDATE") {
        // `FOR UPDATE [OF …]`: the cursor writes through `WHERE CURRENT OF`.
        return Classification::new(Unknown);
    }
    query
}

/// `SET @v = …` and harmless options read; `SET TRANSACTION ISOLATION
/// LEVEL` and `SET IMPLICIT_TRANSACTIONS` control transactions; anything
/// else (e.g. `SET IDENTITY_INSERT`, `SET CONTEXT_INFO`) is `Unknown`.
fn set(sql: Sql<'_>) -> StatementKind {
    if sql.text(1).starts_with('@') {
        return Read;
    }
    let option = sql.word(1).unwrap_or_default();
    match option.as_str() {
        "TRANSACTION" | "IMPLICIT_TRANSACTIONS" => TxControl,
        name if HARMLESS_OPTIONS.contains(&name) => Read,
        _ => Unknown,
    }
}

fn dbcc(sql: Sql<'_>) -> StatementKind {
    let command = sql.word(1).unwrap_or_default();
    if READING_DBCC.contains(&command.as_str()) {
        Read
    } else {
        Write
    }
}

/// `BEGIN TRAN[SACTION]` and `BEGIN DISTRIBUTED TRAN` control
/// transactions; a `BEGIN … END` or `BEGIN TRY` block may hold anything.
fn begin(sql: Sql<'_>) -> StatementKind {
    let next = sql.word(1).unwrap_or_default();
    match next.as_str() {
        "TRAN" | "TRANSACTION" | "DISTRIBUTED" => TxControl,
        _ => Unknown,
    }
}
