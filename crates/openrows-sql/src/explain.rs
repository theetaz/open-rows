// SPDX-License-Identifier: Apache-2.0
//! SQL Server explain accepts queries/DML, never session or transaction controls.
use crate::Dialect;
use crate::lexer::{TokenKind, significant};

/// Whether a T-SQL query/DML batch can run with explain's options and rollback intact.
#[must_use]
pub fn sqlserver_explainable(statement: &str) -> bool {
    let tokens = significant(statement, Dialect::SqlServer);
    let Some(head) = tokens.first() else {
        return false;
    };
    let query = ["SELECT", "WITH", "INSERT", "UPDATE", "DELETE", "MERGE"];
    if !query.iter().any(|word| head.is_word(statement, word)) {
        return false;
    }
    let sets = tokens
        .iter()
        .filter(|token| token.is_word(statement, "SET"))
        .count();
    if sets > 1
        || (sets == 1
            && !tokens
                .iter()
                .any(|token| token.is_word(statement, "UPDATE")))
    {
        return false;
    }
    // T-SQL has optional separators: a second statement can follow a DML head.
    // Quoted identifiers, strings and comments are not control keywords.
    let controls = [
        "BEGIN",
        "COMMIT",
        "ROLLBACK",
        "SAVE",
        "EXEC",
        "EXECUTE",
        "GO",
        "SHOWPLAN_XML",
        "SHOWPLAN_ALL",
        "SHOWPLAN_TEXT",
        "STATISTICS",
        "NOEXEC",
        "PARSEONLY",
        "FMTONLY",
        "IMPLICIT_TRANSACTIONS",
        "XACT_ABORT",
        "USE",
        "BACKUP",
        "RESTORE",
        "KILL",
        "SHUTDOWN",
        "DBCC",
        "OPENROWSET",
        "OPENDATASOURCE",
        "CREATE",
        "ALTER",
        "DROP",
        "GRANT",
        "REVOKE",
        "DENY",
    ];
    !tokens.iter().any(|token| {
        token.kind == TokenKind::Word && controls.iter().any(|word| token.is_word(statement, word))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlserver_controls_cannot_escape_explain() {
        for sql in [
            "COMMIT",
            "EXEC p",
            "SELECT 1 COMMIT",
            "UPDATE t SET x=1 SET SHOWPLAN_XML OFF",
            "SELECT 1 SET IMPLICIT_TRANSACTIONS ON",
            "SELECT * FROM OPENROWSET('x')",
            "SELECT 1 SET NOCOUNT ON",
            "UPDATE t SET x=1 SET NOCOUNT ON",
        ] {
            assert!(!sqlserver_explainable(sql), "{sql}");
        }
        for sql in [
            "-- comment\nSELECT 1",
            "UPDATE [commit] SET x='ROLLBACK'",
            "WITH t AS (SELECT 1 x) SELECT * FROM t",
            "SELECT NEXT VALUE FOR seq",
        ] {
            assert!(sqlserver_explainable(sql), "{sql}");
        }
    }
}
