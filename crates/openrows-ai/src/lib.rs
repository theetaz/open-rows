// SPDX-License-Identifier: Apache-2.0
//! Metadata-only verification and cancellable text streaming for BYOK/local AI.
mod config;
mod control;
mod error;
mod http;
mod models;
mod provider;
mod streaming;
mod types;
mod wire;

pub use config::{AiSettings, ProviderConfig, ProviderKind, validate_id};
pub use control::{AiControl, Cancellation, RequestOptions};
pub use error::AiError;
pub use http::NetworkOptions;
pub use provider::{HttpProvider, Provider, TextRequest};
pub use types::{Capabilities, Health, Model, Usage, Verification};
