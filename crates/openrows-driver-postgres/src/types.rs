// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` type OIDs → logical kinds and Arrow fields (docs/04 §5).
use arrow_schema::{DataType, Field, TimeUnit};
use openrows_driver_api::metadata::{
    DECIMAL_EXTENSION, EXTENSION_NAME, JSON_EXTENSION, SOURCE_TYPE, TEXT_FALLBACK, UUID_EXTENSION,
};
use std::collections::HashMap;

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
    /// Requested in text format; stored as the server's text representation.
    Unknown,
}

const NUMERIC_OID: u32 = 1700;
const VARHDRSZ: i32 = 4;

impl ColumnKind {
    pub(crate) fn of(desc: &ColumnDesc) -> Self {
        match desc.type_oid {
            16 => Self::Bool,
            17 => Self::Bytea,
            19 | 25 | 1042 | 1043 => Self::Text,
            20 => Self::Int8,
            21 => Self::Int2,
            23 => Self::Int4,
            114 => Self::Json,
            700 => Self::Float4,
            701 => Self::Float8,
            1082 => Self::Date,
            1114 => Self::Timestamp,
            1184 => Self::TimestampTz,
            NUMERIC_OID => numeric_kind(desc.type_modifier),
            2950 => Self::Uuid,
            3802 => Self::Jsonb,
            _ => Self::Unknown,
        }
    }

    /// Wire result format code: 1 = binary, 0 = text.
    pub(crate) fn format_code(self) -> i16 {
        i16::from(self != Self::Unknown)
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
            Self::Uuid => DataType::FixedSizeBinary(16),
            Self::Decimal { precision, scale } => DataType::Decimal128(precision, scale),
            Self::Text | Self::NumericText | Self::Json | Self::Jsonb | Self::Unknown => {
                DataType::Utf8
            }
        }
    }

    fn extension(self) -> Option<&'static str> {
        match self {
            Self::Uuid => Some(UUID_EXTENSION),
            Self::Json | Self::Jsonb => Some(JSON_EXTENSION),
            Self::NumericText => Some(DECIMAL_EXTENSION),
            _ => None,
        }
    }
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

/// Built-in type names; other names come from the session's `pg_type` lookup.
fn builtin_name(oid: u32) -> Option<&'static str> {
    Some(match oid {
        16 => "bool",
        17 => "bytea",
        19 => "name",
        20 => "int8",
        21 => "int2",
        23 => "int4",
        25 => "text",
        114 => "json",
        700 => "float4",
        701 => "float8",
        1042 => "bpchar",
        1043 => "varchar",
        1082 => "date",
        1114 => "timestamp",
        1184 => "timestamptz",
        NUMERIC_OID => "numeric",
        2950 => "uuid",
        3802 => "jsonb",
        _ => return None,
    })
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
mod tests {
    use super::*;

    fn desc(type_oid: u32, type_modifier: i32) -> ColumnDesc {
        ColumnDesc {
            name: "c".to_owned(),
            type_oid,
            type_modifier,
        }
    }

    fn typmod(precision: i32, scale: i32) -> i32 {
        ((precision << 16) | scale) + VARHDRSZ
    }

    #[test]
    fn numeric_fits_decimal128_only_with_known_precision_up_to_38() {
        let kind = |modifier| ColumnKind::of(&desc(NUMERIC_OID, modifier));
        assert_eq!(kind(-1), ColumnKind::NumericText);
        assert_eq!(kind(typmod(10, 2)).data_type(), DataType::Decimal128(10, 2));
        assert_eq!(
            kind(typmod(38, 38)).data_type(),
            DataType::Decimal128(38, 38)
        );
        assert_eq!(kind(typmod(39, 2)), ColumnKind::NumericText);
        assert_eq!(kind(typmod(1000, 0)), ColumnKind::NumericText);
    }

    #[test]
    fn unknown_types_use_text_format_and_carry_their_name() {
        let interval = desc(1186, -1);
        let names = HashMap::from([(1186, "interval".to_owned())]);
        let kind = ColumnKind::of(&interval);
        assert_eq!((kind, kind.format_code()), (ColumnKind::Unknown, 0));
        assert!(!needs_name(&interval, &names));
        assert!(needs_name(&interval, &HashMap::new()));
        let field = field(&interval, kind, &names);
        assert_eq!(field.data_type(), &DataType::Utf8);
        assert_eq!(field.metadata()[SOURCE_TYPE], "interval");
    }

    #[test]
    fn extension_types_are_tagged() {
        let names = HashMap::new();
        for (oid, extension) in [(2950, UUID_EXTENSION), (3802, JSON_EXTENSION)] {
            let column = desc(oid, -1);
            let field = field(&column, ColumnKind::of(&column), &names);
            assert_eq!(field.metadata()[EXTENSION_NAME], extension);
            assert_eq!(ColumnKind::of(&column).format_code(), 1);
        }
        let numeric = desc(NUMERIC_OID, -1);
        let field = field(&numeric, ColumnKind::of(&numeric), &names);
        assert_eq!(field.metadata()[EXTENSION_NAME], DECIMAL_EXTENSION);
        assert_eq!(field.metadata()[SOURCE_TYPE], "numeric");
    }
}
