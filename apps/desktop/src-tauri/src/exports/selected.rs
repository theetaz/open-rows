// SPDX-License-Identifier: Apache-2.0
//! The selected rows of a stored result, read batch by batch as zero-copy
//! slices of the stored batches (one slice per run of adjacent rows).
use arrow_array::RecordBatch;
use std::collections::VecDeque;

/// Batches of `batches` narrowed to the rows at `rows` (positions in the
/// whole result), in stored order.
pub(super) struct Selected<I> {
    batches: I,
    /// Sorted, distinct positions still to come.
    rows: Vec<u32>,
    next: usize,
    /// Position of the next stored batch's first row.
    offset: u64,
    ready: VecDeque<RecordBatch>,
}

impl<I> Selected<I> {
    pub(super) fn new(batches: I, mut rows: Vec<u32>) -> Self {
        rows.sort_unstable();
        rows.dedup();
        Self {
            batches,
            rows,
            next: 0,
            offset: 0,
            ready: VecDeque::new(),
        }
    }

    /// Queue the slices of `batch` that hold selected rows.
    fn select(&mut self, batch: &RecordBatch) {
        let end = self.offset + batch.num_rows() as u64;
        while let Some(&first) = self.rows.get(self.next) {
            let first = u64::from(first);
            if first >= end {
                break;
            }
            let mut last = first;
            self.next += 1;
            while self
                .rows
                .get(self.next)
                .is_some_and(|&row| u64::from(row) == last + 1 && last + 1 < end)
            {
                last += 1;
                self.next += 1;
            }
            let start = usize::try_from(first - self.offset).unwrap_or(usize::MAX);
            let length = usize::try_from(last - first + 1).unwrap_or(0);
            if start < batch.num_rows() {
                self.ready.push_back(batch.slice(start, length));
            }
        }
        self.offset = end;
    }
}

impl<I, E> Iterator for Selected<I>
where
    I: Iterator<Item = Result<RecordBatch, E>>,
{
    type Item = Result<RecordBatch, E>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(batch) = self.ready.pop_front() {
                return Some(Ok(batch));
            }
            if self.next >= self.rows.len() {
                return None;
            }
            match self.batches.next()? {
                Ok(batch) => self.select(&batch),
                Err(error) => return Some(Err(error)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Selected;
    use arrow_array::cast::AsArray;
    use arrow_array::types::Int32Type;
    use arrow_array::{ArrayRef, Int32Array, RecordBatch};
    use std::convert::Infallible;
    use std::sync::Arc;

    fn batch(values: std::ops::Range<i32>) -> Result<RecordBatch, arrow_schema::ArrowError> {
        let column: ArrayRef = Arc::new(Int32Array::from_iter_values(values));
        RecordBatch::try_from_iter([("v", column)])
    }

    #[test]
    fn selected_rows_come_out_in_stored_order_across_batches()
    -> Result<(), Box<dyn std::error::Error>> {
        let batches = vec![batch(0..4)?, batch(4..8)?, batch(8..10)?];
        let selected = Selected::new(
            batches.into_iter().map(Ok::<_, Infallible>),
            vec![9, 1, 2, 3, 4, 2, 7, 42],
        );
        let mut values = Vec::new();
        let mut slices = 0;
        for batch in selected {
            slices += 1;
            values.extend(
                batch?
                    .column(0)
                    .as_primitive::<Int32Type>()
                    .values()
                    .iter()
                    .copied(),
            );
        }
        assert_eq!(values, [1, 2, 3, 4, 7, 9]);
        assert_eq!(slices, 4);
        Ok(())
    }
}
