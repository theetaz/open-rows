// SPDX-License-Identifier: Apache-2.0
//! Prototype `PostgreSQL` driver from the M0-S3 streaming spike.
//!
//! Speaks the v3 wire protocol through `postgres-protocol`, requests binary
//! results for known types and text for the rest (per column), and decodes
//! `DataRow` cells straight into pre-sized Arrow builders. Not the production
//! M1-11 driver: no TLS, cancel, parameters, notices or flavour detection.
#![forbid(unsafe_code)]
mod decode;
pub mod fixture;
mod session;
mod startup;
mod stream;
mod types;
mod wire;

pub use decode::ResultDecoder;
pub use session::PgSession;
pub use startup::PgConfig;
pub use types::ColumnDesc;
