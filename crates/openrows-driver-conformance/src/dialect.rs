// SPDX-License-Identifier: Apache-2.0
//! Per-dialect SQL and the expected answers: the only engine-specific part
//! of the suite. Adding a dialect means adding its rows here.

use arrow_schema::{DataType, IntervalUnit, TimeUnit};
use openrows_driver_api::metadata::{INET_EXTENSION, RANGE_EXTENSION};
use openrows_sql::Dialect;

/// The Arrow type a value must decode to.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Arrow {
    Int16,
    Int32,
    Int64,
    Float64,
    Boolean,
    Utf8,
    Binary,
    Decimal(u8, i8),
    Date32,
    Time64,
    Interval,
    /// A list of 32-bit integers.
    ListInt32,
}

impl Arrow {
    pub(crate) fn matches(self, data_type: &DataType) -> bool {
        match (self, data_type) {
            (Self::Decimal(precision, scale), DataType::Decimal128(p, s)) => {
                (precision, scale) == (*p, *s)
            }
            (Self::Int16, DataType::Int16)
            | (Self::Int32, DataType::Int32)
            | (Self::Int64, DataType::Int64)
            | (Self::Float64, DataType::Float64)
            | (Self::Boolean, DataType::Boolean)
            | (Self::Utf8, DataType::Utf8)
            | (Self::Binary, DataType::Binary)
            | (Self::Date32, DataType::Date32)
            | (Self::Time64, DataType::Time64(TimeUnit::Microsecond))
            | (Self::Interval, DataType::Interval(IntervalUnit::MonthDayNano)) => true,
            (Self::ListInt32, DataType::List(item)) => item.data_type() == &DataType::Int32,
            _ => false,
        }
    }
}

/// One literal round-trip: `SELECT <expression>` decodes to `arrow` holding
/// `text` (`None` is NULL), with the `ARROW:extension:name` when set.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TypeCase {
    pub(crate) name: &'static str,
    pub(crate) expression: &'static str,
    pub(crate) arrow: Arrow,
    pub(crate) text: Option<&'static str>,
    pub(crate) extension: Option<&'static str>,
}

const fn case(
    name: &'static str,
    expression: &'static str,
    arrow: Arrow,
    text: Option<&'static str>,
) -> TypeCase {
    TypeCase {
        name,
        expression,
        arrow,
        text,
        extension: None,
    }
}

/// Unicode text exercising multi-byte characters and quotes.
pub(crate) const UNICODE: &str = "h\u{e9}llo, \u{4e16}\u{754c} \u{2713} 'q'";

const POSTGRES_TYPES: &[TypeCase] = &[
    case("null", "CAST(NULL AS bigint)", Arrow::Int64, None),
    case(
        "int2",
        "CAST(-32768 AS smallint)",
        Arrow::Int16,
        Some("-32768"),
    ),
    case(
        "int4",
        "CAST(2147483647 AS integer)",
        Arrow::Int32,
        Some("2147483647"),
    ),
    case(
        "int8",
        "CAST(-9007199254740993 AS bigint)",
        Arrow::Int64,
        Some("-9007199254740993"),
    ),
    case(
        "float8",
        "CAST(1.5 AS double precision)",
        Arrow::Float64,
        Some("1.5"),
    ),
    case("bool", "true", Arrow::Boolean, Some("true")),
    case(
        "text",
        "CAST('h\u{e9}llo, \u{4e16}\u{754c} \u{2713} ''q''' AS text)",
        Arrow::Utf8,
        Some(UNICODE),
    ),
    case("empty text", "CAST('' AS text)", Arrow::Utf8, Some("")),
    case(
        "bytea",
        "CAST('\\x00ff10' AS bytea)",
        Arrow::Binary,
        Some("00ff10"),
    ),
    case(
        "numeric",
        "CAST(12.50 AS numeric(10, 2))",
        Arrow::Decimal(10, 2),
        Some("12.50"),
    ),
    case(
        "date",
        "DATE '2024-05-06'",
        Arrow::Date32,
        Some("2024-05-06"),
    ),
    case(
        "time",
        "TIME '13:05:00.25'",
        Arrow::Time64,
        Some("13:05:00.25"),
    ),
    case(
        "interval",
        "INTERVAL '1 year 2 mons -3 days 04:05:06.5'",
        Arrow::Interval,
        Some("1 year 2 mons -3 days +04:05:06.5"),
    ),
    case(
        "int4 array",
        "ARRAY[1, NULL, 3]",
        Arrow::ListInt32,
        Some("{1,NULL,3}"),
    ),
    TypeCase {
        extension: Some(INET_EXTENSION),
        ..case(
            "cidr",
            "CAST('10.1.0.0/16' AS cidr)",
            Arrow::Utf8,
            Some("10.1.0.0/16"),
        )
    },
    TypeCase {
        extension: Some(RANGE_EXTENSION),
        ..case("int4range", "int4range(1, 10)", Arrow::Utf8, Some("[1,10)"))
    },
];

