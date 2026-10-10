// SPDX-License-Identifier: Apache-2.0
//! libSQL/Turso remote driver: Hrana 3 pipelines, bounded incremental JSON
//! decoding, baton transactions and request-abort cancellation (docs/05 §18).
#![forbid(unsafe_code)]
mod cancel;
mod capabilities;
mod catalog;
mod config;
mod driver;
mod editor;
mod error;
mod execute;
mod http;
mod parse;
mod protocol;
mod session;
mod transport;

pub use config::ResultLimits;
pub use driver::{LibsqlDriver, connect};
pub use protocol::{NamedArg, Value};
pub use session::LibsqlSession;
