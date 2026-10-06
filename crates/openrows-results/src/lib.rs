// SPDX-License-Identifier: Apache-2.0
//! In-memory Arrow results and uncompressed viewport IPC streams.
mod store;
pub mod synthetic;
pub use store::{ResultStore, Window, WindowError};
#[cfg(test)]
mod tests;
