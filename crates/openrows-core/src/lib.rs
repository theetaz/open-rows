// SPDX-License-Identifier: Apache-2.0
//! `OpenRows` engine core: the actor-per-session runtime (ADR-0007, docs/04 §3).
//!
//! [`SessionManager`] spawns one tokio task per session that exclusively owns
//! a driver [`openrows_driver_api::Session`]. Commands travel over a bounded
//! queue; each query reports [`QueryEvent`]s on its own channel and writes
//! result batches to a [`BatchSink`]. Cancellation is out of band: a
//! [`Canceller`] fires the query's token and the actor sends the driver's
//! cancel request without waiting for the statement. A panicking driver only
//! fails its own session. No Tauri dependency (docs/03 §5).
//!
//! Sessions opened with [`SessionManager::open_resilient`] probe their
//! connection while idle and reconnect transparently when it was lost; a
//! statement that was running is reported, never retried (M1-08).
//!
//! [`openrows_results::ResultStore`] is the production [`BatchSink`].
//! [`script::run_script`] runs editor text statement by statement after
//! [`safety::SafetyPlan`] has gated it (read-only, confirm-before-write).
//! [`export`] streams a statement's rows, or a stored result, to a file.
#![forbid(unsafe_code)]
mod actor;
mod error;
mod event;
pub mod export;
mod handle;
mod manager;
mod resilience;
mod results;
pub mod safety;
pub mod script;
mod shared;
mod sink;
#[cfg(test)]
mod tests;

pub use error::{CoreError, ErrorKind};
pub use event::{QueryEvent, SessionEvent, SessionState};
pub use handle::{Canceller, QueryHandle};
pub use manager::SessionManager;
pub use resilience::{Reconnect, Resilience, RetryPolicy};
pub use sink::BatchSink;
