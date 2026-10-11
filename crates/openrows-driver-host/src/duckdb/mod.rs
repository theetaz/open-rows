// SPDX-License-Identifier: Apache-2.0
//! Host-only native `DuckDB` connection; the independent control runtime interrupts it.
mod bridge;
mod connect;
mod error;
mod nested;
mod session;
mod stream;
pub(crate) use connect::connect;
