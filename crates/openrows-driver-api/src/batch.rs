// SPDX-License-Identifier: Apache-2.0
//! Batch sizing rules from docs/05 §2: an early first batch, then bounded batches.

/// When a driver flushes accumulated rows into a `RecordBatch`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchPolicy {
    /// Upper bound on rows in the first batch, so the UI can paint quickly.
    pub first_batch_rows: usize,
    /// Target cells (rows × columns) for every later batch.
    pub target_cells: usize,
    /// Flush once this many wire bytes have been decoded into the open batch.
    pub max_bytes: usize,
}

impl Default for BatchPolicy {
    fn default() -> Self {
        Self {
            first_batch_rows: 200,
            target_cells: 64 * 1024,
            max_bytes: 4 * 1024 * 1024,
        }
    }
}

impl BatchPolicy {
    /// Row limit for a batch with `columns` columns; never zero.
    #[must_use]
    pub fn row_limit(&self, columns: usize, first: bool) -> usize {
        let by_cells = (self.target_cells / columns.max(1)).max(1);
        if first {
            by_cells.min(self.first_batch_rows.max(1))
        } else {
            by_cells
        }
    }

    /// True when an open batch with `rows` rows and `bytes` wire bytes must flush.
    #[must_use]
    pub fn should_flush(&self, rows: usize, bytes: usize, limit: usize) -> bool {
        rows >= limit || bytes >= self.max_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::BatchPolicy;

    #[test]
    fn first_batch_is_capped_at_200_rows() {
        let policy = BatchPolicy::default();
        assert_eq!(policy.row_limit(1, true), 200);
        assert_eq!(policy.row_limit(20, true), 200);
        assert_eq!(policy.row_limit(1, false), 65_536);
    }

    #[test]
    fn later_batches_target_64k_cells() {
        let policy = BatchPolicy::default();
        assert_eq!(policy.row_limit(20, false), 3_276);
        assert_eq!(policy.row_limit(200, false), 327);
        assert!(policy.row_limit(200, false) * 200 <= 65_536);
    }

    #[test]
    fn wide_results_keep_the_cell_target_for_the_first_batch() {
        let policy = BatchPolicy::default();
        assert_eq!(policy.row_limit(1_000, true), 65);
        assert_eq!(policy.row_limit(100_000, true), 1);
        assert_eq!(policy.row_limit(0, false), 65_536);
    }

    #[test]
    fn byte_budget_flushes_before_the_row_limit() {
        let policy = BatchPolicy::default();
        assert!(!policy.should_flush(10, 1_024, 200));
        assert!(policy.should_flush(200, 0, 200));
        assert!(policy.should_flush(10, 4 * 1024 * 1024, 200));
    }
}
