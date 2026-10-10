#![allow(clippy::unwrap_used, reason = "synthetic fixtures and test assertions")]
// SPDX-License-Identifier: Apache-2.0
//! Real-process lifecycle, security and backpressure acceptance tests.
#[path = "host/control.rs"]
mod control;
#[path = "host/handshake.rs"]
mod handshake;
#[path = "host/isolation.rs"]
mod isolation;
#[path = "host/parent.rs"]
mod parent;
mod support;
