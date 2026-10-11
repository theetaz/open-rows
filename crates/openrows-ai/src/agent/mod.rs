// SPDX-License-Identifier: Apache-2.0
//! Bounded provider-agnostic assistant, with data kept out of its control plane.
mod budget;
pub(crate) mod calls;
mod types;
use crate::{AiControl, RequestOptions, sharing::GatedPayload};
use std::time::Duration;
use tokio::sync::mpsc;
pub use types::{
    AgentError, AgentEvent, AgentProvider, AgentTools, Continuation, Message, ToolCall,
    ToolDefinition, ToolReply, Trace, Turn, TurnRequest,
};

/// Hard ceilings for one user-triggered investigation.
#[derive(Clone, Copy, Debug)]
pub struct AgentLimits {
    /// Maximum provider rounds plus tool calls.
    pub steps: u32,
    /// Conservative one-token-per-byte accounting, including retransmitted inputs.
    pub tokens: u32,
    /// Total deadline covering tools, transport and consumer backpressure.
    pub timeout: Duration,
}
impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            steps: 12,
            tokens: 65536,
            timeout: Duration::from_secs(60),
        }
    }
}
/// Mandatory gated context, explicit model, limits and cancellation.
pub struct AgentRequest {
    /// User-selected model identifier.
    pub model: String,
    /// Exact initial context and revocation guard.
    pub context: GatedPayload,
    /// Only tighter budgets are accepted.
    pub limits: AgentLimits,
    /// User cancellation and caller's deadline.
    pub options: RequestOptions,
}
/// Run the same finite loop for every provider; no text can become a tool call.
/// # Errors
/// Typed transport, policy, cancellation, deadline, malformed-call or budget failures.
pub async fn run_agent(
    provider: &dyn AgentProvider,
    tools: &dyn AgentTools,
    request: &AgentRequest,
    sink: mpsc::Sender<AgentEvent>,
) -> Result<(), AgentError> {
    budget::validate(request)?;
    let control = AiControl::default();
    control.set_enabled(true);
    let options = RequestOptions {
        cancel: request.options.cancel.clone(),
        timeout: request.options.timeout.min(request.limits.timeout),
    };
    // The outer guard bounds tool waits as well as text-channel backpressure.
    request
        .context
        .guard(control.run(&options, async {
            Ok(investigate(provider, tools, request, sink).await)
        }))
        .await??;
    Ok(())
}
async fn investigate(
    provider: &dyn AgentProvider,
    tools: &dyn AgentTools,
    request: &AgentRequest,
    sink: mpsc::Sender<AgentEvent>,
) -> Result<(), AgentError> {
    let definitions = if provider.tool_calling() {
        tools.definitions()
    } else {
        vec![]
    };
    let mut budget = budget::Budget::new(request.limits);
    let mut messages = Vec::new();
    loop {
        budget.step()?;
        budget.input(&request.context, &messages, &definitions)?;
        let (turn, text) = budget
            .round(provider, request, (&messages, &definitions), &sink)
            .await?;
        calls::validate(&turn.calls, &definitions)?;
        if turn.calls.is_empty() {
            sink.send(AgentEvent::Done {
                tokens: budget.used(),
            })
            .await
            .map_err(|_| crate::AiError::Cancelled)?;
            return Ok(());
        }
        messages.push(Message::Assistant {
            text,
            calls: turn.calls.clone(),
            continuation: turn.continuation,
        });
        for call in turn.calls {
            budget.step()?;
            let message = calls::perform(tools, &call, budget.steps(), &sink).await?;
            messages.push(message);
        }
    }
}
