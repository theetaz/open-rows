// SPDX-License-Identifier: Apache-2.0
//! Arrow results in memory with spill to disk, and uncompressed viewport
//! IPC streams.
//!
//! Results arrive either complete ([`ResultStore::insert`]) or streamed
//! ([`ResultStore::begin`] then [`ResultStore::append`]); windows can be read
//! while a result is still streaming. A store created with a [`SpillDir`]
//! keeps at most [`Limits::max_bytes`] of Arrow memory and spills the rest
//! to LZ4-compressed Arrow IPC files (docs/06 §3).
mod admission;
pub use admission::Admission;
mod cache;
mod cell;
mod limits;
mod set;
mod shared;
mod slice;
mod spill;
mod store;
mod stored;
pub mod synthetic;
pub use cell::{CellInfo, CellKind};
pub use limits::{Append, DEFAULT_MAX_BYTES, DEFAULT_MAX_ROWS, Limits};
pub use spill::SpillDir;
pub use store::{ResultStore, Window, WindowError};
pub use stored::StoredBatches;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_spill;
#[cfg(test)]
mod tests_stream;

#[cfg(test)]
mod tests_admission;
