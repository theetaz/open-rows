// SPDX-License-Identifier: Apache-2.0
//! Integration tests against a real `MySQL` or `MariaDB` server; run with
//! `just test-mysql` (ignored in the default run).
mod catalog;
mod diagram;
mod edits;
mod helpers;
mod inspection;
mod inspection_visibility;
mod key_constraints;
mod monitor;
mod monitor_bench;
mod objects;
mod session;
mod structure;
mod support;
mod temporal;

#[path = "connection_import.rs"]
mod connection_import;
