// SPDX-License-Identifier: Apache-2.0
//! Discovery facts; absent capability and billing information stays unknown.
use crate::AiError;
use serde::{Deserialize, Serialize};

/// Model capabilities reported by discovery; `None` means unknown.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// Adapter supports incremental text transport.
    pub streaming: Option<bool>,
    /// Model supports tools.
    pub tool_calling: Option<bool>,
    /// Model supports JSON/structured output.
    pub structured_output: Option<bool>,
    /// Model accepts images.
    pub vision: Option<bool>,
    /// Reported input token limit.
    pub context_window: Option<u32>,
    /// Reported output token limit.
    pub max_output: Option<u32>,
}

/// A model identifier and its discovery metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// Opaque provider model identifier.
    pub id: String,
    /// Provider display name, or identifier when absent.
    pub name: String,
    /// Capabilities without guessed model limits.
    pub capabilities: Capabilities,
}

/// Reported key usage/limits, in the provider's currency units (`OpenRouter` USD).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Total reported spend.
    pub used: Option<f64>,
    /// Configured spending limit.
    pub limit: Option<f64>,
    /// Remaining key allowance.
    pub remaining: Option<f64>,
}

/// Verification state without inferring generation access from a public catalogue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Health {
    /// Saved but not probed in this session.
    Configured,
    /// Authenticated metadata endpoint succeeded (local servers: reachable).
    Verified,
    /// Probe failed with a scrubbed typed reason.
    Failed {
        /// Safe reason.
        reason: AiError,
    },
    /// Metadata cannot establish health/authentication.
    Unknown,
}

/// Metadata-only probe result safe for IPC; contains no raw HTTP response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    /// Probe outcome.
    pub health: Health,
    /// Discovered models (fallbacks are explicitly Unknown).
    pub models: Vec<Model>,
    /// Key billing metadata, when exposed by the provider.
    pub usage: Option<Usage>,
}
