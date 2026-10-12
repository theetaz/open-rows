// SPDX-License-Identifier: Apache-2.0
//! Metadata-only verification and cancellable text streaming for BYOK/local AI.
pub mod agent;
mod agent_wire;
mod config;
mod continuation;
mod control;
mod error;
mod http;
pub mod investigation;
mod models;
mod provider;
pub mod scheduled;
pub mod sharing;
mod streaming;
mod tool_stream;
mod types;
mod wire;

pub use config::{AiSettings, ProviderConfig, ProviderKind, validate_id};
pub use control::{AiControl, Cancellation, RequestOptions};
pub use error::AiError;
pub use http::NetworkOptions;
pub use provider::{HttpProvider, Provider, TextRequest};
pub use types::{Capabilities, Health, Model, Usage, Verification};
