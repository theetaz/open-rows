// SPDX-License-Identifier: Apache-2.0
//! Deterministic `PostgreSQL` binary `DataRow` frames for benchmarks and tests.
//!
//! [`select_sql`] produces the same values server-side, so an integration test
//! can prove that these frames decode identically to real server output.
use crate::types::ColumnDesc;
use bytes::{BufMut, BytesMut};
use std::fmt::Write;

/// Fixed seed shared by the generator and its SQL twin.
pub const SEED: u64 = 42;

/// Column types the fixture can generate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureType {
    /// `int2`
    Int2,
    /// `int4`
    Int4,
    /// `int8`
    Int8,
    /// `float4`
    Float4,
    /// `float8`
    Float8,
    /// `bool`
    Bool,
    /// `text` with ASCII and two-byte UTF-8 characters
    Text,
    /// `bytea`, 4–16 bytes
    Bytea,
    /// `timestamp`
    Timestamp,
    /// `timestamptz`
    TimestampTz,
    /// `date`
    Date,
    /// `uuid`
    Uuid,
    /// `numeric(18, 4)` → `Decimal128`
    Decimal,
    /// unconstrained `numeric` → exact text
    Numeric,
    /// `jsonb`
    Jsonb,
}

impl FixtureType {
    /// Every fixture type, in declaration order.
    pub const ALL: [Self; 15] = [
        Self::Int2,
        Self::Int4,
        Self::Int8,
        Self::Float4,
        Self::Float8,
        Self::Bool,
        Self::Text,
        Self::Bytea,
        Self::Timestamp,
        Self::TimestampTz,
        Self::Date,
        Self::Uuid,
        Self::Decimal,
        Self::Numeric,
        Self::Jsonb,
    ];

    fn sql_type(self) -> &'static str {
        match self {
            Self::Int2 => "int2",
            Self::Int4 => "int4",
            Self::Int8 => "int8",
            Self::Float4 => "float4",
            Self::Float8 => "float8",
            Self::Bool => "bool",
            Self::Text => "text",
            Self::Bytea => "bytea",
            Self::Timestamp => "timestamp",
            Self::TimestampTz => "timestamptz",
            Self::Date => "date",
            Self::Uuid => "uuid",
            Self::Decimal => "numeric(18,4)",
            Self::Numeric => "numeric",
            Self::Jsonb => "jsonb",
        }
    }

    fn desc(self, column: usize) -> ColumnDesc {
        let (type_oid, type_modifier) = match self {
            Self::Int2 => (21, -1),
            Self::Int4 => (23, -1),
            Self::Int8 => (20, -1),
            Self::Float4 => (700, -1),
            Self::Float8 => (701, -1),
            Self::Bool => (16, -1),
            Self::Text => (25, -1),
            Self::Bytea => (17, -1),
            Self::Timestamp => (1114, -1),
            Self::TimestampTz => (1184, -1),
            Self::Date => (1082, -1),
            Self::Uuid => (2950, -1),
            Self::Decimal => (1700, ((18 << 16) | 4) + 4),
            Self::Numeric => (1700, -1),
            Self::Jsonb => (3802, -1),
        };
        ColumnDesc {
            name: format!("c{column}"),
            type_oid,
            type_modifier,
        }
    }

    /// SQL producing the value [`FixtureType::encode`] writes for base `n`.
    fn sql(self, n: &str) -> String {
        match self {
            Self::Int2 => format!("({n} % 32768) - 16384"),
            Self::Int4 => format!("{n} - 500000"),
            Self::Int8 => format!("{n} * 1000000007 - 3"),
            Self::Float4 => format!("{n} / 8.0"),
            Self::Float8 => format!("{n} / 1024.0"),
            Self::Bool => format!("{n} % 2 = 0"),
            Self::Text => format!("'v' || {n} || '-' || repeat('é', ({n} % 8)::int)"),
            Self::Bytea => {
                format!("decode(repeat(lpad(to_hex({n}), 8, '0'), ({n} % 4 + 1)::int), 'hex')")
            }
            Self::Timestamp => {
                format!("timestamp '2000-01-01' + ({n} * 1000003) * interval '1 microsecond'")
            }
            Self::TimestampTz => format!(
                "timestamptz '2000-01-01 00:00:00+00' + ({n} * 1000003) * interval '1 microsecond'"
            ),
            Self::Date => format!("date '2000-01-01' + (({n} % 20000) - 10000)::int"),
            Self::Uuid => format!("repeat(lpad(to_hex({n}), 8, '0'), 4)::uuid"),
            Self::Decimal | Self::Numeric => format!("({n} * 10007 - 5000000000) * 0.0001"),
            Self::Jsonb => format!("jsonb_build_object('n', {n})"),
        }
    }

    fn encode(self, n: u64, out: &mut BytesMut) {
        let signed = i64::try_from(n).unwrap_or_default();
        match self {
            Self::Int2 => put_cell(
                out,
                &(i16::try_from(n % 32_768).unwrap_or_default() - 16_384).to_be_bytes(),
            ),
            Self::Int4 => put_cell(
                out,
                &(i32::try_from(signed).unwrap_or_default() - 500_000).to_be_bytes(),
            ),
            Self::Int8 => put_cell(out, &(signed * 1_000_000_007 - 3).to_be_bytes()),
            Self::Float4 => put_cell(out, &(exact_f32(n) / 8.0).to_be_bytes()),
            Self::Float8 => put_cell(
                out,
                &(f64::from(u32::try_from(n).unwrap_or_default()) / 1024.0).to_be_bytes(),
            ),
            Self::Bool => put_cell(out, &[u8::from(n.is_multiple_of(2))]),
            Self::Text => put_cell(out, text(n).as_bytes()),
            Self::Bytea => put_cell(out, &bytea(n)),
            Self::Timestamp | Self::TimestampTz => {
                put_cell(out, &(signed * 1_000_003).to_be_bytes());
            }
            Self::Date => put_cell(
                out,
                &(i32::try_from(n % 20_000).unwrap_or_default() - 10_000).to_be_bytes(),
            ),
            Self::Uuid => put_cell(
                out,
                &u32::try_from(n).unwrap_or_default().to_be_bytes().repeat(4),
            ),
            Self::Decimal | Self::Numeric => {
                put_cell(out, &numeric(signed * 10_007 - 5_000_000_000));
            }
            Self::Jsonb => put_cell(out, format!("\x01{{\"n\": {n}}}").as_bytes()),
        }
    }
}

