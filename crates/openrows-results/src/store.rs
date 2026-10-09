// SPDX-License-Identifier: Apache-2.0
use crate::limits::{Append, Limits};
use crate::slice::{project_slice, ranges};
use arrow_array::cast::AsArray;
use arrow_array::types::Int64Type;
use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_ipc::writer::StreamWriter;
use arrow_schema::{ArrowError, SchemaRef};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

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
    /// The cell holds numbers, dates or another type with no byte value.
    #[error("cell does not hold text or binary data")]
    NotBytes,
    /// Arrow rejected the schema or stream encoding.
    #[error(transparent)]
    Arrow(#[from] ArrowError),
}

struct ResultSet {
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
    rows: usize,
    bytes: usize,
    truncated: bool,
}

impl ResultSet {
    fn new(schema: SchemaRef) -> Self {
        Self {
            schema,
            batches: Vec::new(),
            rows: 0,
            bytes: 0,
            truncated: false,
        }
    }
}

/// The batches (shared, not copied) and row ranges one window encodes.
struct WindowPlan {
    schema: SchemaRef,
    indices: Vec<usize>,
    slices: Vec<(RecordBatch, Range<usize>)>,
}

#[derive(Default)]
struct Inner {
    results: HashMap<String, ResultSet>,
    bytes: usize,
}

impl Inner {
    fn remove(&mut self, id: &str) -> bool {
        let Some(old) = self.results.remove(id) else {
            return false;
        };
        self.bytes = self.bytes.saturating_sub(old.bytes);
        true
    }
}

/// Result ownership stays in the core; slices share the original Arrow buffers.
///
/// Thread-safe: session actors append while the UI reads windows. Results stay
/// in memory (spilling to disk is later work, docs/06 §3); [`Limits`] caps rows
/// per result and Arrow memory across all results.
#[derive(Default)]
pub struct ResultStore {
    inner: RwLock<Inner>,
    limits: Limits,
}

impl ResultStore {
    /// An empty store with explicit limits.
    #[must_use]
    pub fn with_limits(limits: Limits) -> Self {
        Self {
            inner: RwLock::default(),
            limits,
        }
    }

    /// Insert or replace a complete result. An empty result uses a zero-row batch.
    ///
    /// # Errors
    /// Returns `InvalidBatches` for missing or inconsistent batches.
    pub fn insert(&self, id: String, batches: Vec<RecordBatch>) -> Result<(), WindowError> {
        let schema = batches.first().ok_or(WindowError::InvalidBatches)?.schema();
        if batches.iter().any(|batch| batch.schema() != schema) {
            return Err(WindowError::InvalidBatches);
        }
        let rows = batches.iter().try_fold(0usize, |rows, batch| {
            rows.checked_add(batch.num_rows())
                .ok_or(WindowError::InvalidBatches)
        })?;
        let bytes = batches.iter().map(RecordBatch::get_array_memory_size).sum();
        let mut inner = self.write();
        inner.remove(&id);
        inner.bytes = inner.bytes.saturating_add(bytes);
        inner.results.insert(
            id,
            ResultSet {
                schema,
                batches,
                rows,
                bytes,
                truncated: false,
            },
        );
        Ok(())
    }

    /// Start a streamed result with no rows, replacing any result with the same id.
    pub fn begin(&self, id: &str, schema: SchemaRef) {
        let mut inner = self.write();
        inner.remove(id);
        inner.results.insert(id.to_owned(), ResultSet::new(schema));
    }

    /// Append a batch to a streamed result.
    ///
    /// Returns [`Append::Full`] when the result hit the row cap (the batch is
    /// cut to fit), the global memory budget is exhausted (the batch is
    /// dropped), the id is unknown (released) or the schema does not match.
    pub fn append(&self, id: &str, batch: RecordBatch) -> Append {
        let mut guard = self.write();
        let inner = &mut *guard;
        let Some(result) = inner.results.get_mut(id) else {
            return Append::Full;
        };
        if batch.schema() != result.schema {
            result.truncated = true;
            return Append::Full;
        }
        let room = self.limits.max_rows.saturating_sub(result.rows);
        let batch = if batch.num_rows() > room {
            result.truncated = true;
            batch.slice(0, room)
        } else {
            batch
        };
        let bytes = batch.get_array_memory_size();
        if inner.bytes.saturating_add(bytes) > self.limits.max_bytes {
            result.truncated = true;
            return Append::Full;
        }
        inner.bytes += bytes;
        result.bytes += bytes;
        result.rows += batch.num_rows();
        if batch.num_rows() > 0 {
            result.batches.push(batch);
        }
        if result.truncated {
            Append::Full
        } else {
            Append::Continue
        }
    }

