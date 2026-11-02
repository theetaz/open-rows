// SPDX-License-Identifier: Apache-2.0
//! Bounded AST admission for protected model reads, never interactive execution.
mod expressions;
mod inspect;
#[cfg(test)]
mod tests;
use crate::{Dialect, lexer, split};
use sqlparser::{
    ast::{Query, Statement, Visit},
    dialect,
    parser::Parser,
};

/// A model statement could not be proven to be a single side-effect-free read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsafeAiSql;
impl std::fmt::Display for UnsafeAiSql {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AI execution requires a protected SELECT with qualified base tables and allowed built-in functions")
    }
}
impl std::error::Error for UnsafeAiSql {}

/// Admit joins, read-only CTEs/subqueries, aggregates, windows and set operations.
/// Unknown functions are refused on every engine: read-only mode cannot prevent
/// network, filesystem, session or external side effects of user-defined code.
/// # Errors
/// Refuses unsupported syntax, direct-column aliases, excessive nesting and all writes.
pub fn validate_ai_read(sql: &str, dialect: Dialect) -> Result<&str, UnsafeAiSql> {
    if sql.len() > 16384 {
        return Err(UnsafeAiSql);
    }
    let statements = split(sql, dialect);
    if statements.len() != 1 {
        return Err(UnsafeAiSql);
    }
    let statement = statements[0].text(sql).trim();
    parse(statement, dialect)?;
    Ok(statement)
}

/// Resolve every schema-qualified base relation, including nested SELECTs.
/// # Errors
/// Refuses the same syntax as model read admission.
pub fn ai_read_sources(sql: &str, dialect: Dialect) -> Result<Vec<(String, String)>, UnsafeAiSql> {
    let query = parse(sql, dialect)?;
    let mut inspection = inspect::Inspection {
        dialect,
        ..inspect::Inspection::default()
    };
    if query.visit(&mut inspection).is_break() {
        return Err(UnsafeAiSql);
    }
    Ok(inspection.sources)
}

/// Preserve exact sample provenance only for an unaliased direct base-table projection.
/// Other admitted reads must redact all sampled values at the execution boundary.
/// # Errors
/// Refuses unsupported SQL.
pub fn ai_read_source(
    sql: &str,
    dialect: Dialect,
) -> Result<Option<(String, String)>, UnsafeAiSql> {
    let query = parse(sql, dialect)?;
    Ok(inspect::simple_source(&query, dialect))
}
fn parse(sql: &str, dialect: Dialect) -> Result<Box<Query>, UnsafeAiSql> {
    lexical(sql, dialect)?;
    let engine = dialect;
    let dialect: &dyn dialect::Dialect = match dialect {
        Dialect::Postgres => &dialect::PostgreSqlDialect {},
        Dialect::MySql => &dialect::MySqlDialect {},
        Dialect::Sqlite => &dialect::SQLiteDialect {},
        Dialect::MongoDb | Dialect::Duckdb | Dialect::SqlServer | Dialect::ClickHouse => {
            return Err(UnsafeAiSql);
        }
    };
    let mut statements = Parser::new(dialect)
        .with_recursion_limit(64)
        .try_with_sql(sql)
        .map_err(|_| UnsafeAiSql)?
        .parse_statements()
        .map_err(|_| UnsafeAiSql)?;
    if statements.len() != 1 {
        return Err(UnsafeAiSql);
    }
    let Some(Statement::Query(query)) = statements.pop() else {
        return Err(UnsafeAiSql);
    };
    if query
        .visit(&mut inspect::Inspection {
            dialect: engine,
            ..inspect::Inspection::default()
        })
        .is_break()
    {
        return Err(UnsafeAiSql);
    }
    Ok(query)
}
fn lexical(sql: &str, dialect: Dialect) -> Result<(), UnsafeAiSql> {
    if sql.len() > 16384
        || sql.contains('\0')
        || sql.contains('\\')
        || sql.contains("/*!")
        || sql.contains("/*+")
        || sql
            .as_bytes()
            .windows(4)
            .any(|bytes| bytes.eq_ignore_ascii_case(b"/*M!"))
    {
        return Err(UnsafeAiSql);
    }
    let tokens = lexer::significant(sql, dialect);
    if tokens.len() > 1024 || dialect == Dialect::MySql && mysql_function_gaps(sql, &tokens) {
        return Err(UnsafeAiSql);
    }
    let mut depth = 0usize;
    for token in tokens {
        match token.kind {
            lexer::TokenKind::OpenParen => {
                depth += 1;
                if depth > 32 {
                    return Err(UnsafeAiSql);
                }
            }
            lexer::TokenKind::CloseParen => {
                depth = depth.checked_sub(1).ok_or(UnsafeAiSql)?;
            }
            lexer::TokenKind::Param => return Err(UnsafeAiSql),
            _ => {}
        }
        if [
            "INTO",
            "OUTFILE",
            "DUMPFILE",
            "PROCEDURE",
            "OPERATOR",
            "COLLATE",
            "LOCK",
            "FOR",
            "ANALYZE",
        ]
        .iter()
        .any(|word| token.is_word(sql, word))
            || matches!(token.text(sql), "@" | ":")
        {
            return Err(UnsafeAiSql);
        }
    }
    if depth != 0 {
        return Err(UnsafeAiSql);
    }
    Ok(())
}
fn mysql_function_gaps(sql: &str, tokens: &[lexer::Token]) -> bool {
    tokens.windows(2).any(|pair| {
        let function = pair[0];
        let parenthesis = pair[1];
        parenthesis.kind == lexer::TokenKind::OpenParen
            && function.end != parenthesis.start
            && ["COUNT", "SUM", "MIN", "MAX"]
                .iter()
                .any(|name| function.is_word(sql, name))
    })
}
