// SPDX-License-Identifier: Apache-2.0
//! Per-dialect spelling of the SQL that [`crate::select`] and
//! [`crate::edit`] generate: identifier quoting, bind placeholders, text
//! casts, NULL-safe equality and pattern operators.

use crate::Dialect;
use crate::select::SelectError;

impl Dialect {
    /// `name` as a quoted identifier: `"name"` (`PostgreSQL`, `SQLite`) or
    /// `` `name` `` (`MySQL`), with embedded quote characters doubled.
    ///
    /// # Errors
    /// [`SelectError::NulCharacter`] when the name contains NUL.
    pub fn quote_ident(self, name: &str) -> Result<String, SelectError> {
        if name.contains('\0') {
            return Err(SelectError::NulCharacter);
        }
        Ok(match self {
            Self::Postgres | Self::Sqlite => format!("\"{}\"", name.replace('"', "\"\"")),
            Self::MySql => format!("`{}`", name.replace('`', "``")),
        })
    }

    /// `schema.name`, both quoted.
    ///
    /// # Errors
    /// As [`Dialect::quote_ident`].
    pub fn qualified(self, schema: &str, name: &str) -> Result<String, SelectError> {
        Ok(format!(
            "{}.{}",
            self.quote_ident(schema)?,
            self.quote_ident(name)?
        ))
    }

    /// The expression's exact text form as the server renders it, used for
    /// edit snapshots and their optimistic guards: `expr::text`,
    /// `CAST(expr AS CHAR)` or `CAST(expr AS TEXT)`.
    #[must_use]
    pub fn text_of(self, expression: &str) -> String {
        match self {
            Self::Postgres => format!("{expression}::text"),
            Self::MySql => format!("CAST({expression} AS CHAR)"),
            Self::Sqlite => format!("CAST({expression} AS TEXT)"),
        }
    }

    /// The bind placeholder for the `number`-th parameter (1-based).
    pub(crate) fn placeholder(self, number: usize) -> String {
        match self {
            Self::Postgres => format!("${number}"),
            Self::MySql => "?".to_owned(),
            Self::Sqlite => format!("?{number}"),
        }
    }

    /// `text_of(column)` compared NULL-safely and byte-exactly with the text
    /// parameter `placeholder`.
    pub(crate) fn same_text(self, column: &str, placeholder: &str) -> String {
        match self {
            Self::Postgres => {
                format!("{column}::text IS NOT DISTINCT FROM {placeholder}::text")
            }
            Self::MySql => {
                format!("CAST({column} AS CHAR) COLLATE utf8mb4_bin <=> {placeholder}")
            }
            Self::Sqlite => format!("CAST({column} AS TEXT) COLLATE BINARY IS {placeholder}"),
        }
    }

    /// Case-insensitive pattern match of `text_of(column)` against a
    /// parameter whose `%`, `_` and `\` were escaped with `\`.
    pub(crate) fn like(self, column: &str, placeholder: &str, negate: bool) -> String {
        let not = if negate { "NOT " } else { "" };
        let text = self.text_of(column);
        match self {
            Self::Postgres => format!("{text} {not}ILIKE {placeholder}"),
            // `CAST(… AS CHAR)` uses the connection's case-insensitive collation.
            Self::MySql => format!("{text} {not}LIKE {placeholder}"),
            // `LIKE` is ASCII case-insensitive but has no default escape.
            Self::Sqlite => format!("{text} {not}LIKE {placeholder} ESCAPE '\\'"),
        }
    }

    /// Regular-expression match of `text_of(column)`.
    ///
    /// # Errors
    /// [`SelectError::Unsupported`] for `SQLite`, which has no built-in
    /// `REGEXP` function.
    pub(crate) fn regex(
        self,
        column: &str,
        placeholder: &str,
        negate: bool,
    ) -> Result<String, SelectError> {
        let text = self.text_of(column);
        match (self, negate) {
            (Self::Postgres, false) => Ok(format!("{text} ~ {placeholder}")),
            (Self::Postgres, true) => Ok(format!("{text} !~ {placeholder}")),
            (Self::MySql, false) => Ok(format!("{text} REGEXP {placeholder}")),
            (Self::MySql, true) => Ok(format!("{text} NOT REGEXP {placeholder}")),
            (Self::Sqlite, _) => Err(SelectError::Unsupported(
                "SQLite has no regular-expression filter",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_dialect_quotes_with_its_own_character() -> Result<(), SelectError> {
        assert_eq!(Dialect::Postgres.quote_ident("a\"b")?, "\"a\"\"b\"");
        assert_eq!(Dialect::Sqlite.quote_ident("a\"b")?, "\"a\"\"b\"");
        assert_eq!(Dialect::MySql.quote_ident("a`b\"")?, "`a``b\"`");
        assert_eq!(
            Dialect::MySql.qualified("shop", "or`ders")?,
            "`shop`.`or``ders`"
        );
        assert_eq!(
            Dialect::MySql.quote_ident("x\0"),
            Err(SelectError::NulCharacter)
        );
        Ok(())
    }

    #[test]
    fn placeholders_and_text_casts_follow_the_dialect() {
        assert_eq!(Dialect::Postgres.placeholder(3), "$3");
        assert_eq!(Dialect::MySql.placeholder(3), "?");
        assert_eq!(Dialect::Sqlite.placeholder(3), "?3");
        assert_eq!(Dialect::MySql.text_of("`c`"), "CAST(`c` AS CHAR)");
        assert_eq!(Dialect::Sqlite.text_of("\"c\""), "CAST(\"c\" AS TEXT)");
        assert_eq!(
            Dialect::MySql.same_text("`c`", "?"),
            "CAST(`c` AS CHAR) COLLATE utf8mb4_bin <=> ?"
        );
        assert_eq!(
            Dialect::Sqlite.same_text("\"c\"", "?1"),
            "CAST(\"c\" AS TEXT) COLLATE BINARY IS ?1"
        );
    }
}
