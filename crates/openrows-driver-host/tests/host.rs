#![allow(clippy::unwrap_used, reason = "synthetic fixtures and test assertions")]
// SPDX-License-Identifier: Apache-2.0
//! Real-process lifecycle, security and backpressure acceptance tests.
#[path = "host/adapter.rs"]
mod adapter;
#[path = "host/admission.rs"]
mod admission;
#[path = "host/control.rs"]
mod control;
#[path = "host/core.rs"]
mod core;
#[path = "host/facets.rs"]
mod facets;
#[path = "host/handshake.rs"]
mod handshake;
#[path = "host/isolation.rs"]
mod isolation;
#[path = "host/parent.rs"]
mod parent;
#[path = "host/store.rs"]
mod store;
mod support;
#[path = "host/timeouts.rs"]
mod timeouts;