const MYSQL_TYPES: &[TypeCase] = &[
    case("null", "CAST(NULL AS SIGNED)", Arrow::Int64, None),
    case(
        "bigint",
        "CAST(-9007199254740993 AS SIGNED)",
        Arrow::Int64,
        Some("-9007199254740993"),
    ),
    case("double", "1.5e0", Arrow::Float64, Some("1.5")),
    case("boolean", "TRUE", Arrow::Int64, Some("1")),
    case(
        "text",
        "CAST('h\u{e9}llo, \u{4e16}\u{754c} \u{2713} ''q''' AS CHAR)",
        Arrow::Utf8,
        Some(UNICODE),
    ),
    case("empty text", "CAST('' AS CHAR)", Arrow::Utf8, Some("")),
    case("binary", "X'00FF10'", Arrow::Binary, Some("00ff10")),
    case(
        "decimal",
        "CAST(12.50 AS DECIMAL(10, 2))",
        Arrow::Decimal(10, 2),
        Some("12.50"),
    ),
    TypeCase {
        extension: Some(openrows_driver_api::metadata::DECIMAL_EXTENSION),
        ..case(
            "wide decimal",
            "CAST(12.50 AS DECIMAL(65, 2))",
            Arrow::Utf8,
            Some("12.50"),
        )
    },
    case(
        "date",
        "DATE '2024-05-06'",
        Arrow::Date32,
        Some("2024-05-06"),
    ),
];

const SQLITE_TYPES: &[TypeCase] = &[
    case(
        "integer",
        "-9007199254740993",
        Arrow::Int64,
        Some("-9007199254740993"),
    ),
    case("real", "1.5", Arrow::Float64, Some("1.5")),
    case("boolean", "TRUE", Arrow::Int64, Some("1")),
    case(
        "text",
        "'h\u{e9}llo, \u{4e16}\u{754c} \u{2713} ''q'''",
        Arrow::Utf8,
        Some(UNICODE),
    ),
    case("empty text", "''", Arrow::Utf8, Some("")),
    case("blob", "X'00FF10'", Arrow::Binary, Some("00ff10")),
    case(
        "date",
        "DATE('2024-05-06')",
        Arrow::Utf8,
        Some("2024-05-06"),
    ),
];

const SQL_SERVER_TYPES: &[TypeCase] = &[
    case("null", "CAST(NULL AS bigint)", Arrow::Int64, None),
    case(
        "smallint",
        "CAST(-32768 AS smallint)",
        Arrow::Int16,
        Some("-32768"),
    ),
    case(
        "int",
        "CAST(2147483647 AS int)",
        Arrow::Int32,
        Some("2147483647"),
    ),
    case(
        "bigint",
        "CAST(-9007199254740993 AS bigint)",
        Arrow::Int64,
        Some("-9007199254740993"),
    ),
    case("float", "CAST(1.5 AS float)", Arrow::Float64, Some("1.5")),
    case("bit", "CAST(1 AS bit)", Arrow::Boolean, Some("true")),
    case(
        "nvarchar",
        "N'h\u{e9}llo, \u{4e16}\u{754c} \u{2713} ''q'''",
        Arrow::Utf8,
        Some(UNICODE),
    ),
    case(
        "empty text",
        "CAST(N'' AS nvarchar(10))",
        Arrow::Utf8,
        Some(""),
    ),
    case(
        "varbinary",
        "CAST(0x00FF10 AS varbinary(3))",
        Arrow::Binary,
        Some("00ff10"),
    ),
    case(
        "decimal",
        "CAST(12.50 AS decimal(10, 2))",
        Arrow::Decimal(10, 2),
        Some("12.50"),
    ),
    case(
        "date",
        "CAST('2024-05-06' AS date)",
        Arrow::Date32,
        Some("2024-05-06"),
    ),
    case(
        "time",
        "CAST('13:05:00.25' AS time(2))",
        Arrow::Time64,
        Some("13:05:00.25"),
    ),
];

