// SPDX-License-Identifier: Apache-2.0
//! Exact values of single cells for the value inspector.
//!
//! Grid tiles carry display text only (a binary cell shows its size). The
//! inspector asks for the exact value of one cell, so a large text or binary
//! value is read on demand and never has to live in the grid.
use arrow_array::Array;
use arrow_array::cast::AsArray;

use crate::store::{ResultStore, WindowError};

/// What a cell holds, as the value inspector tells values apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    /// SQL NULL.
    Null,
    /// Character data, exact as UTF-8 bytes.
    Text,
    /// Binary data, including fixed-width binary such as UUIDs.
    Binary,
    /// Numbers, dates and other types: the display text is the value.
    Other,
}

/// The kind and byte length of one cell (zero unless text or binary).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellInfo {
    /// What the cell holds.
    pub kind: CellKind,
    /// Bytes of the exact value (UTF-8 for text).
    pub byte_len: usize,
}

impl ResultStore {
    /// Kind and byte length of one cell, without copying its value.
    ///
    /// # Errors
    /// `UnknownResult` for an unknown id, `OutOfRange` for a cell beyond the result.
    pub fn cell_info(&self, id: &str, row: usize, column: usize) -> Result<CellInfo, WindowError> {
        let (array, index) = self.cell_column(id, row, column)?;
        Ok(match value_bytes(array.as_ref(), index) {
            Value::Null => CellInfo {
                kind: CellKind::Null,
                byte_len: 0,
            },
            Value::Bytes(kind, bytes) => CellInfo {
                kind,
                byte_len: bytes.len(),
            },
            Value::Other => CellInfo {
                kind: CellKind::Other,
                byte_len: 0,
            },
        })
    }

    /// The exact bytes of a text (UTF-8) or binary cell.
    ///
    /// # Errors
    /// `NotBytes` for NULL and other types, plus the errors of [`ResultStore::cell_info`].
    pub fn cell_bytes(&self, id: &str, row: usize, column: usize) -> Result<Vec<u8>, WindowError> {
        let (array, index) = self.cell_column(id, row, column)?;
        match value_bytes(array.as_ref(), index) {
            Value::Bytes(_, bytes) => Ok(bytes.to_vec()),
            Value::Null | Value::Other => Err(WindowError::NotBytes),
        }
    }
}

/// One cell's value, borrowed from the column array.
enum Value<'a> {
    Null,
    Bytes(CellKind, &'a [u8]),
    Other,
}

fn value_bytes(array: &dyn Array, index: usize) -> Value<'_> {
    if array.is_null(index) {
        return Value::Null;
    }
    if let Some(text) = array.as_string_opt::<i32>() {
        return Value::Bytes(CellKind::Text, text.value(index).as_bytes());
    }
    if let Some(text) = array.as_string_opt::<i64>() {
        return Value::Bytes(CellKind::Text, text.value(index).as_bytes());
    }
    if let Some(binary) = array.as_binary_opt::<i32>() {
        return Value::Bytes(CellKind::Binary, binary.value(index));
    }
    if let Some(binary) = array.as_binary_opt::<i64>() {
        return Value::Bytes(CellKind::Binary, binary.value(index));
    }
    if let Some(fixed) = array.as_fixed_size_binary_opt() {
        return Value::Bytes(CellKind::Binary, fixed.value(index));
    }
    Value::Other
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow_array::{BinaryArray, Int64Array, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};

    use super::{CellInfo, CellKind};
    use crate::store::{ResultStore, WindowError};

    fn store() -> Result<ResultStore, Box<dyn std::error::Error>> {
        let schema = Arc::new(Schema::new(vec![
            Field::new("text", DataType::Utf8, true),
            Field::new("blob", DataType::Binary, true),
            Field::new("number", DataType::Int64, true),
        ]));
        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(StringArray::from(vec![Some("héllo"), None])),
                Arc::new(BinaryArray::from(vec![Some(&[1_u8, 2, 3][..]), None])),
                Arc::new(Int64Array::from(vec![Some(7), Some(8)])),
            ],
        )?;
        let store = ResultStore::default();
        store.insert("r".to_owned(), vec![batch])?;
        Ok(store)
    }

    #[test]
    fn kinds_and_byte_lengths_describe_each_cell() -> Result<(), Box<dyn std::error::Error>> {
        let store = store()?;
        assert_eq!(
            store.cell_info("r", 0, 0)?,
            CellInfo {
                kind: CellKind::Text,
                byte_len: "héllo".len(),
            }
        );
        assert_eq!(store.cell_info("r", 0, 1)?.kind, CellKind::Binary);
        assert_eq!(store.cell_info("r", 0, 1)?.byte_len, 3);
        assert_eq!(store.cell_info("r", 1, 0)?.kind, CellKind::Null);
        assert_eq!(store.cell_info("r", 0, 2)?.kind, CellKind::Other);
        Ok(())
    }

    #[test]
    fn exact_bytes_come_back_for_text_and_binary_only() -> Result<(), Box<dyn std::error::Error>> {
        let store = store()?;
        assert_eq!(store.cell_bytes("r", 0, 0)?, "héllo".as_bytes());
        assert_eq!(store.cell_bytes("r", 0, 1)?, vec![1, 2, 3]);
        assert!(matches!(
            store.cell_bytes("r", 1, 0),
            Err(WindowError::NotBytes)
        ));
        assert!(matches!(
            store.cell_bytes("r", 0, 2),
            Err(WindowError::NotBytes)
        ));
        Ok(())
    }

    #[test]
    fn unknown_results_and_cells_beyond_the_result_are_refused()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = store()?;
        assert!(matches!(
            store.cell_info("missing", 0, 0),
            Err(WindowError::UnknownResult(_))
        ));
        assert!(matches!(
            store.cell_info("r", 2, 0),
            Err(WindowError::OutOfRange)
        ));
        assert!(matches!(
            store.cell_info("r", 0, 3),
            Err(WindowError::OutOfRange)
        ));
        Ok(())
    }
}
