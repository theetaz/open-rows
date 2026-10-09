// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` type OIDs → logical kinds and Arrow fields (docs/04 §5).
mod names;

use arrow_schema::{DataType, Field, IntervalUnit, TimeUnit};
use openrows_driver_api::metadata::{
    DECIMAL_EXTENSION, EXTENSION_NAME, INET_EXTENSION, JSON_EXTENSION, MACADDR_EXTENSION,
    RANGE_EXTENSION, SOURCE_TYPE, TEXT_FALLBACK, UUID_EXTENSION,
};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) use names::array_element;
use names::{builtin_name, is_range};

/// One result column as described by the server's `RowDescription`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColumnDesc {
    /// Column label.
    pub name: String,
    /// `pg_type` OID.
    pub type_oid: u32,
    /// Type modifier (`atttypmod`), e.g. numeric precision/scale; −1 when absent.
    pub type_modifier: i32,
}

/// How a column is requested on the wire and decoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ColumnKind {
    Int2,
    Int4,
    Int8,
    Float4,
    Float8,
    Bool,
    Text,
    Bytea,
    Timestamp,
    TimestampTz,
    Date,
    /// `time`: `Time64(µs)`; `24:00:00` falls back to tagged text.
    Time,
    /// `timetz`: the server's text form (Arrow has no zoned time of day).
    TimeTz,
    /// `interval`: `Interval(MonthDayNano)`.
    Interval,
    Uuid,
    /// `numeric(p, s)` with 0 ≤ s ≤ p ≤ 38.
    Decimal {
        precision: u8,
        scale: i8,
    },
    /// Any other numeric, kept exact as text.
    NumericText,
    Json,
    Jsonb,
    /// `inet` and `cidr` as text, tagged [`INET_EXTENSION`].
    Inet,
    /// `macaddr` and `macaddr8` as text, tagged [`MACADDR_EXTENSION`].
    MacAddr,
    /// Built-in range and multirange types, requested in text format.
    Range,
    /// A one-dimensional array of a supported element type: `List<element>`.
    Array {
        element_oid: u32,
        type_modifier: i32,
    },
    /// Requested in text format; stored as the server's text representation.
    Unknown,
}

const NUMERIC_OID: u32 = 1700;
const VARHDRSZ: i32 = 4;

impl ColumnKind {
    pub(crate) fn of(desc: &ColumnDesc) -> Self {
        match Self::scalar(desc.type_oid, desc.type_modifier) {
            Self::Unknown => match array_element(desc.type_oid) {
                Some(element_oid) => Self::Array {
                    element_oid,
                    type_modifier: desc.type_modifier,
                },
                None if is_range(desc.type_oid) => Self::Range,
                None => Self::Unknown,
            },
            kind => kind,
        }
    }

    /// The kind of a non-array type; `Unknown` for anything else.
    pub(crate) fn scalar(type_oid: u32, type_modifier: i32) -> Self {
        match type_oid {
            16 => Self::Bool,
            17 => Self::Bytea,
            19 | 25 | 1042 | 1043 => Self::Text,
            20 => Self::Int8,
            21 => Self::Int2,
            23 => Self::Int4,
            114 => Self::Json,
            650 | 869 => Self::Inet,
            700 => Self::Float4,
            701 => Self::Float8,
            774 | 829 => Self::MacAddr,
            1082 => Self::Date,
            1083 => Self::Time,
            1114 => Self::Timestamp,
            1184 => Self::TimestampTz,
            1186 => Self::Interval,
            1266 => Self::TimeTz,
            NUMERIC_OID => numeric_kind(type_modifier),
            2950 => Self::Uuid,
            3802 => Self::Jsonb,
            _ => Self::Unknown,
        }
    }

    /// The element kind of an array column.
    pub(crate) fn element(self) -> Option<Self> {
        match self {
            Self::Array {
                element_oid,
                type_modifier,
            } => Some(Self::scalar(element_oid, type_modifier)),
            _ => None,
        }
    }

    /// Wire result format code: 1 = binary, 0 = text.
    pub(crate) fn format_code(self) -> i16 {
        i16::from(!matches!(self, Self::Unknown | Self::Range))
    }

