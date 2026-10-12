// SPDX-License-Identifier: Apache-2.0
//! SQL Server and Azure SQL driver (M2-02, Tier 2).
//!
//! Speaks TDS 7.4 ([`tds`]) over any `openrows-net` stream (direct TCP, an
//! SSH tunnel or a proxy). TLS is negotiated in `PRELOGIN` and runs the
//! rustls handshake of `openrows-net` inside `PRELOGIN` packets, with
//! libpq `sslmode` semantics. Logins use SQL Server authentication;
//! Windows/Kerberos and Microsoft Entra ID are not supported.
//!
//! A statement (or a whole T-SQL batch) runs per call; bound values go
//! through `sp_executesql` as `nvarchar(max)` text. Every result set
//! streams into Arrow batches ([`decode`]), messages and `PRINT` output
//! become notices, and cancel sends the TDS attention signal on the same
//! connection. The catalog reads the `sys` views; staged edits and
//! structure changes commit in one transaction with exact row guards.
#![forbid(unsafe_code)]
mod admin;
mod capabilities;
mod catalog;
mod ddl;
mod decode;
mod driver;
mod editor;
mod error;
mod handshake;
mod info;
mod monitor;
mod reply;
mod rows;
mod session;
mod tds;
mod transport;

pub use driver::{SqlServerDriver, connect};
pub use session::SqlServerSession;