    /// Drop a result and its memory; returns false when the id is unknown.
    pub fn remove(&self, id: &str) -> bool {
        self.write().remove(id)
    }

    /// Rows stored so far and whether a limit cut the result short.
    #[must_use]
    pub fn rows(&self, id: &str) -> Option<(usize, bool)> {
        self.read()
            .results
            .get(id)
            .map(|result| (result.rows, result.truncated))
    }

    /// The first row's first value as an integer, e.g. a `count(*)`; `None`
    /// when the result is unknown, empty, NULL there or not a 64-bit integer.
    #[must_use]
    pub fn first_integer(&self, id: &str) -> Option<i64> {
        let inner = self.read();
        let batch = inner.results.get(id)?.batches.first()?;
        let column = batch.columns().first()?.as_primitive_opt::<Int64Type>()?;
        column.is_valid(0).then(|| column.value(0))
    }

    /// Arrow memory held by all results.
    #[must_use]
    pub fn memory(&self) -> usize {
        self.read().bytes
    }

    /// Slice rows and columns and encode an uncompressed Arrow IPC stream.
    ///
    /// # Errors
    /// Returns typed identifier, bounds or Arrow encoding errors.
    pub fn window(&self, id: &str, window: Window) -> Result<Vec<u8>, WindowError> {
        let plan = self.slices(id, window)?;
        let mut bytes = Vec::new();
        let mut writer = StreamWriter::try_new(&mut bytes, &plan.schema)?;
        for (batch, rows) in &plan.slices {
            writer.write(&project_slice(
                batch,
                plan.schema.clone(),
                &plan.indices,
                rows.clone(),
            )?)?;
        }
        writer.finish()?;
        Ok(bytes)
    }

    /// Projected schema, column indices and the batch row ranges of a window,
    /// collected under the read lock; encoding happens after it is released.
    fn slices(&self, id: &str, window: Window) -> Result<WindowPlan, WindowError> {
        let inner = self.read();
        let result = inner
            .results
            .get(id)
            .ok_or_else(|| WindowError::UnknownResult(id.to_owned()))?;
        let (rows, cols) = ranges(result.rows, result.schema.fields().len(), window)?;
        let indices: Vec<_> = cols.collect();
        let schema = Arc::new(result.schema.project(&indices)?);
        let mut slices = Vec::new();
        let mut offset = 0;
        for batch in &result.batches {
            let start = rows.start.saturating_sub(offset).min(batch.num_rows());
            let end = rows.end.saturating_sub(offset).min(batch.num_rows());
            if start < end {
                slices.push((batch.clone(), start..end));
            }
            offset += batch.num_rows();
            if offset >= rows.end {
                break;
            }
        }
        Ok(WindowPlan {
            schema,
            indices,
            slices,
        })
    }

    /// The column array holding one cell, and the cell's index inside that
    /// array's batch. Shares the buffers; the lock is released on return.
    pub(crate) fn cell_column(
        &self,
        id: &str,
        row: usize,
        column: usize,
    ) -> Result<(ArrayRef, usize), WindowError> {
        let inner = self.read();
        let result = inner
            .results
            .get(id)
            .ok_or_else(|| WindowError::UnknownResult(id.to_owned()))?;
        if row >= result.rows || column >= result.schema.fields().len() {
            return Err(WindowError::OutOfRange);
        }
        let mut index = row;
        for batch in &result.batches {
            if index < batch.num_rows() {
                return Ok((Arc::clone(batch.column(column)), index));
            }
            index -= batch.num_rows();
        }
        Err(WindowError::OutOfRange)
    }

    fn read(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }
}