    pub(crate) fn data_type(self) -> DataType {
        match self {
            Self::Int2 => DataType::Int16,
            Self::Int4 => DataType::Int32,
            Self::Int8 => DataType::Int64,
            Self::Float4 => DataType::Float32,
            Self::Float8 => DataType::Float64,
            Self::Bool => DataType::Boolean,
            Self::Bytea => DataType::Binary,
            Self::Timestamp => DataType::Timestamp(TimeUnit::Microsecond, None),
            Self::TimestampTz => DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            Self::Date => DataType::Date32,
            Self::Time => DataType::Time64(TimeUnit::Microsecond),
            Self::Interval => DataType::Interval(IntervalUnit::MonthDayNano),
            Self::Uuid => DataType::FixedSizeBinary(16),
            Self::Decimal { precision, scale } => DataType::Decimal128(precision, scale),
            Self::Array {
                element_oid,
                type_modifier,
            } => DataType::List(Arc::new(element_field(element_oid, type_modifier))),
            Self::Text
            | Self::TimeTz
            | Self::NumericText
            | Self::Json
            | Self::Jsonb
            | Self::Inet
            | Self::MacAddr
            | Self::Range
            | Self::Unknown => DataType::Utf8,
        }
    }

    fn extension(self) -> Option<&'static str> {
        match self {
            Self::Uuid => Some(UUID_EXTENSION),
            Self::Json | Self::Jsonb => Some(JSON_EXTENSION),
            Self::NumericText => Some(DECIMAL_EXTENSION),
            Self::Inet => Some(INET_EXTENSION),
            Self::MacAddr => Some(MACADDR_EXTENSION),
            Self::Range => Some(RANGE_EXTENSION),
            _ => None,
        }
    }
}

/// The `item` field of an array column: the element's Arrow type, source
/// type name and extension tag.
fn element_field(element_oid: u32, type_modifier: i32) -> Field {
    let kind = ColumnKind::scalar(element_oid, type_modifier);
    let source = builtin_name(element_oid).unwrap_or("unknown");
    let mut metadata = HashMap::from([(SOURCE_TYPE.to_owned(), source.to_owned())]);
    if let Some(extension) = kind.extension() {
        metadata.insert(EXTENSION_NAME.to_owned(), extension.to_owned());
    }
    Field::new("item", kind.data_type(), true).with_metadata(metadata)
}

/// Typmod layout from `PostgreSQL`'s `numeric.c`: ((precision << 16) | scale) + VARHDRSZ.
fn numeric_kind(type_modifier: i32) -> ColumnKind {
    if type_modifier < VARHDRSZ {
        return ColumnKind::NumericText;
    }
    let packed = type_modifier - VARHDRSZ;
    let precision = (packed >> 16) & 0xffff;
    let scale = packed & 0x7ff;
    match (u8::try_from(precision), i8::try_from(scale)) {
        (Ok(precision), Ok(scale))
            if precision <= 38 && i32::from(scale) <= i32::from(precision) =>
        {
            ColumnKind::Decimal { precision, scale }
        }
        _ => ColumnKind::NumericText,
    }
}

/// True when the session must look up this column's type name.
pub(crate) fn needs_name(desc: &ColumnDesc, names: &HashMap<u32, String>) -> bool {
    builtin_name(desc.type_oid).is_none() && !names.contains_key(&desc.type_oid)
}

/// Arrow field carrying the source type name and any extension tag.
pub(crate) fn field(desc: &ColumnDesc, kind: ColumnKind, names: &HashMap<u32, String>) -> Field {
    let source = builtin_name(desc.type_oid)
        .map(str::to_owned)
        .or_else(|| names.get(&desc.type_oid).cloned())
        .unwrap_or_else(|| format!("oid:{}", desc.type_oid));
    let mut metadata = HashMap::from([(SOURCE_TYPE.to_owned(), source)]);
    if let Some(extension) = kind.extension() {
        metadata.insert(EXTENSION_NAME.to_owned(), extension.to_owned());
    }
    Field::new(&desc.name, kind.data_type(), true).with_metadata(metadata)
}

/// The same column as tagged text: `Utf8`, original source type kept, marked
/// with [`TEXT_FALLBACK`].
pub(crate) fn tagged_text(field: &Field) -> Field {
    let mut metadata = field.metadata().clone();
    metadata.remove(EXTENSION_NAME);
    metadata.insert(TEXT_FALLBACK.to_owned(), "true".to_owned());
    Field::new(field.name(), DataType::Utf8, true).with_metadata(metadata)
}

#[cfg(test)]
#[path = "types/tests.rs"]
mod tests;
