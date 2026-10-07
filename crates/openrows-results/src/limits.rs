// SPDX-License-Identifier: Apache-2.0
//! Memory accounting limits for streamed results (docs/06 §3).

/// Default per-result row cap; the query stops with `truncated` when reached.
pub const DEFAULT_MAX_ROWS: usize = 1_000_000;
/// Default Arrow memory budget across all results (no spill to disk yet).
pub const DEFAULT_MAX_BYTES: usize = 1 << 30;

/// Caps that stop a streamed result early.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// Rows kept per result.
    pub max_rows: usize,
    /// Arrow memory (`get_array_memory_size`) kept across all results.
    pub max_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_rows: DEFAULT_MAX_ROWS,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

/// Whether the producer may keep streaming after an append.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Append {
    /// The batch was stored; send more.
    Continue,
    /// Stop: a limit was hit or the result is gone; the result is truncated.
    Full,
}
