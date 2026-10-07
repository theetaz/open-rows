// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` classification by the statement's leading keyword.

use super::{Classification, Sql, StatementKind, session};
use crate::lexer::TokenKind;

use StatementKind::{Ddl, Read, Unknown, Write};

/// Statement keywords that can follow a `WITH` clause.
const WITH_BODIES: &[&str] = &[
    "SELECT", "VALUES", "TABLE", "INSERT", "UPDATE", "DELETE", "MERGE",
];

/// Classify by the first keyword; `EXPLAIN` is handled by [`explain`].
pub(super) fn dispatch(sql: Sql<'_>) -> Classification {
    let Some(head) = sql.word(0) else {
        return Classification::new(Unknown);
    };
    match head.as_str() {
        "SELECT" | "VALUES" | "TABLE" | "SHOW" | "FETCH" | "MOVE" | "CLOSE" | "LISTEN"
        | "UNLISTEN" | "DEALLOCATE" => Classification::new(Read),
        "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "VACUUM" | "ANALYZE" | "ANALYSE" | "CLUSTER"
        | "REINDEX" | "REFRESH" | "LOCK" | "NOTIFY" | "CHECKPOINT" => Classification::new(Write),
        "TRUNCATE" => Classification::dangerous(Write),
        "DROP" => Classification::dangerous(Ddl),
        "ALTER" => alter(sql),
        "CREATE" | "COMMENT" | "GRANT" | "REVOKE" | "SECURITY" | "IMPORT" | "REASSIGN" => {
            Classification::new(Ddl)
        }
        "WITH" => with(sql),
        "COPY" => copy(sql),
        "PREPARE" => prepare(sql),
        "DECLARE" => declare(sql),
        _ => session::dispatch(sql, &head),
    }
}

fn alter(sql: Sql<'_>) -> Classification {
    Classification {
        kind: Ddl,
        dangerous: sql.has_word("DROP"),
    }
}

/// `WITH … <body>`: classify the main body; data-modifying CTEs are caught by
/// the caller's escalation scan.
fn with(sql: Sql<'_>) -> Classification {
    sql.find_top_level(1, WITH_BODIES)
        .map_or(Classification::new(Unknown), |body| {
            sql.from(body).classify()
        })
}

/// `COPY … TO STDOUT` reads; `COPY … FROM` and `COPY … TO <file | PROGRAM>` write.
fn copy(sql: Sql<'_>) -> Classification {
    let Some(direction) = sql.find_top_level(1, &["TO", "FROM"]) else {
        return Classification::new(Unknown);
    };
    let to_stdout = sql.is_word(direction, "TO") && sql.is_word(direction + 1, "STDOUT");
    Classification::new(if to_stdout { Read } else { Write })
}

/// `PREPARE TRANSACTION` is transaction control; `PREPARE name AS stmt` takes
/// the class of `stmt`.
fn prepare(sql: Sql<'_>) -> Classification {
    if sql.is_word(1, "TRANSACTION") {
        return Classification::new(StatementKind::TxControl);
    }
    inner_after(sql, sql.find_top_level(1, &["AS"]))
}

/// `DECLARE name … CURSOR … FOR query` takes the class of `query`.
fn declare(sql: Sql<'_>) -> Classification {
    let query = sql
        .find_top_level(1, &["CURSOR"])
        .and_then(|cursor| sql.find_top_level(cursor + 1, &["FOR"]));
    inner_after(sql, query)
}

fn inner_after(sql: Sql<'_>, keyword: Option<usize>) -> Classification {
    keyword.map_or(Classification::new(Unknown), |index| {
        sql.from(index + 1).classify()
    })
}

/// `EXPLAIN` without `ANALYZE` only plans, so it reads. `EXPLAIN ANALYZE`
/// executes its statement and takes that statement's class.
pub(super) fn explain(sql: Sql<'_>) -> Classification {
    let (analyze, inner) = if sql.kind(1) == Some(TokenKind::OpenParen) {
        explain_options(sql)
    } else {
        legacy_explain_options(sql)
    };
    if analyze {
        sql.from(inner).classify()
    } else {
        Classification::new(Read)
    }
}

/// `EXPLAIN (opt [value], …) stmt`: returns (analyze, index of stmt).
fn explain_options(sql: Sql<'_>) -> (bool, usize) {
    let mut analyze = false;
    let mut index = 2;
    while let Some(kind) = sql.kind(index) {
        if kind == TokenKind::CloseParen {
            return (analyze, index + 1);
        }
        let is_analyze = sql.is_word(index, "ANALYZE") || sql.is_word(index, "ANALYSE");
        if is_analyze && !is_false(sql, index + 1) {
            analyze = true;
        }
        index += 1;
    }
    (analyze, index)
}

fn is_false(sql: Sql<'_>, index: usize) -> bool {
    sql.is_word(index, "FALSE") || sql.is_word(index, "OFF") || sql.text(index) == "0"
}

/// `EXPLAIN [ANALYZE] [VERBOSE] stmt`: returns (analyze, index of stmt).
fn legacy_explain_options(sql: Sql<'_>) -> (bool, usize) {
    let mut analyze = false;
    let mut index = 1;
    loop {
        if sql.is_word(index, "ANALYZE") || sql.is_word(index, "ANALYSE") {
            analyze = true;
        } else if !sql.is_word(index, "VERBOSE") {
            return (analyze, index);
        }
        index += 1;
    }
}
