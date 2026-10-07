// SPDX-License-Identifier: Apache-2.0
//! Minimal driver contract drafted in the M0-S3 `PostgreSQL` streaming spike.
//!
//! Drivers decode wire rows straight into Arrow builders and hand finished
//! `RecordBatch`es to a [`ResultSink`]; [`BatchPolicy`] decides when to flush.
//! The full contract (capabilities, cancel, introspection, editing) is M1-10;
//! see docs/05 §2.
mod batch;
mod error;
pub mod metadata;
mod session;
mod sink;

pub use batch::BatchPolicy;
pub use error::DriverError;
pub use session::{BoxFuture, Session};
pub use sink::{ExecSummary, ResultSink, SinkFlow};
