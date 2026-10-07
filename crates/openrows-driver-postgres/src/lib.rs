// SPDX-License-Identifier: Apache-2.0
//! `PostgreSQL` driver (dogfood slice D-04 of M1-11).
//!
//! Speaks the v3 wire protocol through `postgres-protocol`, requests binary
//! results for known types and text for the rest (per column), and decodes
//! `DataRow` cells straight into pre-sized Arrow builders. Connects over TCP or
//! a Unix socket with per-stage timeouts, reports [`ServerInfo`], runs startup
//! commands, cancels out of band and forwards notices. Values with no exact
//! Arrow form fall back to tagged text. Lists databases, schemas and objects
//! for the schema tree (`PgSession::list_*`, `search_index`). No TLS (D-11)
//! or bind parameters yet.
#![forbid(unsafe_code)]
mod cancel;
mod connect;
mod decode;
pub mod fixture;
mod info;
mod introspect;
mod options;
mod session;
mod simple;
mod startup;
mod stream;
mod types;
mod wire;

pub use decode::ResultDecoder;
pub use info::{Flavour, ServerInfo, TlsStatus};
pub use options::{ConnectOptions, ConnectTimeouts, SslMode};
pub use session::PgSession;
pub use startup::PgConfig;
pub use types::ColumnDesc;
