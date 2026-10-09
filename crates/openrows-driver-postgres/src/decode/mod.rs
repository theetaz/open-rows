// SPDX-License-Identifier: Apache-2.0
//! `DataRow` bodies → Arrow `RecordBatch`es, flushed per [`BatchPolicy`].
mod array;
mod column;
mod fallback;
mod network;
mod numeric;
mod scalar;
mod temporal;
mod text;

use crate::types::{self, ColumnDesc, ColumnKind};
use arrow_array::{RecordBatch, RecordBatchOptions};
use arrow_schema::{Schema, SchemaRef};
use column::ColumnDecoder;
use openrows_driver_api::{BatchPolicy, DriverError};
use std::{collections::HashMap, sync::Arc};

/// Streaming decoder for one result set.
///
/// A timestamp, date, time, interval, `numeric(p, s)` or array value with no
/// exact Arrow form (±infinity, beyond Arrow's µs range, `NaN`, `24:00:00`, a
/// multidimensional array) switches that column to tagged text for the rest
/// of the result: [`ResultDecoder::schema`] changes and later batches carry
/// the new schema. After [`ResultDecoder::decode_row`] fails the decoder is
/// poisoned: drop it.
pub struct ResultDecoder {
    schema: SchemaRef,
    columns: Vec<ColumnDecoder>,
    formats: Vec<i16>,
    policy: BatchPolicy,
    limit: usize,
    rows: usize,
    bytes: usize,
}

impl ResultDecoder {
    /// Build pre-sized column builders; `type_names` names non-built-in OIDs.
    #[must_use]
    pub fn new(
        columns: &[ColumnDesc],
        type_names: &HashMap<u32, String>,
        policy: BatchPolicy,
    ) -> Self {
        let kinds: Vec<_> = columns.iter().map(ColumnKind::of).collect();
        let fields: Vec<_> = columns
            .iter()
            .zip(&kinds)
            .map(|(desc, &kind)| types::field(desc, kind, type_names))
            .collect();
        let limit = policy.row_limit(columns.len(), true);
        Self {
            schema: Arc::new(Schema::new(fields)),
            columns: kinds
                .iter()
                .map(|&k| ColumnDecoder::new(k, limit))
                .collect(),
            formats: kinds.iter().map(|k| k.format_code()).collect(),
            policy,
            limit,
            rows: 0,
            bytes: 0,
        }
    }

    /// Current Arrow schema with per-field source-type metadata.
    #[must_use]
    pub fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }

    /// Per-column result format codes for `Bind` (1 = binary, 0 = text).
    #[must_use]
    pub fn result_formats(&self) -> &[i16] {
        &self.formats
    }

    /// Decode one `DataRow` body (the bytes after its column count).
    ///
    /// # Errors
    /// `Decode` for malformed cells, `Arrow` on batch assembly.
    pub fn decode_row(&mut self, mut row: &[u8]) -> Result<Option<RecordBatch>, DriverError> {
        self.bytes += row.len();
        for index in 0..self.columns.len() {
            let cell = next_cell(&mut row).map_err(|reason| decode_error(index, reason))?;
            if let Err(reason) = self.columns[index].append(cell) {
                self.fall_back(index, cell, reason)?;
            }
        }
        if !row.is_empty() {
            return Err(DriverError::Protocol(
                "DataRow has extra columns".to_owned(),
            ));
        }
        self.rows += 1;
        if self.policy.should_flush(self.rows, self.bytes, self.limit) {
            return self.flush().map(Some);
        }
        Ok(None)
    }

    /// Flush any buffered rows as a final batch.
    ///
    /// # Errors
    /// `Arrow` if the batch cannot be assembled.
    pub fn finish(&mut self) -> Result<Option<RecordBatch>, DriverError> {
        if self.rows == 0 {
            return Ok(None);
        }
        self.flush().map(Some)
    }

    /// Retry a failed cell as tagged text and retag the column's field.
    fn fall_back(
        &mut self,
        index: usize,
        cell: Option<&[u8]>,
        reason: scalar::CellError,
    ) -> Result<(), DriverError> {
        let Some(value) = cell.filter(|_| matches!(reason, scalar::CellError::Unrepresentable(_)))
        else {
            return Err(decode_error(index, reason));
        };
        self.columns[index]
            .fall_back(value, reason)
            .map_err(|reason| decode_error(index, reason))?;
        let mut fields: Vec<_> = self.schema.fields().iter().cloned().collect();
        fields[index] = Arc::new(types::tagged_text(&fields[index]));
        self.schema = Arc::new(Schema::new_with_metadata(
            fields,
            self.schema.metadata().clone(),
        ));
        Ok(())
    }

    fn flush(&mut self) -> Result<RecordBatch, DriverError> {
        let next = self.policy.row_limit(self.columns.len(), false);
        let arrays = self.columns.iter_mut().map(|c| c.finish(next)).collect();
        let options = RecordBatchOptions::new().with_row_count(Some(self.rows));
        let batch = RecordBatch::try_new_with_options(self.schema.clone(), arrays, &options)?;
        (self.limit, self.rows, self.bytes) = (next, 0, 0);
        Ok(batch)
    }
}

/// Wire cell: `i32` length (−1 = NULL) followed by that many bytes.
#[inline]
pub(crate) fn next_cell<'a>(row: &mut &'a [u8]) -> Result<Option<&'a [u8]>, scalar::CellError> {
    let (length, rest) = row
        .split_first_chunk::<4>()
        .ok_or(scalar::CellError::Truncated)?;
    let length = i32::from_be_bytes(*length);
    if length == -1 {
        *row = rest;
        return Ok(None);
    }
    let length = usize::try_from(length).map_err(|_| scalar::CellError::Truncated)?;
    let (value, rest) = rest
        .split_at_checked(length)
        .ok_or(scalar::CellError::Truncated)?;
    *row = rest;
    Ok(Some(value))
}

fn decode_error(column: usize, reason: scalar::CellError) -> DriverError {
    DriverError::Decode {
        column,
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests;
