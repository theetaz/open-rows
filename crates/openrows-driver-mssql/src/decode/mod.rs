// SPDX-License-Identifier: Apache-2.0
//! Result values into Arrow (docs/04 §5).
//!
//! | SQL Server type | Arrow | Field metadata |
//! | --- | --- | --- |
//! | `tinyint`, `smallint` / `int` / `bigint` | `Int16` / `Int32` / `Int64` | |
//! | `bit` | `Boolean` | |
//! | `real` / `float` | `Float32` / `Float64` | |
//! | `decimal(p, s)`, `numeric(p, s)` | `Decimal128(p, s)` | |
//! | `money` / `smallmoney` | `Decimal128(19, 4)` / `Decimal128(10, 4)` | |
//! | `date` | `Date32` | |
//! | `time(n)` | `Time64(µs)`, `Time64(ns)` for n = 7 | `openrows.fraction_digits = n` |
//! | `datetime2(n)` | `Timestamp(µs)`, `Timestamp(ns)` for n = 7 | `openrows.fraction_digits = n` |
//! | `datetime` / `smalldatetime` | `Timestamp(µs)` | `openrows.fraction_digits` 3 / 0 |
//! | `datetimeoffset(n)` | `Utf8`: local time and offset as the server prints it | |
//! | `uniqueidentifier` | `FixedSizeBinary(16)` | `arrow.uuid` |
//! | `char`, `varchar`, `text`, `nchar`, `nvarchar`, `ntext`, `xml` | `Utf8` | |
//! | `binary`, `varbinary`, `image`, `timestamp`, CLR types | `Binary` | |
//! | `sql_variant` | `Utf8`: the base value's text | `openrows.text_fallback` |
//!
//! `datetime2(7)` values outside 1677–2262 (e.g. the `9999-12-31`
//! sentinel) have no exact 64-bit nanosecond form: the first one switches
//! the column to tagged text for the rest of the result. Every field
//! carries `openrows.source_type`.
pub(crate) mod builder;
mod number;
pub(crate) mod temporal;
pub(crate) mod text;
mod variant;

use std::collections::HashMap;
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use openrows_driver_api::metadata::{
    EXTENSION_NAME, FRACTION_DIGITS, SOURCE_TYPE, TEXT_FALLBACK, UUID_EXTENSION,
};

use crate::tds::collation::Collation;
use crate::tds::token::Column;
use crate::tds::types::SqlType;

/// `user_type` of `timestamp`/`rowversion` columns.
const ROWVERSION: u32 = 80;

/// How one column's wire values become Arrow values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    TinyInt,
    SmallInt,
    Int,
    BigInt,
    Bit,
    Real,
    Float,
    Decimal {
        precision: u8,
        scale: u8,
    },
    SmallMoney,
    Money,
    Date,
    Time {
        scale: u8,
    },
    DateTime2 {
        scale: u8,
    },
    DateTime,
    SmallDateTime,
    DateTimeOffset {
        scale: u8,
    },
    Guid,
    /// Single-byte or UTF-8 text in the collation's code page.
    Char(Collation),
    /// UTF-16 text (`nchar`, `nvarchar`, `ntext`, `xml`).
    Utf16,
    Binary,
    Variant,
    /// The `NULL` type: every value is NULL.
    Null,
}

/// Why one cell could not be decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CellError {
    /// The bytes are not a value of the column's type.
    Malformed(String),
    /// No exact Arrow value; the column switches to tagged text.
    Unrepresentable(&'static str),
}

impl std::fmt::Display for CellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(reason) => f.write_str(reason),
            Self::Unrepresentable(what) => write!(f, "{what} value has no exact Arrow form"),
        }
    }
}

/// The decoding kind of a wire type.
pub(crate) fn kind(sql: &SqlType) -> Kind {
    match sql {
        SqlType::Null => Kind::Null,
        SqlType::TinyInt => Kind::TinyInt,
        SqlType::SmallInt => Kind::SmallInt,
        SqlType::Int => Kind::Int,
        SqlType::BigInt => Kind::BigInt,
        SqlType::Bit => Kind::Bit,
        SqlType::Real => Kind::Real,
        SqlType::Float => Kind::Float,
        SqlType::SmallMoney => Kind::SmallMoney,
        SqlType::Money => Kind::Money,
        SqlType::SmallDateTime => Kind::SmallDateTime,
        SqlType::DateTime => Kind::DateTime,
        SqlType::Guid => Kind::Guid,
        SqlType::Decimal {
            precision, scale, ..
        } => Kind::Decimal {
            precision: *precision,
            scale: *scale,
        },
        SqlType::Date => Kind::Date,
        SqlType::Time { scale } => Kind::Time { scale: *scale },
        SqlType::DateTime2 { scale } => Kind::DateTime2 { scale: *scale },
        SqlType::DateTimeOffset { scale } => Kind::DateTimeOffset { scale: *scale },
        SqlType::Char { collation, .. } | SqlType::Text { collation } => Kind::Char(*collation),
        SqlType::NChar { .. } | SqlType::NText | SqlType::Xml => Kind::Utf16,
        SqlType::Binary { .. } | SqlType::Image | SqlType::Udt { .. } => Kind::Binary,
        SqlType::Variant => Kind::Variant,
    }
}

