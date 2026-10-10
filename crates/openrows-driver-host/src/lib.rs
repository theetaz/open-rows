// SPDX-License-Identifier: Apache-2.0
//! Isolated `SQLite` test host; native registry and worker never run in the parent.
mod bootstrap;
mod control;
mod facet_output;
mod facets;
mod flow;
#[cfg(feature = "test-hooks")]
mod hooks;
mod output;
mod registry;
mod sink;
mod worker;

/// Run the one-connection helper from its bounded inherited bootstrap.
/// # Errors
/// Bootstrap, authentication, worker or transport failure (payload-free).
pub async fn run() -> Result<(), openrows_host_protocol::ProtocolError> {
    bootstrap::run().await
}

#[cfg(feature = "test-hooks")]
pub use hooks::{fixture, fixture_large};
