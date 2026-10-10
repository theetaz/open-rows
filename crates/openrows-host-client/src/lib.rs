// SPDX-License-Identifier: Apache-2.0
//! Lazy isolated-session launcher and Tauri-free driver adapter.
mod adapter;
mod adapter_metrics;
pub use adapter_metrics::HostMetrics;
mod adapter_cancel;
mod adapter_catalog;
mod adapter_editor;
mod adapter_facet;
mod adapter_session;
mod adapter_stream;
mod adapter_trace;
mod admission;
pub use adapter::{HOSTED_SQLITE_CAPABILITIES, HostedConnection, HostedDriver};
mod binary;
mod circuit;
mod failure_signal;
mod handshake;
mod launch;
mod monitor;
mod platform;
mod resolve;
mod runtime;
mod session;
mod supervisor;
pub mod transport;

pub use admission::{HostReservation, LargeReservation, MemoryBudget};
pub use circuit::{CircuitBreaker, Clock, SystemClock};
pub use handshake::{read_bootstrap, write_bootstrap};
pub use launch::{LaunchOptions, launch_command};
pub use openrows_driver_api::HostFailure;
pub use openrows_host_protocol::HostDriverId;
pub use resolve::{HOST_BINARY_NAME, resolve_installed};
pub use runtime::DriverHostRuntime;
pub use session::{HostSession, HostState};
pub use supervisor::HostSupervisor;

/// Per-child monitored memory threshold (in addition to engine limits).
pub const HOST_MEMORY: usize = 256 * 1024 * 1024;
/// Shared active-host and exclusive-batch reservation ceiling.
pub const AGGREGATE_MEMORY: usize = 512 * 1024 * 1024;
/// Handshake deadline, separate from driver connect deadlines.
pub const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
/// Close/cancel/stop grace before targeted termination.
pub const CLOSE_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

#[cfg(windows)]
pub use platform::close_bootstrap_stdin;

#[cfg(windows)]
pub use platform::is_process_alive;
