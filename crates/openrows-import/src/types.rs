// SPDX-License-Identifier: Apache-2.0
//! Value categories: what inference finds in a column, which values a
//! target column type accepts, and the type a new table gets per dialect.
use std::borrow::Cow;

use openrows_sql::Dialect;

use crate::temporal::{Temporal, temporal};

/// The kind of values a column holds or accepts.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TypeCategory {
    /// Whole numbers.
    Integer,
    /// Decimal or floating-point numbers.
    Decimal,
    /// `true`/`false`.
    Boolean,
    /// Calendar dates (`YYYY-MM-DD`).
    Date,
    /// Date and time without a zone.
    Timestamp,
    /// Date and time with a zone or offset.
    TimestampTz,
    /// JSON documents (objects and arrays).
    Json,
    /// Anything else; every value is accepted.
    Text,
}

/// Longest text a new `MySQL` column stores as `VARCHAR(255)` (keyable).
const VARCHAR_LIMIT: usize = 255;

impl TypeCategory {
    /// Lower-case name for messages, e.g. `integer`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Integer => "integer",
            Self::Decimal => "number",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Timestamp => "timestamp",
            Self::TimestampTz => "timestamp with time zone",
            Self::Json => "JSON",
            Self::Text => "text",
        }
    }

    /// The category of a column type as the catalog formats it, e.g.
    /// `character varying(40)`, `bigint unsigned` or `timestamp with time zone`.
    #[must_use]
    pub fn of_type(data_type: &str) -> Self {
        let lower = data_type.trim().to_ascii_lowercase();
        if lower.ends_with("[]") {
            return Self::Text;
        }
        let base = lower.split('(').next().unwrap_or_default().trim();
        let first = base.split_whitespace().next().unwrap_or_default();
        match first {
            "smallint" | "integer" | "int" | "bigint" | "int2" | "int4" | "int8" | "tinyint"
            | "mediumint" | "serial" | "bigserial" | "smallserial" | "year" => Self::Integer,
            "numeric" | "decimal" | "real" | "double" | "float" | "float4" | "float8" | "dec"
            | "fixed" => Self::Decimal,
            "boolean" | "bool" => Self::Boolean,
            "date" => Self::Date,
            "timestamptz" => Self::TimestampTz,
            "timestamp" if base.contains("with time zone") && !base.contains("without") => {
                Self::TimestampTz
            }
            "timestamp" | "datetime" => Self::Timestamp,
            "json" | "jsonb" => Self::Json,
            _ => Self::Text,
        }
    }

    /// True when `value` can be stored in a column of this category. More
    /// lenient than inference: leading zeros, `yes`/`no`, dates in timestamp
    /// columns.
    #[must_use]
    pub fn accepts(self, value: &str) -> bool {
        match self {
            Self::Integer => is_integer(value),
            Self::Decimal => is_integer(value) || is_decimal(value),
            Self::Boolean => boolean(value).is_some(),
            Self::Date => temporal(value) == Some(Temporal::Date),
            Self::Timestamp | Self::TimestampTz => temporal(value).is_some(),
            Self::Json => serde_json::from_str::<serde::de::IgnoredAny>(value).is_ok(),
            Self::Text => true,
        }
    }

    /// `value` as the engine reads it: booleans become `true`/`false`
    /// (`PostgreSQL`) or `1`/`0`; everything else is sent as written.
    #[must_use]
    pub fn normalise(self, dialect: Dialect, value: &str) -> Cow<'_, str> {
        match (self, boolean(value)) {
            (Self::Boolean, Some(truth)) => Cow::Borrowed(match (dialect, truth) {
                (Dialect::Postgres, true) => "true",
                (Dialect::Postgres, false) => "false",
                (_, true) => "1",
                (_, false) => "0",
            }),
            _ => Cow::Borrowed(value),
        }
    }

    /// The column type a new table gets for this category; `longest` is the
    /// longest text seen (`MySQL` keeps short text keyable as `VARCHAR`).
    #[must_use]
    pub fn default_type(self, dialect: Dialect, longest: usize) -> &'static str {
        match dialect {
            Dialect::MySql => self.mysql_type(longest),
            Dialect::Sqlite => match self {
                Self::Integer | Self::Boolean => "INTEGER",
                Self::Decimal => "REAL",
                _ => "TEXT",
            },
            Dialect::SqlServer => self.sql_server_type(longest),
            Dialect::ClickHouse => match self {
                Self::Integer => "Int64",
                Self::Decimal => "Decimal(38, 10)",
                Self::Boolean => "Bool",
                Self::Date => "Date32",
                Self::Timestamp | Self::TimestampTz => "DateTime64(6, 'UTC')",
                Self::Json | Self::Text => "String",
            },
            _ => match self {
                Self::Integer => "bigint",
                Self::Decimal => "numeric",
                Self::Boolean => "boolean",
                Self::Date => "date",
                Self::Timestamp => "timestamp",
                Self::TimestampTz => "timestamptz",
                Self::Json => "jsonb",
                Self::Text => "text",
            },
        }
    }

    /// Short text stays keyable (`nvarchar(max)` cannot be a key).
    fn sql_server_type(self, longest: usize) -> &'static str {
        match self {
            Self::Integer => "bigint",
            Self::Decimal => "float",
            Self::Boolean => "bit",
            Self::Date => "date",
            Self::Timestamp => "datetime2",
            Self::TimestampTz => "datetimeoffset",
            Self::Text if longest <= VARCHAR_LIMIT => "nvarchar(255)",
            Self::Json | Self::Text => "nvarchar(max)",
        }
    }

    fn mysql_type(self, longest: usize) -> &'static str {
        match self {
            Self::Integer => "BIGINT",
            Self::Decimal => "DOUBLE",
            Self::Boolean => "BOOLEAN",
            Self::Date => "DATE",
            Self::Timestamp | Self::TimestampTz => "DATETIME(6)",
            Self::Json => "JSON",
            Self::Text if longest <= VARCHAR_LIMIT => "VARCHAR(255)",
            Self::Text => "TEXT",
        }
    }
}

