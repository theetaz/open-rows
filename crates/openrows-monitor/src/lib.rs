// SPDX-License-Identifier: Apache-2.0
//! Tauri-free, bounded monitoring and redacted Arrow windows (ADR-0011).
pub mod advisor;
mod analysis;
mod arrow;
mod blockers;
mod gauges;
mod investigation;
mod model;
mod retention;
mod retention_support;
mod runtime;
mod schedule;
mod source_analysis;
mod statement_history;

pub use analysis::{Counter, counter_delta, ratio};
pub use investigation::{Investigation, Regression, StatementInterval};
pub use model::*;
pub use retention::LiveStore;
pub use runtime::{DiagnosticGuard, Monitor, MonitorReader, ReadFuture};
pub use schedule::Cadence;
