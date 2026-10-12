// SPDX-License-Identifier: Apache-2.0
//! Explicit in-app schedules; no polling tick can opt into a model request.
use crate::{
    agent::{AgentError, AgentLimits},
    validate_id,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Finite recurring investigation configuration; never enabled by default.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ipc-types", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvestigationSchedule {
    /// Explicit opt-in; false is a stop request.
    pub enabled: bool,
    /// Chosen saved provider, never a fallback.
    pub provider: String,
    /// Explicit model selection.
    pub model: String,
    /// Recurrence, at least five minutes; first run waits for the interval.
    pub interval_secs: u32,
    /// Maximum runs before this opt-in expires.
    pub max_runs: u32,
    /// Maximum tool calls and provider rounds.
    pub steps: u32,
    /// Conservative input/output byte/token ceiling.
    pub tokens: u32,
    /// Total preparation/tool/provider deadline.
    pub timeout_secs: u32,
    /// Maximum estimated per-run spend, millionths of the provider's currency.
    pub max_cost_micros: u32,
    /// Conservative maximum input/output price per million tokens, in millionths.
    pub price_per_million_micros: u32,
    /// Native static tools selected by the user.
    pub tools: Vec<String>,
}
/// Only read/propose monitoring tools are eligible; SQL execution is separately user-triggered.
pub const SCHEDULE_TOOLS: [&str; 8] = [
    "read_health_snapshot",
    "read_metric_history",
    "read_query_statistics",
    "read_lock_graph",
    "read_monitor_plans",
    "record_health_finding",
    "build_report",
    "propose_board",
];
impl InvestigationSchedule {
    /// Refuse unlimited/unknown pricing, provider selection, tools and resource limits.
    /// # Errors
    /// Invalid configuration or a spend cap too small for one conservative token.
    pub fn limits(&self) -> Result<AgentLimits, AgentError> {
        validate_id(&self.provider).map_err(|_| AgentError::Malformed)?;
        if self.model.is_empty()
            || self.model.len() > 256
            || !(300..=86400).contains(&self.interval_secs)
            || !(1..=1000).contains(&self.max_runs)
            || !(1..=12).contains(&self.steps)
            || !(1..=65536).contains(&self.tokens)
            || !(1..=60).contains(&self.timeout_secs)
            || self.max_cost_micros == 0
            || self.price_per_million_micros == 0
            || !self.tools.iter().any(|tool| tool == "read_health_snapshot")
            || self.tools.is_empty()
            || self.tools.len() > SCHEDULE_TOOLS.len()
            || self
                .tools
                .iter()
                .any(|tool| !SCHEDULE_TOOLS.contains(&tool.as_str()))
        {
            return Err(AgentError::Malformed);
        }
        let affordable =
            u64::from(self.max_cost_micros) * 1_000_000 / u64::from(self.price_per_million_micros);
        let tokens = self
            .tokens
            .min(u32::try_from(affordable).unwrap_or(u32::MAX));
        if tokens == 0 {
            return Err(AgentError::Tokens);
        }
        Ok(AgentLimits {
            steps: self.steps,
            tokens,
            timeout: Duration::from_secs(u64::from(self.timeout_secs)),
        })
    }
    /// Ceil-rounded conservative estimated spend; includes retransmitted inputs.
    #[must_use]
    pub fn cost_micros(&self, tokens: u32) -> u32 {
        u32::try_from(
            (u64::from(tokens) * u64::from(self.price_per_million_micros)).div_ceil(1_000_000),
        )
        .unwrap_or(u32::MAX)
    }
}