/// Optional sign, then ASCII digits only.
pub(crate) fn is_integer(value: &str) -> bool {
    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// `1.5`, `.5`, `1.`, `1e10`, `-2.5E-3`: a number with a point or exponent.
pub(crate) fn is_decimal(value: &str) -> bool {
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(index) => (&unsigned[..index], Some(&unsigned[index + 1..])),
        None => (unsigned, None),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok =
        (!whole.is_empty() || !fraction.is_empty()) && digits(whole) && digits(fraction);
    let exponent_ok = exponent.is_none_or(is_integer);
    mantissa_ok && exponent_ok && (mantissa.contains('.') || exponent.is_some())
}

/// The truth value of the common boolean spellings.
pub(crate) fn boolean(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "t" | "yes" | "y" | "on" | "1" => Some(true),
        "false" | "f" | "no" | "n" | "off" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::TypeCategory::{self, *};
    use openrows_sql::Dialect;

    #[test]
    fn catalog_types_map_to_categories() {
        for (name, category) in [
            ("integer", Integer),
            ("bigint unsigned", Integer),
            ("int(11)", Integer),
            ("numeric(10, 2)", Decimal),
            ("double precision", Decimal),
            ("tinyint(1)", Integer),
            ("boolean", Boolean),
            ("date", Date),
            ("timestamp without time zone", Timestamp),
            ("timestamp with time zone", TimestampTz),
            ("DATETIME(6)", Timestamp),
            ("jsonb", Json),
            ("character varying(40)", Text),
            ("integer[]", Text),
            ("uuid", Text),
        ] {
            assert_eq!(TypeCategory::of_type(name), category, "{name}");
        }
    }

    #[test]
    fn checks_are_lenient_and_booleans_normalise_per_dialect() {
        assert!(Integer.accepts("007"));
        assert!(!Integer.accepts("1.5"));
        assert!(!Integer.accepts("-"));
        assert!(Decimal.accepts("-2.5E-3"));
        assert!(Decimal.accepts("42"));
        assert!(!Decimal.accepts("1e"));
        assert!(!Decimal.accepts("."));
        assert!(Boolean.accepts("Yes"));
        assert!(!Boolean.accepts("maybe"));
        assert!(Timestamp.accepts("2024-01-02"));
        assert!(!Date.accepts("2024-01-02 03:04"));
        assert!(Json.accepts("{\"a\": [1]}"));
        assert!(!Json.accepts("{a}"));
        assert!(Text.accepts(""));
        assert_eq!(Boolean.normalise(Dialect::Postgres, "YES"), "true");
        assert_eq!(Boolean.normalise(Dialect::MySql, "f"), "0");
        assert_eq!(Integer.normalise(Dialect::MySql, "007"), "007");
    }

    #[test]
    fn new_tables_get_dialect_types() {
        assert_eq!(Decimal.default_type(Dialect::Postgres, 0), "numeric");
        assert_eq!(
            TimestampTz.default_type(Dialect::Postgres, 0),
            "timestamptz"
        );
        assert_eq!(Text.default_type(Dialect::MySql, 10), "VARCHAR(255)");
        assert_eq!(Text.default_type(Dialect::MySql, 300), "TEXT");
        assert_eq!(Boolean.default_type(Dialect::Sqlite, 0), "INTEGER");
        assert_eq!(Json.default_type(Dialect::Sqlite, 0), "TEXT");
        assert_eq!(Boolean.default_type(Dialect::SqlServer, 0), "bit");
        assert_eq!(Text.default_type(Dialect::SqlServer, 10), "nvarchar(255)");
        assert_eq!(Text.default_type(Dialect::SqlServer, 300), "nvarchar(max)");
        assert_eq!(Boolean.normalise(Dialect::SqlServer, "yes"), "1");
    }
}
