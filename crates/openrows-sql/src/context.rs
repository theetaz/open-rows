// SPDX-License-Identifier: Apache-2.0
//! Statements that set where unqualified names resolve: the session state a
//! reconnect must replay so the next statement runs in the same place
//! (docs/14 §2: "startup commands replayed").

use crate::Dialect;
use crate::lexer::{self, TokenKind};

/// Session state a statement sets; the latest statement of each kind wins.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ContextSetting {
    /// `PostgreSQL` `SET [SESSION] search_path …` or `SET SCHEMA …`.
    SearchPath,
    /// `MySQL` `USE db`.
    Database,
}

/// The session setting `statement` changes, when it is a single, complete
/// statement worth replaying. `SET LOCAL` ends with its transaction, so it
/// is never replayed.
#[must_use]
pub fn session_context(statement: &str, dialect: Dialect) -> Option<ContextSetting> {
    let tokens = lexer::significant(statement, dialect);
    if tokens
        .iter()
        .any(|token| token.kind == TokenKind::Semicolon)
    {
        return None;
    }
    let word = |index: usize| {
        tokens
            .get(index)
            .filter(|token| token.kind == TokenKind::Word)
            .map(|token| token.text(statement).to_ascii_uppercase())
    };
    match (dialect, word(0)?.as_str()) {
        (Dialect::Postgres, "SET") => {
            let name = match word(1)?.as_str() {
                "SESSION" => word(2)?,
                other => other.to_owned(),
            };
            matches!(name.as_str(), "SEARCH_PATH" | "SCHEMA").then_some(ContextSetting::SearchPath)
        }
        (Dialect::MySql, "USE") => (tokens.len() == 2).then_some(ContextSetting::Database),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_path_settings_are_replayed_on_postgres() {
        let pg = |sql| session_context(sql, Dialect::Postgres);
        assert_eq!(
            pg("SET search_path TO app, public"),
            Some(ContextSetting::SearchPath)
        );
        assert_eq!(
            pg("set session search_path = 'app'"),
            Some(ContextSetting::SearchPath)
        );
        assert_eq!(pg("SET SCHEMA 'app'"), Some(ContextSetting::SearchPath));
        assert_eq!(pg("SET LOCAL search_path TO app"), None);
        assert_eq!(pg("SET statement_timeout = 0"), None);
        assert_eq!(pg("SET search_path TO a; DELETE FROM t"), None);
        assert_eq!(pg("SELECT 1"), None);
        assert_eq!(pg("USE app"), None);
    }

    #[test]
    fn use_is_replayed_on_mysql() {
        let my = |sql| session_context(sql, Dialect::MySql);
        assert_eq!(my("USE shop"), Some(ContextSetting::Database));
        assert_eq!(my("use `order items`"), Some(ContextSetting::Database));
        assert_eq!(my("USE shop; DROP TABLE t"), None);
        assert_eq!(my("SET search_path TO x"), None);
        assert_eq!(session_context("USE x", Dialect::Sqlite), None);
    }
}
