// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` arrays: one-dimensional arrays with lower bound 1 decode into
//! Arrow `List<element>`; any other shape (or an element without an exact
//! Arrow value) is `PostgreSQL`'s array text, e.g. `[0:1]={1,2}`.
use super::column::Builder;
use super::next_cell;
use super::scalar::{CellError, array};
use super::text;
use crate::types::ColumnKind;
use arrow_array::builder::NullBufferBuilder;
use arrow_array::cast::AsArray;
use arrow_array::{Array, ArrayRef, ListArray};
use arrow_buffer::OffsetBuffer;
use arrow_schema::{DataType, FieldRef};
use openrows_driver_api::text::write_array_item;
use std::fmt::Write;
use std::sync::Arc;

/// One array dimension: length and lower bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Dimension {
    length: usize,
    lower: i32,
}

/// The dimensions of a wire array and its element cells.
struct Header<'a> {
    dimensions: Vec<Dimension>,
    elements: &'a [u8],
}

/// `ndim`, `has_null` flag, element OID, then `(length, lower bound)` per dimension.
fn header(bytes: &[u8]) -> Result<Header<'_>, CellError> {
    let (fixed, mut rest) = bytes.split_at_checked(12).ok_or(CellError::Truncated)?;
    let dimensions = i32::from_be_bytes(array(&fixed[..4])?);
    let count = usize::try_from(dimensions).map_err(|_| CellError::Truncated)?;
    let mut list = Vec::with_capacity(count.min(6));
    for _ in 0..count {
        let (pair, tail) = rest.split_at_checked(8).ok_or(CellError::Truncated)?;
        let length = i32::from_be_bytes(array(&pair[..4])?);
        list.push(Dimension {
            length: usize::try_from(length).map_err(|_| CellError::Truncated)?,
            lower: i32::from_be_bytes(array(&pair[4..])?),
        });
        rest = tail;
    }
    Ok(Header {
        dimensions: list,
        elements: rest,
    })
}

/// Arrow list column of one element kind, with its own element builder.
pub(super) struct ListValues {
    item: FieldRef,
    values: Builder,
    offsets: Vec<i32>,
    nulls: NullBufferBuilder,
}

impl ListValues {
    pub(super) fn new(kind: ColumnKind, rows: usize, bytes: usize) -> Option<Self> {
        let DataType::List(item) = kind.data_type() else {
            return None;
        };
        let mut offsets = Vec::with_capacity(rows + 1);
        offsets.push(0);
        Some(Self {
            item,
            values: Builder::new(kind.element()?, rows * 4, bytes),
            offsets,
            nulls: NullBufferBuilder::new(rows),
        })
    }

    pub(super) fn append_value(&mut self, bytes: &[u8]) -> Result<(), CellError> {
        let Header {
            dimensions,
            mut elements,
        } = header(bytes)?;
        let length = match dimensions.as_slice() {
            [] => 0,
            [Dimension { length, lower: 1 }] => *length,
            [_] => return Err(CellError::Unrepresentable("array lower bound other than 1")),
            _ => return Err(CellError::Unrepresentable("multidimensional array")),
        };
        for _ in 0..length {
            match next_cell(&mut elements)? {
                Some(value) => self.values.append_value(value)?,
                None => self.values.append_null(),
            }
        }
        let last = self.offsets.last().copied().unwrap_or_default();
        let added = i32::try_from(length).map_err(|_| CellError::Truncated)?;
        self.offsets
            .push(last.checked_add(added).ok_or(CellError::Truncated)?);
        self.nulls.append_non_null();
        Ok(())
    }

    pub(super) fn append_null(&mut self) {
        let last = self.offsets.last().copied().unwrap_or_default();
        self.offsets.push(last);
        self.nulls.append_null();
    }

    pub(super) fn data_bytes(&self) -> usize {
        self.values.data_bytes()
    }

    pub(super) fn finish(&mut self) -> ArrayRef {
        let offsets = std::mem::replace(&mut self.offsets, vec![0]);
        let values = self.values.finish();
        // Offsets only grow and never pass the element count, so they are valid.
        Arc::new(ListArray::new(
            Arc::clone(&self.item),
            OffsetBuffer::new(offsets.into()),
            values,
            self.nulls.finish(),
        ))
    }
}

/// `PostgreSQL` array text of a binary wire array of `element` values.
pub(crate) fn write_text(
    element: ColumnKind,
    bytes: &[u8],
    out: &mut impl Write,
) -> Result<(), CellError> {
    let Header {
        dimensions,
        mut elements,
    } = header(bytes)?;
    let format = |result: std::fmt::Result| result.map_err(|_| CellError::Format);
    if dimensions.iter().any(|dimension| dimension.lower != 1) {
        for dimension in &dimensions {
            let upper =
                i64::from(dimension.lower) + i64::try_from(dimension.length).unwrap_or(0) - 1;
            format(write!(out, "[{}:{upper}]", dimension.lower))?;
        }
        format(out.write_char('='))?;
    }
    if dimensions.is_empty() {
        return format(out.write_str("{}"));
    }
    let mut scratch = String::new();
    write_level(
        &dimensions,
        &mut |out: &mut dyn Write| match next_cell(&mut elements)? {
            None => format(out.write_str("NULL")),
            Some(value) => {
                scratch.clear();
                text::write_wire(element, value, &mut scratch)?;
                format(write_array_item(&scratch, out))
            }
        },
        out,
    )
}

/// `{…}` for the first dimension, recursing into the next ones.
fn write_level(
    dimensions: &[Dimension],
    item: &mut dyn FnMut(&mut dyn Write) -> Result<(), CellError>,
    out: &mut dyn Write,
) -> Result<(), CellError> {
    let Some((first, rest)) = dimensions.split_first() else {
        return item(out);
    };
    out.write_char('{').map_err(|_| CellError::Format)?;
    for index in 0..first.length {
        if index > 0 {
            out.write_char(',').map_err(|_| CellError::Format)?;
        }
        write_level(rest, item, out)?;
    }
    out.write_char('}').map_err(|_| CellError::Format)
}

/// Array text of the non-null list at `index` of a decoded `List` column.
pub(crate) fn write_arrow_text(
    element: ColumnKind,
    values: &dyn Array,
    index: usize,
    out: &mut impl Write,
) -> Result<(), CellError> {
    let list = values.as_list_opt::<i32>().ok_or(CellError::Format)?;
    let items = list.value(index);
    let mut scratch = String::new();
    let format = |result: std::fmt::Result| result.map_err(|_| CellError::Format);
    format(out.write_char('{'))?;
    for item in 0..items.len() {
        if item > 0 {
            format(out.write_char(','))?;
        }
        if items.is_null(item) {
            format(out.write_str("NULL"))?;
            continue;
        }
        scratch.clear();
        text::write_arrow(element, items.as_ref(), item, &mut scratch)?;
        format(write_array_item(&scratch, out))?;
    }
    format(out.write_char('}'))
}

#[cfg(test)]
#[path = "array_tests.rs"]
mod tests;
