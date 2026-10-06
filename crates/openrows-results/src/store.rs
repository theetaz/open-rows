// SPDX-License-Identifier: Apache-2.0
use arrow_array::{RecordBatch, RecordBatchOptions};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{ArrowError, SchemaRef};
use std::{collections::HashMap, sync::Arc};

/// A viewport request; counts are clamped, starts beyond the end are errors.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    /// First row, inclusive (the end itself is a valid empty window).
    pub row_start: usize,
    /// Maximum number of rows.
    pub row_count: usize,
    /// First column, inclusive.
    pub col_start: usize,
    /// Maximum number of columns.
    pub col_count: usize,
}

/// Typed failures at the result boundary.
#[derive(Debug, thiserror::Error)]
pub enum WindowError {
    /// The identifier has no stored result.
    #[error("unknown result: {0}")]
    UnknownResult(String),
    /// A start offset exceeds the corresponding dimension.
    #[error("window start exceeds result bounds")]
    OutOfRange,
    /// Insertion requires at least one batch, all with identical schemas.
    #[error("result requires batches with a shared schema")]
    InvalidBatches,
    /// Arrow rejected the schema or stream encoding.
    #[error(transparent)]
    Arrow(#[from] ArrowError),
}

struct ResultSet {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    rows: usize,
}

/// Result ownership stays in the core; slices share the original Arrow buffers.
#[derive(Default)]
pub struct ResultStore {
    results: HashMap<String, ResultSet>,
}

impl ResultStore {
    /// Insert or replace a result. An empty result uses a zero-row batch.
    ///
    /// # Errors
    /// Returns `InvalidBatches` for missing or inconsistent batches.
    pub fn insert(&mut self, id: String, batches: Vec<RecordBatch>) -> Result<(), WindowError> {
        let schema = batches.first().ok_or(WindowError::InvalidBatches)?.schema();
        if batches.iter().any(|batch| batch.schema() != schema) {
            return Err(WindowError::InvalidBatches);
        }
        let rows = batches.iter().try_fold(0usize, |rows, batch| {
            rows.checked_add(batch.num_rows())
                .ok_or(WindowError::InvalidBatches)
        })?;
        self.results.insert(
            id,
            ResultSet {
                schema,
                batches,
                rows,
            },
        );
        Ok(())
    }

    /// Slice rows and columns and encode an uncompressed Arrow IPC stream.
    ///
    /// # Errors
    /// Returns typed identifier, bounds or Arrow encoding errors.
    pub fn window(&self, id: &str, window: Window) -> Result<Vec<u8>, WindowError> {
        let result = self
            .results
            .get(id)
            .ok_or_else(|| WindowError::UnknownResult(id.to_owned()))?;
        let (rows, cols) = result.ranges(window)?;
        let indices: Vec<_> = cols.collect();
        let schema = Arc::new(result.schema.project(&indices)?);
        let mut bytes = Vec::new();
        let mut writer = StreamWriter::try_new(&mut bytes, &schema)?;
        let mut offset = 0;
        for batch in &result.batches {
            let start = rows.start.saturating_sub(offset).min(batch.num_rows());
            let end = rows.end.saturating_sub(offset).min(batch.num_rows());
            if start < end {
                writer.write(&project_slice(batch, schema.clone(), &indices, start..end)?)?;
            }
            offset += batch.num_rows();
            if offset >= rows.end {
                break;
            }
        }
        writer.finish()?;
        Ok(bytes)
    }
}

impl ResultSet {
    fn ranges(
        &self,
        window: Window,
    ) -> Result<(std::ops::Range<usize>, std::ops::Range<usize>), WindowError> {
        if window.row_start > self.rows || window.col_start > self.schema.fields().len() {
            return Err(WindowError::OutOfRange);
        }
        Ok((
            window.row_start
                ..window
                    .row_start
                    .saturating_add(window.row_count)
                    .min(self.rows),
            window.col_start
                ..window
                    .col_start
                    .saturating_add(window.col_count)
                    .min(self.schema.fields().len()),
        ))
    }
}

fn project_slice(
    batch: &RecordBatch,
    schema: SchemaRef,
    indices: &[usize],
    rows: std::ops::Range<usize>,
) -> Result<RecordBatch, ArrowError> {
    let columns = indices
        .iter()
        .map(|&i| batch.column(i).slice(rows.start, rows.len()))
        .collect();
    let options = RecordBatchOptions::new().with_row_count(Some(rows.len()));
    RecordBatch::try_new_with_options(schema, columns, &options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_slice_shares_source_allocation() -> Result<(), Box<dyn std::error::Error>> {
        let source = crate::synthetic::batch(100, 20, 42)?;
        let tile = project_slice(
            &source,
            Arc::new(source.schema().project(&[0])?),
            &[0],
            13..30,
        )?;
        let source_data = source.column(0).to_data();
        let tile_data = tile.column(0).to_data();
        assert_eq!(
            tile_data.buffers()[0].as_ptr(),
            source_data.buffers()[0].as_ptr().wrapping_add(13 * 4)
        );
        Ok(())
    }
}
