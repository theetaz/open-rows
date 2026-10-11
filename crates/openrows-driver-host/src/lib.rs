// SPDX-License-Identifier: Apache-2.0
//! Isolated native `DuckDB` host (with a test-only `SQLite` registry); native registry and worker never run in the parent.
mod bootstrap;
mod control;
#[cfg(feature = "duckdb")]
mod duckdb;
mod facet_output;
mod facets;
mod flow;
#[cfg(feature = "test-hooks")]
mod hooks;
mod output;
mod registry;
mod sink;
mod startup;
mod worker;

/// Run the one-connection helper from its bounded inherited bootstrap.
/// # Errors
/// Bootstrap, authentication, worker or transport failure (payload-free).
pub async fn run() -> Result<(), openrows_host_protocol::ProtocolError> {
    bootstrap::run(registry::production).await
}

/// Run the SQLite/probe fixture without referencing the production native registry.
/// # Errors
/// Bootstrap, authentication, worker or transport failure (payload-free).
#[cfg(feature = "test-sqlite")]
pub async fn run_sqlite_fixture() -> Result<(), openrows_host_protocol::ProtocolError> {
    bootstrap::run(registry::sqlite).await
}

#[cfg(feature = "test-hooks")]
pub use hooks::{fixture, fixture_large};
