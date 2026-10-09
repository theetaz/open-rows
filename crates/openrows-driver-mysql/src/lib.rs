// SPDX-License-Identifier: Apache-2.0
//! `MySQL` and `MariaDB` driver (D-12, dogfood slice of M1-12).
//!
//! Speaks the client/server protocol over any `openrows-net` stream (direct
//! TCP, a Unix socket file, or an SSH tunnel stream), with packets and the
//! login plugins from `mysql_common` (`caching_sha2_password` including its
//! RSA key exchange, `mysql_native_password`). TLS is negotiated with an
//! `SSLRequest` packet and the rustls handshake of `openrows-net`, with
//! libpq `sslmode` semantics. One statement runs per call
//! (`CLIENT_MULTI_STATEMENTS` is never set); bound parameters use
//! server-side prepared statements. Results stream into Arrow batches
//! ([`decode`]), warnings become notices, and cancel sends `KILL QUERY`
//! from a side connection. The catalog reads `information_schema`; staged
//! edits commit in one transaction with byte-exact row guards.
#![forbid(unsafe_code)]
mod auth;
mod capabilities;
mod catalog;
mod ddl;
mod decode;
mod driver;
mod editor;
mod error;
mod handshake;
mod info;
mod packet;
mod prepared;
mod rows;
mod session;
mod transport;
mod values;
mod wire;

pub use driver::{Flavour, MySqlDriver, connect};
pub use session::MySqlSession;
