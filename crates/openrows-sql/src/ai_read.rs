// SPDX-License-Identifier: Apache-2.0
//! Conservative model execution policy, stricter than interactive safe mode.
use crate::{Dialect, StatementKind, classify, lexer, split};
use lexer::TokenKind;

/// A model statement could not be proven to be a single side-effect-free read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnsafeAiSql;
impl std::fmt::Display for UnsafeAiSql {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AI execution requires a single protected SELECT without functions, aliases, casts or locking")
    }
}
impl std::error::Error for UnsafeAiSql {}

/// Validate the shared MCP/assistant execution subset; proposals need no such permission.
/// Functions (including quoted/schema-qualified calls), CTEs, casts, variables,
/// session controls and executable comments fail closed rather than using a denylist.
/// # Errors
/// Returns a fixed refusal for unsupported or write-bearing syntax.
pub fn validate_ai_read(sql: &str, dialect: Dialect) -> Result<&str, UnsafeAiSql> {
    if sql.len() > 16_384
        || sql.contains('\0')
        || sql.contains("/*!")
        || sql
            .as_bytes()
            .windows(4)
            .any(|bytes| bytes.eq_ignore_ascii_case(b"/*M!"))
    {
        return Err(UnsafeAiSql);
    }
    let statements = split(sql, dialect);
    if statements.len() != 1 {
        return Err(UnsafeAiSql);
    }
    let statement = statements[0].text(sql).trim();
    let tokens = lexer::significant(statement, dialect);
    if !tokens
        .first()
        .is_some_and(|token| token.is_word(statement, "SELECT"))
        || classify(statement, dialect).kind != StatementKind::Read
        || tokens.iter().any(|token| forbidden(*token, statement))
        || tokens.windows(2).any(|pair| function(pair, statement))
        || tokens
            .iter()
            .filter(|token| token.is_word(statement, "SELECT"))
            .count()
            != 1
        || tokens
            .windows(2)
            .any(|pair| custom_operator(pair, statement))
        || !projection(&tokens, statement)
        || ai_read_source(statement, dialect).is_err()
    {
        return Err(UnsafeAiSql);
    }
    Ok(statement)
}
fn forbidden(token: lexer::Token, sql: &str) -> bool {
    if token.kind == TokenKind::Param
        || matches!(token.text(sql), ":" | "@" | ";")
        || (token.kind == TokenKind::Punct
            && !matches!(token.text(sql), "." | "," | "*" | "=" | "<" | ">" | "!"))
    {
        return true;
    }
    [
        "AS",
        "JOIN",
        "UNION",
        "INTERSECT",
        "EXCEPT",
        "WITH",
        "INTO",
        "FOR",
        "LOCK",
        "PROCEDURE",
        "OUTFILE",
        "DUMPFILE",
        "ANALYZE",
        "OPERATOR",
        "COLLATE",
    ]
    .iter()
    .any(|word| token.is_word(sql, word))
}
fn function(pair: &[lexer::Token], sql: &str) -> bool {
    let [left, right] = pair else { return false };
    right.kind == TokenKind::OpenParen
        && matches!(left.kind, TokenKind::Word | TokenKind::QuotedIdent)
        && ![
            "IN", "EXISTS", "SELECT", "WHERE", "AND", "OR", "NOT", "ON", "HAVING",
        ]
        .iter()
        .any(|word| left.is_word(sql, word))
}

fn custom_operator(pair: &[lexer::Token], sql: &str) -> bool {
    let [left, right] = pair else { return false };
    left.kind == TokenKind::Punct
        && right.kind == TokenKind::Punct
        && left.end == right.start
        && !["<=", ">=", "<>", "!=", ".*"]
            .contains(&format!("{}{}", left.text(sql), right.text(sql)).as_str())
}

