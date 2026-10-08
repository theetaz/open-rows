// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` DDL generation (docs/10 §3): the reconstructed table
//! definition shown in the DDL popover ([`create_table_sql`]) and the
//! ordered `ALTER TABLE` / `CREATE INDEX` / `COMMENT ON` statements of a
//! staged structure change set ([`alter_sql`]).
//!
//! Identifiers are always double-quoted and comments are quoted literals.
//! Two inputs are SQL by nature and handled explicitly: type names must match
//! a conservative type grammar ([`valid_type`]) and default expressions only
//! come from [`DefaultValue::Expression`], are wrapped in parentheses and are
//! the last clause of their statement; callers additionally verify that every
//! generated statement is exactly one statement.
use openrows_driver_api::schema::DefaultValue;

mod alter;
mod create;

pub use alter::{AlterError, AlterPlan, AlterTable, alter_sql};
pub use create::create_table_sql;

use crate::introspect::{identifier, literal};

/// Longest accepted type name.
const MAX_TYPE: usize = 200;

/// `"schema"."name"`.
fn relation(schema: &str, name: &str) -> Result<String, AlterError> {
    Ok(format!("{}.{}", quote(schema)?, quote(name)?))
}

fn quote(name: &str) -> Result<String, AlterError> {
    identifier(name).map_err(|_| AlterError::InvalidName)
}

/// A comment or text literal; `None` is SQL `NULL`.
fn text(value: Option<&str>) -> Result<String, AlterError> {
    value.map_or_else(
        || Ok("NULL".to_owned()),
        |value| literal(value).map_err(|_| AlterError::InvalidName),
    )
}

/// The validated type text, trimmed.
fn data_type(text: &str) -> Result<String, AlterError> {
    let trimmed = text.trim();
    if valid_type(trimmed) {
        Ok(trimmed.to_owned())
    } else {
        Err(AlterError::InvalidType(text.to_owned()))
    }
}

/// `None` for no default, else the SQL after `DEFAULT`.
fn default_sql(column: &str, default: &DefaultValue) -> Result<Option<String>, AlterError> {
    match default {
        DefaultValue::None => Ok(None),
        DefaultValue::Literal(value) => text(Some(value)).map(Some),
        DefaultValue::Expression(sql) => {
            let sql = sql.trim();
            if sql.is_empty() || sql.contains('\0') {
                Err(AlterError::InvalidDefault(column.to_owned()))
            } else {
                Ok(Some(format!("({sql})")))
            }
        }
    }
}

/// Type syntax: words (`double precision`, `timestamp with time zone`),
/// quoted or schema-qualified names, integer modifiers (`numeric(10, 2)`) and
/// array suffixes (`text[]`). Nothing else: no quotes, operators or `;`.
#[must_use]
pub fn valid_type(text: &str) -> bool {
    if text.is_empty() || text.len() > MAX_TYPE {
        return false;
    }
    let mut chars = text.chars().peekable();
    let mut words = 0;
    while let Some(next) = chars.next() {
        let ok = match next {
            letter if letter.is_alphabetic() || letter == '_' => {
                while chars
                    .next_if(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                    .is_some()
                {}
                words += 1;
                true
            }
            '"' => {
                words += 1;
                quoted(&mut chars)
            }
            '(' => enclosed(&mut chars, ')', |c| c.is_ascii_digit() || " ,-".contains(c)),
            '[' => enclosed(&mut chars, ']', |c| c.is_ascii_digit()),
            ' ' | '.' => true,
            _ => false,
        };
        if !ok {
            return false;
        }
    }
    words > 0
}

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// The rest of a quoted identifier: any text but NUL, `""` escapes a quote.
fn quoted(chars: &mut Chars<'_>) -> bool {
    loop {
        match chars.next() {
            Some('"') if chars.next_if_eq(&'"').is_none() => return true,
            Some('\0') | None => return false,
            Some(_) => {}
        }
    }
}

/// Characters up to `close`, all accepted by `allowed`.
fn enclosed(chars: &mut Chars<'_>, close: char, allowed: fn(char) -> bool) -> bool {
    for next in chars.by_ref() {
        if next == close {
            return true;
        }
        if !allowed(next) {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_types_are_valid() {
        for text in [
            "integer",
            "varchar(255)",
            "character varying(40)",
            "numeric(10, 2)",
            "numeric(10,-2)",
            "double precision",
            "timestamp(3) with time zone",
            "interval day to second",
            "text[]",
            "int4[3][]",
            "public.mood",
            "\"My Type\"",
            "\"odd\"\"name\"",
            "bit varying(8)",
            "pg_catalog.\"char\"",
        ] {
            assert!(valid_type(text), "{text}");
        }
    }

    #[test]
    fn injection_shaped_types_are_rejected() {
        for text in [
            "",
            "int; DROP TABLE t",
            "int -- comment",
            "int /* x */",
            "text DEFAULT 'x'",
            "numeric(a)",
            "varchar(1) )",
            "text[x]",
            "\"open",
            "int\0",
            "int,int",
            "1abc'",
            &"a".repeat(MAX_TYPE + 1),
        ] {
            assert!(!valid_type(text), "{text:?}");
        }
    }

    #[test]
    fn defaults_are_literals_or_parenthesised_expressions() -> Result<(), AlterError> {
        assert_eq!(default_sql("c", &DefaultValue::None)?, None);
        assert_eq!(
            default_sql("c", &DefaultValue::Literal("it's".into()))?,
            Some("'it''s'".into())
        );
        assert_eq!(
            default_sql("c", &DefaultValue::Literal(r"a\b".into()))?,
            Some(r"E'a\\b'".into())
        );
        assert_eq!(
            default_sql("c", &DefaultValue::Expression(" now() ".into()))?,
            Some("(now())".into())
        );
        assert!(default_sql("c", &DefaultValue::Expression("  ".into())).is_err());
        assert!(default_sql("c", &DefaultValue::Literal("a\0".into())).is_err());
        Ok(())
    }

    #[test]
    fn identifiers_double_embedded_quotes() -> Result<(), AlterError> {
        assert_eq!(relation("public", "a\"b")?, "\"public\".\"a\"\"b\"");
        assert!(relation("public", "").is_err());
        assert_eq!(text(None)?, "NULL");
        Ok(())
    }
}
