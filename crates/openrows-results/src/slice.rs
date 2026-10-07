// SPDX-License-Identifier: Apache-2.0
//! Window bounds and zero-copy batch projection.
use crate::store::{Window, WindowError};
use arrow_array::{RecordBatch, RecordBatchOptions};
use arrow_schema::{ArrowError, SchemaRef};
use std::ops::Range;

/// Row and column ranges of `window` over a `rows` × `columns` result.
pub(crate) fn ranges(
    rows: usize,
    columns: usize,
    window: Window,
) -> Result<(Range<usize>, Range<usize>), WindowError> {
    if window.row_start > rows || window.col_start > columns {
        return Err(WindowError::OutOfRange);
    }
    let row_end = window.row_start.saturating_add(window.row_count).min(rows);
    let col_end = window
        .col_start
        .saturating_add(window.col_count)
        .min(columns);
    Ok((window.row_start..row_end, window.col_start..col_end))
}

/// Columns `indices` of `rows`, sharing the source buffers.
pub(crate) fn project_slice(
    batch: &RecordBatch,
    schema: SchemaRef,
    indices: &[usize],
    rows: Range<usize>,
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
    use std::sync::Arc;

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
