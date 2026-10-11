// SPDX-License-Identifier: Apache-2.0
//! Tauri-free, bounded monitoring and redacted Arrow windows (ADR-0011).
mod analysis;
mod arrow;
mod blockers;
mod model;
mod retention;
mod runtime;
mod schedule;
mod source_analysis;

pub use analysis::{Counter, counter_delta, ratio};
pub use model::*;
pub use retention::LiveStore;
pub use runtime::{Monitor, MonitorReader, ReadFuture};
pub use schedule::Cadence;
