// SPDX-License-Identifier: Apache-2.0
//! In-memory Arrow results and uncompressed viewport IPC streams.
//!
//! Results arrive either complete ([`ResultStore::insert`]) or streamed
//! ([`ResultStore::begin`] then [`ResultStore::append`]); windows can be read
//! while a result is still streaming.
mod limits;
mod slice;
mod store;
pub mod synthetic;
pub use limits::{Append, DEFAULT_MAX_BYTES, DEFAULT_MAX_ROWS, Limits};
pub use store::{ResultStore, Window, WindowError};
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_stream;