const CLICKHOUSE_TYPES: &[TypeCase] = &[
    case("null", "CAST(NULL AS Nullable(Int64))", Arrow::Int64, None),
    case(
        "int8",
        "toInt64(-9007199254740993)",
        Arrow::Int64,
        Some("-9007199254740993"),
    ),
    case("float", "toFloat64(1.5)", Arrow::Float64, Some("1.5")),
    case(
        "text",
        "CAST('héllo, 世界 ✓ ''q''' AS String)",
        Arrow::Utf8,
        Some(UNICODE),
    ),
    case("empty", "CAST('' AS String)", Arrow::Utf8, Some("")),
    case(
        "decimal",
        "CAST('12.50' AS Decimal(10, 2))",
        Arrow::Decimal(10, 2),
        Some("12.50"),
    ),
    case(
        "date",
        "toDate32('2024-05-06')",
        Arrow::Date32,
        Some("2024-05-06"),
    ),
];

/// The literal round-trips of `dialect`.
const DUCKDB_TYPES: &[TypeCase] = &[
    case("null", "CAST(NULL AS BIGINT)", Arrow::Int64, None),
    case(
        "smallint",
        "CAST(-32768 AS SMALLINT)",
        Arrow::Int16,
        Some("-32768"),
    ),
    case("integer", "2147483647", Arrow::Int32, Some("2147483647")),
    case(
        "bigint",
        "CAST(-9007199254740993 AS BIGINT)",
        Arrow::Int64,
        Some("-9007199254740993"),
    ),
    case("double", "CAST(1.5 AS DOUBLE)", Arrow::Float64, Some("1.5")),
    case("bool", "true", Arrow::Boolean, Some("true")),
    case("text", "'héllo, 世界 ✓ ''q'''", Arrow::Utf8, Some(UNICODE)),
    case("empty", "''", Arrow::Utf8, Some("")),
    case("blob", "from_hex('00FF10')", Arrow::Binary, Some("00ff10")),
    case(
        "decimal",
        "12.50::DECIMAL(10, 2)",
        Arrow::Decimal(10, 2),
        Some("12.50"),
    ),
    case(
        "date",
        "DATE '2024-05-06'",
        Arrow::Date32,
        Some("2024-05-06"),
    ),
    case(
        "time",
        "TIME '13:05:00.25'",
        Arrow::Time64,
        Some("13:05:00.25"),
    ),
    case("list", "[1, NULL, 3]", Arrow::ListInt32, Some("{1,NULL,3}")),
];

/// The literal round-trips of `dialect`.
pub(crate) fn type_cases(dialect: Dialect) -> &'static [TypeCase] {
    match dialect {
        Dialect::Duckdb => DUCKDB_TYPES,
        Dialect::MySql => MYSQL_TYPES,
        Dialect::Sqlite => SQLITE_TYPES,
        Dialect::SqlServer => SQL_SERVER_TYPES,
        Dialect::ClickHouse => CLICKHOUSE_TYPES,
        _ => POSTGRES_TYPES,
    }
}

/// Digits 0–9 as a derived table every dialect accepts.
const DIGITS: &str = "(SELECT 0 AS d UNION ALL SELECT 1 UNION ALL SELECT 2 UNION ALL \
    SELECT 3 UNION ALL SELECT 4 UNION ALL SELECT 5 UNION ALL SELECT 6 UNION ALL \
    SELECT 7 UNION ALL SELECT 8 UNION ALL SELECT 9)";

/// Rows `1..=10_000` in one column `n`, ordered.
pub(crate) fn series() -> String {
    format!(
        "SELECT a.d + 10 * b.d + 100 * c.d + 1000 * e.d + 1 AS n \
         FROM {DIGITS} a CROSS JOIN {DIGITS} b CROSS JOIN {DIGITS} c CROSS JOIN {DIGITS} e \
         ORDER BY n"
    )
}

/// Rows in [`series`].
pub(crate) const SERIES_ROWS: usize = 10_000;

