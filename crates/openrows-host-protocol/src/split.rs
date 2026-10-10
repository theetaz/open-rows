// SPDX-License-Identifier: Apache-2.0
//! Prepare one compacted part at a time, retaining no list of serialized batches.
use crate::{ArrowEncoder, BATCH_LIMIT, LARGE_LIMIT, ProtocolError, batch_memory};
use arrow_array::RecordBatch;

/// Lazy multi-row split; only the source and current part overlap.
pub struct BatchParts {
    source: RecordBatch,
    offset: usize,
}
/// Split multi-row batches into normal windows; single rows require exclusive admission.
/// # Errors
/// An individual source row already exceeds the 64 MiB retained-memory ceiling.
pub fn split_batch(batch: &RecordBatch) -> Result<BatchParts, ProtocolError> {
    if batch.num_rows() <= 1 && batch_memory(batch) > LARGE_LIMIT {
        return Err(ProtocolError::Limit);
    }
    Ok(BatchParts {
        source: batch.clone(),
        offset: 0,
    })
}
impl BatchParts {
    fn prepare(&self, rows: usize) -> Result<RecordBatch, ProtocolError> {
        let part = self.source.slice(self.offset, rows);
        if rows < self.source.num_rows() {
            crate::arrow::compact(&part)
        } else {
            Ok(part)
        }
    }
    fn next_part(&mut self) -> Result<RecordBatch, ProtocolError> {
        let mut rows = self.source.num_rows() - self.offset;
        loop {
            let prepared = self.prepare(rows).and_then(|part| {
                let segment = ArrowEncoder::new(part.schema())?.batch(&part)?;
                let fits = rows == 1
                    || (segment.bytes.len() <= BATCH_LIMIT && segment.decoded <= BATCH_LIMIT);
                fits.then_some(part).ok_or(ProtocolError::Limit)
            });
            match prepared {
                Ok(part) => {
                    self.offset += rows;
                    return Ok(part);
                }
                Err(ProtocolError::Limit) if rows > 1 => rows /= 2,
                Err(error) => return Err(error),
            }
        }
    }
}
impl Iterator for BatchParts {
    type Item = Result<RecordBatch, ProtocolError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset == self.source.num_rows() {
            return None;
        }
        let part = self.next_part();
        if part.is_err() {
            self.offset = self.source.num_rows();
        }
        Some(part)
    }
}