/// Pre-encoded result: column descriptions plus framed `DataRow` messages.
pub struct Fixture {
    /// `RowDescription` equivalent.
    pub columns: Vec<ColumnDesc>,
    /// Concatenated `DataRow` messages (tag, length, count, cells).
    pub frames: BytesMut,
    /// Number of rows encoded.
    pub rows: usize,
}

pub(crate) fn base(row: usize, column: usize) -> u64 {
    (row as u64 * 1_664_525 + column as u64 * 1_013_904_223 + SEED) % 1_000_000
}

pub(crate) fn is_null(row: usize, column: usize) -> bool {
    (row + column).is_multiple_of(7)
}

/// Encode `rows` rows; column `c` has type `types[c]` and is NULL when (row + c) % 7 = 0.
#[must_use]
pub fn generate(types: &[FixtureType], rows: usize) -> Fixture {
    let mut frames = BytesMut::new();
    for row in 0..rows {
        let start = frames.len();
        frames.put_u8(b'D');
        frames.put_i32(0);
        frames.put_u16(u16::try_from(types.len()).unwrap_or(u16::MAX));
        for (column, kind) in types.iter().enumerate() {
            if is_null(row, column) {
                frames.put_i32(-1);
            } else {
                kind.encode(base(row, column), &mut frames);
            }
        }
        let length = i32::try_from(frames.len() - start - 1).unwrap_or(i32::MAX);
        frames[start + 1..start + 5].copy_from_slice(&length.to_be_bytes());
    }
    let columns = types.iter().enumerate().map(|(c, t)| t.desc(c)).collect();
    Fixture {
        columns,
        frames,
        rows,
    }
}

/// A `SELECT` that makes the server produce exactly the rows of [`generate`].
#[must_use]
pub fn select_sql(types: &[FixtureType], rows: usize) -> String {
    let mut sql = String::from("SELECT ");
    for (column, kind) in types.iter().enumerate() {
        let n = format!("((g * 1664525 + {column}::int8 * 1013904223 + {SEED}) % 1000000)");
        let separator = if column == 0 { "" } else { ", " };
        let _ = write!(
            sql,
            "{separator}CAST(CASE WHEN (g + {column}) % 7 = 0 THEN NULL ELSE {} END AS {}) AS c{column}",
            kind.sql(&n),
            kind.sql_type()
        );
    }
    let _ = write!(
        sql,
        " FROM generate_series(0::int8, {}) AS g",
        rows.saturating_sub(1)
    );
    sql
}

#[expect(
    clippy::cast_precision_loss,
    reason = "fixture bases are < 2^20, exact in f32"
)]
fn exact_f32(n: u64) -> f32 {
    n as f32
}

fn put_cell(out: &mut BytesMut, value: &[u8]) {
    out.put_i32(i32::try_from(value.len()).unwrap_or(i32::MAX));
    out.put_slice(value);
}

pub(crate) fn text(n: u64) -> String {
    let repeats = usize::try_from(n % 8).unwrap_or_default();
    format!("v{n}-{}", "é".repeat(repeats))
}

fn bytea(n: u64) -> Vec<u8> {
    let repeats = usize::try_from(n % 4 + 1).unwrap_or(1);
    u32::try_from(n)
        .unwrap_or_default()
        .to_be_bytes()
        .repeat(repeats)
}

/// Binary numeric with display scale 4, normalised like `PostgreSQL`.
fn numeric(unscaled: i64) -> Vec<u8> {
    let magnitude = unscaled.unsigned_abs();
    let (mut integer, fraction) = (magnitude / 10_000, magnitude % 10_000);
    let mut groups = Vec::new();
    while integer > 0 {
        groups.insert(0, u16::try_from(integer % 10_000).unwrap_or_default());
        integer /= 10_000;
    }
    let weight = i16::try_from(groups.len()).unwrap_or(i16::MAX) - 1;
    groups.push(u16::try_from(fraction).unwrap_or_default());
    while groups.last() == Some(&0) {
        groups.pop();
    }
    let sign: u16 = if unscaled < 0 { 0x4000 } else { 0 };
    let weight = if groups.is_empty() { 0 } else { weight };
    let mut out = Vec::with_capacity(8 + 2 * groups.len());
    for field in [
        u16::try_from(groups.len()).unwrap_or_default(),
        weight.cast_unsigned(),
        sign,
        4,
    ] {
        out.extend_from_slice(&field.to_be_bytes());
    }
    for group in groups {
        out.extend_from_slice(&group.to_be_bytes());
    }
    out
}