/// Counts 10^12 rows: runs for hours unless cancelled, on every engine.
pub(crate) fn endless() -> String {
    let joins = (0..12)
        .map(|i| format!("{DIGITS} t{i}"))
        .collect::<Vec<_>>()
        .join(" CROSS JOIN ");
    format!("SELECT count(*) FROM {joins}")
}

/// Sleeps `seconds` unless interrupted, then fails with the cancel SQLSTATE.
/// `MySQL`'s `SLEEP()` alone returns 1 when killed instead of failing, so
/// it sleeps inside a filtered derived table. SQL Server's `WAITFOR` names
/// no `sleep(`, so a comment does for the process list.
pub(crate) fn sleep(dialect: Dialect, seconds: u32) -> String {
    match dialect {
        Dialect::MySql => format!(
            "SELECT COUNT(*) FROM (SELECT 1 AS n UNION ALL SELECT 2) AS orc_sleep \
             WHERE SLEEP({seconds}) = 0"
        ),
        Dialect::SqlServer => format!(
            "/* orc sleep({seconds}) */ WAITFOR DELAY '{:02}:{:02}:{:02}'",
            seconds / 3_600,
            seconds / 60 % 60,
            seconds % 60
        ),
        Dialect::ClickHouse => format!("SELECT sleep({})", seconds.min(3)),
        _ => format!("SELECT pg_sleep({seconds})"),
    }
}

/// Expected SQLSTATEs of common failures.
pub(crate) struct Codes {
    pub(crate) syntax: &'static str,
    pub(crate) undefined_table: &'static str,
    pub(crate) unique: &'static str,
    pub(crate) not_null: &'static str,
    pub(crate) cancelled: &'static str,
}

/// `MySQL` reports several classes with generic SQLSTATEs (`42000`, `23000`)
/// and `ER_QUERY_INTERRUPTED` as `70100`; SQL Server follows the ODBC
/// driver's SQLSTATEs (cancel `HY008`); the others follow `PostgreSQL`.
pub(crate) fn codes(dialect: Dialect) -> Codes {
    match dialect {
        Dialect::ClickHouse => Codes {
            syntax: "42601",
            undefined_table: "42P01",
            unique: "HY000",
            not_null: "23502",
            cancelled: "57014",
        },
        Dialect::SqlServer => Codes {
            syntax: "42000",
            undefined_table: "42S02",
            unique: "23000",
            not_null: "23000",
            cancelled: "HY008",
        },
        Dialect::MySql => Codes {
            syntax: "42000",
            undefined_table: "42S02",
            unique: "23000",
            not_null: "23000",
            cancelled: "70100",
        },
        _ => Codes {
            syntax: "42601",
            undefined_table: "42P01",
            unique: "23505",
            not_null: "23502",
            cancelled: "57014",
        },
    }
}

/// True when the server can cancel another session's statement and keep
/// that session (`pg_cancel_backend`, `KILL QUERY`); SQL Server can only
/// end the session.
pub(crate) fn cancels_other_sessions(dialect: Dialect) -> bool {
    dialect != Dialect::SqlServer
}

/// A keyed fixture table every dialect accepts.
pub(crate) fn create_table(relation: &str) -> String {
    format!(
        "CREATE TABLE {relation} (id INTEGER PRIMARY KEY, \
         name VARCHAR(40) NOT NULL UNIQUE, note TEXT)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_expectations_check_precision_and_scale() {
        assert!(Arrow::Decimal(10, 2).matches(&DataType::Decimal128(10, 2)));
        assert!(!Arrow::Decimal(10, 2).matches(&DataType::Decimal128(10, 3)));
        assert!(!Arrow::Int64.matches(&DataType::Int32));
        assert_eq!(codes(Dialect::Sqlite).cancelled, "57014");
        assert!(type_cases(Dialect::Postgres).len() > type_cases(Dialect::Sqlite).len());
        assert_eq!(endless().matches("CROSS JOIN").count(), 11);
        assert!(sleep(Dialect::MySql, 5).contains("SLEEP(5)"));
        assert_eq!(sleep(Dialect::Postgres, 5), "SELECT pg_sleep(5)");
        assert_eq!(
            sleep(Dialect::SqlServer, 61),
            "/* orc sleep(61) */ WAITFOR DELAY '00:01:01'"
        );
        assert!(!cancels_other_sessions(Dialect::SqlServer));
        assert!(cancels_other_sessions(Dialect::MySql));
    }
}
