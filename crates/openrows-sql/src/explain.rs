// SPDX-License-Identifier: Apache-2.0
//! Explain statement subsets exclude session, transaction and maintenance controls.
use crate::Dialect;
use crate::lexer::{TokenKind, significant};

/// Whether one `PostgreSQL` statement starts with a query/DML head, never a utility.
#[must_use]
pub fn postgres_explainable(statement: &str) -> bool {
    let statements = crate::split(statement, Dialect::Postgres);
    if statements.len() != 1 {
        return false;
    }
    let sql = statements[0].text(statement);
    let tokens = significant(sql, Dialect::Postgres);
    let Some(head) = tokens
        .iter()
        .find(|token| token.kind != TokenKind::OpenParen)
    else {
        return false;
    };
    // Safety classification also calls SET, VACUUM and COPY writes; that does
    // not make them query/DML inputs for an isolated Explain session.
    [
        "SELECT", "VALUES", "TABLE", "WITH", "INSERT", "UPDATE", "DELETE", "MERGE",
    ]
    .iter()
    .any(|word| head.is_word(sql, word))
}

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
    fn postgres_utilities_are_not_explain_inputs() {
        for sql in [
            "SET work_mem = '1MB'",
            "/* SELECT */ SET search_path = public",
            "VACUUM t",
            "ANALYZE t",
            "SHOW work_mem",
            "COPY t TO STDOUT",
            "PREPARE q AS SELECT 1",
            "DECLARE c CURSOR FOR SELECT 1",
            "EXPLAIN SELECT 1",
            "BEGIN",
            "COMMIT",
            "SELECT 1; SET work_mem = '1MB'",
        ] {
            assert!(!postgres_explainable(sql), "{sql}");
        }
    }

    #[test]
    fn postgres_query_and_dml_heads_use_the_shared_lexer() {
        for sql in [
            "-- SET work_mem\nSELECT 'VACUUM'",
            "/* comment */ (SELECT 1)",
            "VALUES (1)",
            "TABLE public.events",
            "WITH q AS (SELECT 1) SELECT * FROM q",
            "WITH q AS (DELETE FROM t RETURNING *) SELECT * FROM q",
            "INSERT INTO t VALUES (1)",
            "UPDATE t SET \"set\" = 1",
            "DELETE FROM t WHERE id = 1",
            "MERGE INTO t USING s ON t.id = s.id WHEN MATCHED THEN DELETE",
        ] {
            assert!(postgres_explainable(sql), "{sql}");
        }
    }

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
