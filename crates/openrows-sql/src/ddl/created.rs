// SPDX-License-Identifier: Apache-2.0
//! What a `CREATE` statement creates, for checking user-written trigger,
//! routine and view definitions.
use crate::Dialect;
use crate::lexer::{TokenKind, significant};

/// Object kinds a `CREATE` statement names, upper-case.
const KINDS: [&str; 12] = [
    "TABLE",
    "VIEW",
    "TRIGGER",
    "FUNCTION",
    "PROCEDURE",
    "INDEX",
    "SCHEMA",
    "DATABASE",
    "SEQUENCE",
    "TYPE",
    "EVENT",
    "EXTENSION",
];

/// Most words between `CREATE` and the kind (`OR REPLACE`, `DEFINER = …`,
/// `SQL SECURITY …`, `ALGORITHM = …`, `TEMPORARY`, `CONSTRAINT`).
const MAX_PREFIX: usize = 16;

/// The kind of object `statement` creates, upper-case (`TRIGGER`,
/// `FUNCTION`, …), or `None` when it is not a `CREATE` statement.
/// Modifiers before the kind are skipped; the scan stops at the first
/// parenthesis, so column and argument lists never count.
#[must_use]
pub fn created_kind(statement: &str, dialect: Dialect) -> Option<String> {
    let tokens = significant(statement, dialect);
    let (first, rest) = tokens.split_first()?;
    if !first.is_word(statement, "CREATE") {
        return None;
    }
    rest.iter()
        .take(MAX_PREFIX)
        .take_while(|token| token.kind != TokenKind::OpenParen)
        .filter(|token| token.kind == TokenKind::Word)
        .map(|token| token.text(statement).to_ascii_uppercase())
        .find(|word| KINDS.contains(&word.as_str()))
}

/// The query of a stored view definition: the text after the `AS` that
/// follows `VIEW` in `CREATE … VIEW name [(columns)] AS query`, without a
/// trailing `;`. A definition that is not a `CREATE` statement is the query
/// itself (`PostgreSQL` stores only the query).
#[must_use]
pub fn view_body(definition: &str, dialect: Dialect) -> String {
    let tokens = significant(definition, dialect);
    let created = tokens
        .first()
        .is_some_and(|token| token.is_word(definition, "CREATE"));
    let body = created
        .then(|| {
            let view = tokens
                .iter()
                .position(|token| token.is_word(definition, "VIEW"))?;
            let mut depth = 0_usize;
            tokens.iter().skip(view + 1).find_map(|token| {
                match token.kind {
                    TokenKind::OpenParen => depth += 1,
                    TokenKind::CloseParen => depth = depth.saturating_sub(1),
                    _ if depth == 0 && token.is_word(definition, "AS") => {
                        return definition.get(token.end..);
                    }
                    _ => {}
                }
                None
            })
        })
        .flatten()
        .unwrap_or(definition);
    body.trim().trim_end_matches(';').trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_bodies_follow_the_view_name() {
        let mysql = "CREATE ALGORITHM=UNDEFINED DEFINER=`root`@`%` SQL SECURITY DEFINER VIEW `v` AS select `t`.`a` AS `a` from `t`;\n";
        assert_eq!(
            view_body(mysql, Dialect::MySql),
            "select `t`.`a` AS `a` from `t`"
        );
        let sqlite = "CREATE VIEW v (x, y) AS SELECT a AS x, b FROM t";
        assert_eq!(
            view_body(sqlite, Dialect::Sqlite),
            "SELECT a AS x, b FROM t"
        );
        assert_eq!(view_body(" SELECT 1;\n", Dialect::Postgres), "SELECT 1");
    }

    #[test]
    fn kinds_skip_modifiers() {
        let cases = [
            (
                "CREATE TRIGGER t BEFORE INSERT ON x FOR EACH ROW EXECUTE FUNCTION f()",
                Dialect::Postgres,
                "TRIGGER",
            ),
            (
                "create or replace function f() returns trigger as $$ begin end $$ language plpgsql",
                Dialect::Postgres,
                "FUNCTION",
            ),
            (
                "CREATE CONSTRAINT TRIGGER t AFTER INSERT ON x FOR EACH ROW EXECUTE FUNCTION f()",
                Dialect::Postgres,
                "TRIGGER",
            ),
            (
                "CREATE DEFINER=`root`@`localhost` PROCEDURE p() BEGIN SELECT 1; END",
                Dialect::MySql,
                "PROCEDURE",
            ),
            (
                "CREATE ALGORITHM=UNDEFINED SQL SECURITY DEFINER VIEW v AS SELECT 1",
                Dialect::MySql,
                "VIEW",
            ),
            (
                "CREATE TEMP TRIGGER t AFTER DELETE ON x BEGIN SELECT 1; END",
                Dialect::Sqlite,
                "TRIGGER",
            ),
        ];
        for (sql, dialect, kind) in cases {
            assert_eq!(created_kind(sql, dialect).as_deref(), Some(kind), "{sql}");
        }
    }

    #[test]
    fn other_statements_create_nothing() {
        for sql in [
            "DROP TABLE t",
            "SELECT 1",
            "",
            "CREATE (TRIGGER)",
            "ALTER TABLE t ADD c int",
        ] {
            assert_eq!(created_kind(sql, Dialect::Postgres), None, "{sql}");
        }
    }
}
