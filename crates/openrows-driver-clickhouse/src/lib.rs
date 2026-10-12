// SPDX-License-Identifier: Apache-2.0
//! `ClickHouse` HTTP(S) over `OpenRows` routes, streaming native Arrow IPC.
#![forbid(unsafe_code)]

mod admin;
mod catalog;
mod driver;
mod error;
mod http;
mod monitor;
mod query;
mod request;
mod session;
mod stream;
mod transport;

pub use driver::{ClickHouseDriver, connect};
pub use session::ClickHouseSession;

#[cfg(test)]
mod tests;
