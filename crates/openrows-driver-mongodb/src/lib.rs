// SPDX-License-Identifier: Apache-2.0
//! `MongoDB` MQL, document results and guarded document transactions.
mod cancel;
mod catalog;
mod command;
mod driver;
mod editor;
mod error;
mod proxy;
mod session;
mod streaming;
mod validation;
pub use driver::{MongoDriver, connect};
pub use editor::validate_edits;
pub use session::MongoSession;