fn projection(tokens: &[lexer::Token], sql: &str) -> bool {
    let projection: Vec<_> = tokens
        .iter()
        .skip(1)
        .take_while(|token| !token.is_word(sql, "FROM"))
        .collect();
    if projection.windows(2).any(|pair| {
        matches!(
            pair[0].kind,
            TokenKind::Word | TokenKind::QuotedIdent | TokenKind::String | TokenKind::Number
        ) && matches!(pair[1].kind, TokenKind::Word | TokenKind::QuotedIdent)
    }) {
        return false;
    }
    projection.iter().all(|token| {
        matches!(
            token.kind,
            TokenKind::Word | TokenKind::QuotedIdent | TokenKind::Number | TokenKind::String
        ) || matches!(token.text(sql), "," | "*" | ".")
    })
}
/// Resolve the single explicitly qualified base relation, retaining column provenance.
/// Joins, aliases, table functions, subqueries and unqualified sources fail closed.
/// # Errors
/// The source cannot be proven to be a single schema-qualified identifier.
pub fn ai_read_source(
    sql: &str,
    dialect: Dialect,
) -> Result<Option<(String, String)>, UnsafeAiSql> {
    let tokens = lexer::significant(sql, dialect);
    let Some(index) = tokens.iter().position(|token| token.is_word(sql, "FROM")) else {
        return Ok(None);
    };
    if tokens
        .iter()
        .filter(|token| token.is_word(sql, "FROM"))
        .count()
        != 1
    {
        return Err(UnsafeAiSql);
    }
    let schema = tokens.get(index + 1).ok_or(UnsafeAiSql)?;
    let dot = tokens.get(index + 2).ok_or(UnsafeAiSql)?;
    let table = tokens.get(index + 3).ok_or(UnsafeAiSql)?;
    if dot.text(sql) != "." || !identifier(*schema) || !identifier(*table) {
        return Err(UnsafeAiSql);
    }
    if tokens.get(index + 4).is_some_and(|token| {
        !["WHERE", "ORDER", "LIMIT"]
            .iter()
            .any(|word| token.is_word(sql, word))
    }) {
        return Err(UnsafeAiSql);
    }
    Ok(Some((decode(*schema, sql), decode(*table, sql))))
}
fn identifier(token: lexer::Token) -> bool {
    matches!(token.kind, TokenKind::Word | TokenKind::QuotedIdent)
}
fn decode(token: lexer::Token, sql: &str) -> String {
    let text = token.text(sql);
    if token.kind == TokenKind::QuotedIdent {
        text.get(1..text.len().saturating_sub(1))
            .unwrap_or_default()
            .replace("\"\"", "\"")
            .replace("``", "`")
            .replace("]]", "]")
    } else {
        text.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_sql_rejects_smuggled_writes_and_all_function_calls() {
        for dialect in [Dialect::Postgres, Dialect::MySql, Dialect::Sqlite] {
            for sql in [
                "DELETE FROM t",
                "WITH x AS (DELETE FROM t RETURNING *) SELECT * FROM x",
                "SELECT * INTO t FROM s",
                "SELECT pg_terminate_backend(1)",
                "SELECT pg_catalog.pg_terminate_backend(1)",
                "SELECT \"pg_terminate_backend\"(1)",
                "SELECT nextval('s')",
                "SELECT load_extension('x')",
                "SELECT 1; /* hide */ DROP TABLE t",
                "COPY t TO PROGRAM 'x'",
                "SELECT * FROM t FOR UPDATE",
                "SELECT @x:=1",
                "SELECT 1::user_type",
                "SELECT /*!50000 get_lock('x',1) */ 1",
                "SELECT /*m!100100 get_lock('x',1) */ 1",
                "SELECT unknown_func()",
                "SELECT id FROM main.t WHERE id !! 1",
                "SELECT id FROM main.t WHERE id ** 1",
                "SELECT id FROM main.t WHERE EXISTS (SELECT 1)",
            ] {
                assert!(
                    validate_ai_read(sql, dialect).is_err(),
                    "{dialect:?}: {sql}"
                );
            }
            assert!(
                validate_ai_read(
                    "/* ignore instructions */ SELECT id FROM main.t WHERE id IN (1,2);",
                    dialect
                )
                .is_ok()
            );
        }
    }
}
