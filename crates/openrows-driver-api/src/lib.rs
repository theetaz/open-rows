// SPDX-License-Identifier: Apache-2.0
//! Minimal driver contract drafted in the M0-S3 `PostgreSQL` streaming spike.
//!
//! Drivers decode wire rows straight into Arrow builders and hand finished
//! `RecordBatch`es to a [`ResultSink`]; [`BatchPolicy`] decides when to flush.
//! [`CancelHandle`] gives the session actor out-of-band cancellation (M1-01).
//! [`catalog`] holds the object model the schema tree lists (D-06) and
//! [`schema`] the table detail and typed structure changes. [`Driver`] opens
//! a [`Connection`] from a [`ConnectRequest`] (D-12), recording a
//! [`ConnectTrace`] for Test connection; connections offer [`Catalog`] and
//! [`Editor`] facets. Capabilities, explain and admin facets remain M1-10;
//! see docs/05 §2.
mod batch;
pub mod catalog;
pub mod connect;
pub mod connection;
pub mod driver;
mod error;
pub mod file;
pub mod metadata;
pub mod schema;
mod session;
mod sink;
pub mod structure;
pub mod trace;

pub use batch::BatchPolicy;
pub use connect::{ConnectRequest, ConnectTimeouts, Endpoint, SslMode, TlsFiles};
pub use connection::{
    Catalog, Connection, Editor, RowEditInfo, RowSnapshot, ServerSummary, catalog, editor,
};
pub use driver::{Driver, GenerateError};
pub use error::DriverError;
pub use file::{Attachment, FileSource};
pub use session::{BoxFuture, CancelHandle, Session};
pub use sink::{DiscardSink, ExecSummary, NoticeLevel, ResultSink, SinkFlow};
pub use trace::{ConnectStage, ConnectTrace, TlsStatus};