/// The type name shown for a column, e.g. `nvarchar` or `geography`.
pub(crate) fn source_type(column: &Column) -> String {
    let name = match &column.info.sql {
        SqlType::Udt { name } => return name.clone(),
        SqlType::Binary { .. } if column.user_type == ROWVERSION => "timestamp",
        SqlType::Null => "null",
        SqlType::TinyInt => "tinyint",
        SqlType::SmallInt => "smallint",
        SqlType::Int => "int",
        SqlType::BigInt => "bigint",
        SqlType::Bit => "bit",
        SqlType::Real => "real",
        SqlType::Float => "float",
        SqlType::SmallMoney => "smallmoney",
        SqlType::Money => "money",
        SqlType::SmallDateTime => "smalldatetime",
        SqlType::DateTime => "datetime",
        SqlType::Guid => "uniqueidentifier",
        SqlType::Decimal { numeric: true, .. } => "numeric",
        SqlType::Decimal { .. } => "decimal",
        SqlType::Date => "date",
        SqlType::Time { .. } => "time",
        SqlType::DateTime2 { .. } => "datetime2",
        SqlType::DateTimeOffset { .. } => "datetimeoffset",
        SqlType::Char { varying: true, .. } => "varchar",
        SqlType::Char { .. } => "char",
        SqlType::NChar { varying: true } => "nvarchar",
        SqlType::NChar { .. } => "nchar",
        SqlType::Binary { varying: true } => "varbinary",
        SqlType::Binary { .. } => "binary",
        SqlType::Text { .. } => "text",
        SqlType::NText => "ntext",
        SqlType::Image => "image",
        SqlType::Xml => "xml",
        SqlType::Variant => "sql_variant",
    };
    name.to_owned()
}

/// The Arrow type of a kind.
pub(crate) fn data_type(kind: Kind) -> DataType {
    match kind {
        Kind::TinyInt | Kind::SmallInt => DataType::Int16,
        Kind::Int => DataType::Int32,
        Kind::BigInt => DataType::Int64,
        Kind::Bit => DataType::Boolean,
        Kind::Real => DataType::Float32,
        Kind::Float => DataType::Float64,
        Kind::Decimal { precision, scale } => DataType::Decimal128(precision, scale_of(scale)),
        Kind::SmallMoney => DataType::Decimal128(10, 4),
        Kind::Money => DataType::Decimal128(19, 4),
        Kind::Date => DataType::Date32,
        Kind::Time { scale } => DataType::Time64(unit(scale)),
        Kind::DateTime2 { scale } => DataType::Timestamp(unit(scale), None),
        Kind::DateTime | Kind::SmallDateTime => DataType::Timestamp(TimeUnit::Microsecond, None),
        Kind::Guid => DataType::FixedSizeBinary(16),
        Kind::Binary => DataType::Binary,
        Kind::DateTimeOffset { .. } | Kind::Char(_) | Kind::Utf16 | Kind::Variant | Kind::Null => {
            DataType::Utf8
        }
    }
}

/// Nanoseconds only where microseconds would round (scale 7).
fn unit(scale: u8) -> TimeUnit {
    if scale > 6 {
        TimeUnit::Nanosecond
    } else {
        TimeUnit::Microsecond
    }
}

/// SQL Server scales are at most 38.
fn scale_of(scale: u8) -> i8 {
    i8::try_from(scale).unwrap_or(i8::MAX)
}

/// Fixed fraction digits the server prints for a temporal kind.
fn fraction_digits(kind: Kind) -> Option<u8> {
    match kind {
        Kind::Time { scale } | Kind::DateTime2 { scale } => Some(scale),
        Kind::DateTime => Some(3),
        Kind::SmallDateTime => Some(0),
        _ => None,
    }
}

/// The Arrow schema of a result set.
pub(crate) fn schema(columns: &[Column]) -> SchemaRef {
    let fields: Vec<Field> = columns
        .iter()
        .map(|column| {
            let kind = kind(&column.info.sql);
            let mut metadata = HashMap::from([(SOURCE_TYPE.to_owned(), source_type(column))]);
            if kind == Kind::Guid {
                metadata.insert(EXTENSION_NAME.to_owned(), UUID_EXTENSION.to_owned());
            }
            if kind == Kind::Variant {
                metadata.insert(TEXT_FALLBACK.to_owned(), "true".to_owned());
            }
            if let Some(digits) = fraction_digits(kind) {
                metadata.insert(FRACTION_DIGITS.to_owned(), digits.to_string());
            }
            Field::new(&column.name, data_type(kind), true).with_metadata(metadata)
        })
        .collect();
    Arc::new(Schema::new(fields))
}

/// The same column as tagged text: `Utf8`, source type kept, marked with
/// [`TEXT_FALLBACK`].
pub(crate) fn tagged_text(field: &Field) -> Field {
    let mut metadata = field.metadata().clone();
    metadata.remove(EXTENSION_NAME);
    metadata.remove(FRACTION_DIGITS);
    metadata.insert(TEXT_FALLBACK.to_owned(), "true".to_owned());
    Field::new(field.name(), DataType::Utf8, true).with_metadata(metadata)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
