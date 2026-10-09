// SPDX-License-Identifier: Apache-2.0
//! Cloud and zero-trust helper hops with structured CLI arguments and shared lifetimes.
mod arguments;
mod config;
mod dialer;
mod error;
mod manager;
mod process;
mod stderr;
mod validation;
pub use config::{CloudFlavor, HelperHop};
pub use dialer::HelperDialer;
pub use error::HelperError;
pub use manager::{Helper, Helpers};
pub use process::{HelperSpec, local_endpoint};
pub use stderr::scrub;
pub mod tools;
