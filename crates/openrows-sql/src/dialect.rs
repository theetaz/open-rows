// SPDX-License-Identifier: Apache-2.0
//! Per-dialect spelling of the SQL that [`crate::select`] and
//! [`crate::edit`] generate: identifier quoting, bind placeholders, text
//! casts, NULL-safe equality and pattern operators.

use crate::Dialect;
use crate::select::SelectError;

impl Dialect {
    /// `name` as a quoted identifier: `"name"` (`PostgreSQL`, `SQLite`),
    /// `` `name` `` (`MySQL`) or `[name]` (SQL Server), with embedded quote
    /// characters doubled.
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
            Self::SqlServer => format!("[{}]", name.replace(']', "]]")),
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
    /// `CAST(expr AS CHAR)`, `CAST(expr AS TEXT)` or SQL Server's
    /// `CONVERT(NVARCHAR(MAX), expr)`. SQL Server's default text of some
    /// types loses precision (`float`, `money`, `datetime`), so its driver
    /// replaces this expression with an exact per-type form in edit
    /// snapshots and guards.
    #[must_use]
    pub fn text_of(self, expression: &str) -> String {
        match self {
            Self::Postgres => format!("{expression}::text"),
            Self::MySql => format!("CAST({expression} AS CHAR)"),
            Self::Sqlite => format!("CAST({expression} AS TEXT)"),
            Self::SqlServer => format!("CONVERT(NVARCHAR(MAX), {expression})"),
        }
    }

    /// The bind placeholder for the `number`-th parameter (1-based).
    pub(crate) fn placeholder(self, number: usize) -> String {
        match self {
            Self::Postgres => format!("${number}"),
            Self::MySql => "?".to_owned(),
            Self::Sqlite => format!("?{number}"),
            Self::SqlServer => format!("@P{number}"),
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
            // `=` on text ignores trailing spaces even in a binary collation;
            // the UTF-16 bytes compare exactly (the text never holds NUL).
            Self::SqlServer => format!(
                "(CONVERT(VARBINARY(MAX), {}) = CONVERT(VARBINARY(MAX), {placeholder}) \
                 OR ({column} IS NULL AND {placeholder} IS NULL))",
                self.text_of(column)
            ),
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
            // No default escape; `SQLite` matches ASCII case-insensitively,
            // SQL Server by the column's (usually case-insensitive) collation.
            Self::Sqlite | Self::SqlServer => {
                format!("{text} {not}LIKE {placeholder} ESCAPE '\\'")
            }
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
            (Self::SqlServer, _) => Err(SelectError::Unsupported(
                "SQL Server has no regular-expression filter",
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

    #[test]
    fn sql_server_quotes_with_brackets_and_binds_at_p() -> Result<(), SelectError> {
        let tsql = Dialect::SqlServer;
        assert_eq!(tsql.quote_ident("a]b[")?, "[a]]b[]");
        assert_eq!(tsql.qualified("dbo", "or ders")?, "[dbo].[or ders]");
        assert_eq!(tsql.placeholder(2), "@P2");
        assert_eq!(tsql.text_of("[c]"), "CONVERT(NVARCHAR(MAX), [c])");
        assert_eq!(
            tsql.same_text("[c]", "@P1"),
            "(CONVERT(VARBINARY(MAX), CONVERT(NVARCHAR(MAX), [c])) = CONVERT(VARBINARY(MAX), @P1) \
             OR ([c] IS NULL AND @P1 IS NULL))"
        );
        assert_eq!(
            tsql.like("[c]", "@P1", true),
            "CONVERT(NVARCHAR(MAX), [c]) NOT LIKE @P1 ESCAPE '\\'"
        );
        assert!(tsql.regex("[c]", "@P1", false).is_err());
        Ok(())
    }
}
